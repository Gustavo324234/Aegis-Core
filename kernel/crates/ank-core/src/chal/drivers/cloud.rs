use crate::chal::{
    DriverStatus, ExecutionError, GenerateStreamResult, Grammar, InferenceDriver, SystemError,
};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::sync::Arc;
use std::time::Duration;

const RETRYABLE_STATUS_CODES: &[u16] = &[429, 502, 503, 504];
const MAX_RETRIES: u32 = 2;
const BASE_DELAY_MS: u64 = 1000;

#[derive(Debug, Clone)]
pub struct CloudProxyDriver {
    pub api_url: String,
    pub api_key: String,
    pub model_id: String,
    client: Arc<Client>,
}

impl CloudProxyDriver {
    pub fn new(client: Arc<Client>, api_url: String, api_key: String, model_id: String) -> Self {
        Self {
            client,
            api_url,
            api_key,
            model_id,
        }
    }

    pub fn from_env(client: Arc<Client>) -> Option<Self> {
        let api_url = env::var("AEGIS_CLOUD_API_URL").ok()?;
        let api_key = env::var("AEGIS_CLOUD_API_KEY").ok()?;
        let model_id = env::var("AEGIS_CLOUD_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());
        Some(Self::new(client, api_url, api_key, model_id))
    }

    fn is_retryable_error(status: reqwest::StatusCode) -> bool {
        RETRYABLE_STATUS_CODES.contains(&status.as_u16())
    }

    async fn send_with_retry(
        &self,
        request_body: ChatCompletionRequest,
    ) -> Result<reqwest::Response, SystemError> {
        let mut last_error = None;

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay_ms = BASE_DELAY_MS * 2u64.pow(attempt - 1);
                tracing::warn!(
                    attempt = attempt,
                    delay_ms = delay_ms,
                    model = %self.model_id,
                    "Retrying request after error"
                );
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }

            let request = self
                .client
                .post(&self.api_url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .timeout(std::time::Duration::from_secs(30))
                .json(&request_body);

            match request.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response);
                    }

                    let status = response.status();
                    if Self::is_retryable_error(status) && attempt < MAX_RETRIES {
                        let text = response
                            .text()
                            .await
                            .unwrap_or_else(|_| "Unknown".to_string());
                        tracing::warn!(
                            attempt = attempt + 1,
                            status = %status,
                            "Retryable error received"
                        );
                        last_error = Some(SystemError::HardwareFailure(format!(
                            "API Error {}: {}",
                            status, text
                        )));
                        continue;
                    }

                    return Ok(response);
                }
                Err(e) => {
                    if attempt < MAX_RETRIES {
                        tracing::warn!(
                            attempt = attempt + 1,
                            error = %e,
                            "Request failed, will retry"
                        );
                        last_error = Some(SystemError::HardwareFailure(format!(
                            "Reqwest error: {}",
                            e
                        )));
                        continue;
                    }
                    last_error = Some(SystemError::HardwareFailure(format!(
                        "Reqwest error after {} retries: {}",
                        attempt, e
                    )));
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| SystemError::HardwareFailure("Max retries exceeded".to_string())))
    }
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    json_schema: Option<Value>,
}

#[derive(Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallChunk>>,
}

#[derive(Deserialize)]
struct ToolCallChunk {
    index: u32,
    id: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    call_type: Option<String>,
    function: Option<FunctionCallDelta>,
}

#[derive(Deserialize)]
struct FunctionCallDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[async_trait]
impl InferenceDriver for CloudProxyDriver {
    async fn generate_stream(
        &self,
        prompt: String,
        grammar: Option<Grammar>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> GenerateStreamResult {
        let mut request_body = ChatCompletionRequest {
            model: self.model_id.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt,
            }],
            stream: true,
            response_format: None,
            tools,
        };

        if let Some(Grammar::JsonSchema(schema)) = grammar {
            request_body.response_format = Some(ResponseFormat {
                format_type: "json_schema".to_string(),
                json_schema: Some(schema),
            });
        }

        let response = self.send_with_retry(request_body).await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %text, "Cloud API returned error status");
            return Err(SystemError::HardwareFailure(format!(
                "Cloud API Error {}: {}",
                status, text
            )));
        }

        let stream = response.bytes_stream();
        // Tool call accumulator: maps index -> (id, name, arguments_buffer)
        let tool_calls_acc: std::collections::HashMap<u32, (String, String, String)> =
            std::collections::HashMap::new();
        let state = (stream, String::new(), tool_calls_acc);

        let parsed_stream = futures_util::stream::unfold(
            state,
            |(mut stream, mut buffer, mut tool_calls_acc)| async move {
                loop {
                    // Yield any complete lines we already have in buffer
                    while let Some(idx) = buffer.find('\n') {
                        let line = buffer[..idx].trim().to_string();
                        buffer = buffer[idx + 1..].to_string();

                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                // Stream ended — emit any accumulated tool calls
                                if !tool_calls_acc.is_empty() {
                                    let mut calls: Vec<(u32, (String, String, String))> =
                                        tool_calls_acc.drain().collect();
                                    calls.sort_by_key(|(idx, _)| *idx);
                                    for (_, (id, name, arguments)) in calls {
                                        let tool_call_json = serde_json::json!({
                                            "id": id,
                                            "name": name,
                                            "arguments": serde_json::from_str::<serde_json::Value>(&arguments)
                                                .unwrap_or(serde_json::Value::String(arguments.clone())),
                                        });
                                        let token =
                                            format!("__TOOL_CALL__{}", tool_call_json.to_string());
                                        return Some((Ok(token), (stream, buffer, tool_calls_acc)));
                                    }
                                }
                                continue;
                            }
                            if let Ok(parsed) = serde_json::from_str::<ChatCompletionChunk>(data) {
                                if let Some(choice) = parsed.choices.first() {
                                    // Accumulate tool_calls chunks
                                    if let Some(tc_chunks) = &choice.delta.tool_calls {
                                        for tc in tc_chunks {
                                            let entry = tool_calls_acc
                                                .entry(tc.index)
                                                .or_insert_with(|| {
                                                    (String::new(), String::new(), String::new())
                                                });
                                            if let Some(id) = &tc.id {
                                                entry.0 = id.clone();
                                            }
                                            if let Some(func) = &tc.function {
                                                if let Some(name) = &func.name {
                                                    entry.1 = name.clone();
                                                }
                                                if let Some(args) = &func.arguments {
                                                    entry.2.push_str(args);
                                                }
                                            }
                                        }
                                    }

                                    // Emit finish_reason == "tool_calls" accumulated results
                                    if choice.finish_reason.as_deref() == Some("tool_calls") {
                                        if !tool_calls_acc.is_empty() {
                                            let mut calls: Vec<(u32, (String, String, String))> =
                                                tool_calls_acc.drain().collect();
                                            calls.sort_by_key(|(idx, _)| *idx);
                                            for (_, (id, name, arguments)) in calls {
                                                let tool_call_json = serde_json::json!({
                                                    "id": id,
                                                    "name": name,
                                                    "arguments": serde_json::from_str::<serde_json::Value>(&arguments)
                                                        .unwrap_or(serde_json::Value::String(arguments.clone())),
                                                });
                                                let token = format!(
                                                    "__TOOL_CALL__{}",
                                                    tool_call_json.to_string()
                                                );
                                                return Some((
                                                    Ok(token),
                                                    (stream, buffer, tool_calls_acc),
                                                ));
                                            }
                                        }
                                    }

                                    // Emit regular content tokens
                                    if let Some(content) = &choice.delta.content {
                                        if !content.is_empty() {
                                            return Some((
                                                Ok(content.clone()),
                                                (stream, buffer, tool_calls_acc),
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Need more data from the network
                    match stream.next().await {
                        Some(Ok(chunk)) => {
                            if let Ok(text) = String::from_utf8(chunk.to_vec()) {
                                buffer.push_str(&text);
                            } else {
                                return Some((
                                    Err(ExecutionError::Interrupted("Invalid UTF-8 chunk".into())),
                                    (stream, buffer, tool_calls_acc),
                                ));
                            }
                        }
                        Some(Err(e)) => {
                            return Some((
                                Err(ExecutionError::Interrupted(e.to_string())),
                                (stream, buffer, tool_calls_acc),
                            ));
                        }
                        None => {
                            // Stream ended — emit any remaining tool calls
                            if !tool_calls_acc.is_empty() {
                                let mut calls: Vec<(u32, (String, String, String))> =
                                    tool_calls_acc.drain().collect();
                                calls.sort_by_key(|(idx, _)| *idx);
                                for (_, (id, name, arguments)) in calls {
                                    let tool_call_json = serde_json::json!({
                                        "id": id,
                                        "name": name,
                                        "arguments": serde_json::from_str::<serde_json::Value>(&arguments)
                                            .unwrap_or(serde_json::Value::String(arguments.clone())),
                                    });
                                    let token =
                                        format!("__TOOL_CALL__{}", tool_call_json.to_string());
                                    return Some((Ok(token), (stream, buffer, tool_calls_acc)));
                                }
                            }
                            return None; // Stream ended
                        }
                    }
                }
            },
        );

        Ok(Box::pin(crate::chal::SyncStream(parsed_stream)))
    }

    async fn get_health_status(&self) -> DriverStatus {
        DriverStatus {
            is_ready: true,
            vram_usage_bytes: 0,
            active_models: vec![self.model_id.clone()],
        }
    }

    async fn load_model(&mut self, model_id: &str) -> Result<(), SystemError> {
        self.model_id = model_id.to_string();
        Ok(())
    }
}

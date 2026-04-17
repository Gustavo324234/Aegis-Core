use crate::chal::{DriverStatus, ExecutionError, Grammar, InferenceDriver, SystemError};
use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::pin::Pin;
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
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
}

#[async_trait]
impl InferenceDriver for CloudProxyDriver {
    async fn generate_stream(
        &self,
        prompt: &str,
        grammar: Option<Grammar>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String, ExecutionError>> + Send>>, SystemError>
    {
        let mut request_body = ChatCompletionRequest {
            model: self.model_id.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            stream: true,
            response_format: None,
        };

        if let Some(Grammar::JsonSchema(schema)) = grammar {
            request_body.response_format = Some(ResponseFormat {
                format_type: "json_schema".to_string(),
                json_schema: Some(schema),
            });
        }

        let response = self.send_with_retry(request_body).await?;

        let stream = response.bytes_stream();
        let state = (stream, String::new());

        let parsed_stream =
            futures_util::stream::unfold(state, |(mut stream, mut buffer)| async move {
                loop {
                    // Yield any complete lines we already have in buffer
                    while let Some(idx) = buffer.find('\n') {
                        let line = buffer[..idx].trim().to_string();
                        buffer = buffer[idx + 1..].to_string();

                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                continue;
                            }
                            if let Ok(parsed) = serde_json::from_str::<ChatCompletionChunk>(data) {
                                if let Some(choice) = parsed.choices.first() {
                                    if let Some(content) = &choice.delta.content {
                                        if !content.is_empty() {
                                            return Some((Ok(content.clone()), (stream, buffer)));
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
                                    (stream, buffer),
                                ));
                            }
                        }
                        Some(Err(e)) => {
                            return Some((
                                Err(ExecutionError::Interrupted(e.to_string())),
                                (stream, buffer),
                            ));
                        }
                        None => {
                            return None; // Stream ended
                        }
                    }
                }
            });

        Ok(Box::pin(parsed_stream))
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

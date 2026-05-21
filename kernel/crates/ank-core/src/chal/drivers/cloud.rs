use crate::chal::{
    ChatMessage, DriverStatus, ExecutionError, GenerateStreamResult, Grammar, InferenceDriver,
    SystemError,
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

#[derive(Clone)]
pub struct CloudProxyDriver {
    pub api_url: String,
    pub api_key: String,
    pub model_id: String,
    pub key_id: Option<String>,
    client: Arc<Client>,
    /// CORE-267: callback invocado cuando el provider devuelve 429.
    on_rate_limited: Option<Arc<dyn Fn(chrono::DateTime<chrono::Utc>) + Send + Sync>>,
}

impl std::fmt::Debug for CloudProxyDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudProxyDriver")
            .field("api_url", &self.api_url)
            .field("model_id", &self.model_id)
            .field("key_id", &self.key_id)
            .finish()
    }
}

impl CloudProxyDriver {
    pub fn new(client: Arc<Client>, api_url: String, api_key: String, model_id: String) -> Self {
        Self::new_with_callback(client, api_url, api_key, model_id, None, None)
    }

    pub fn new_with_callback(
        client: Arc<Client>,
        api_url: String,
        api_key: String,
        model_id: String,
        key_id: Option<String>,
        on_rate_limited: Option<Arc<dyn Fn(chrono::DateTime<chrono::Utc>) + Send + Sync>>,
    ) -> Self {
        Self {
            client,
            api_url,
            api_key,
            model_id,
            key_id,
            on_rate_limited,
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

    /// CORE-FIX (E): parses a Gemini / Google AI Studio error body looking for
    /// the standardised `RESOURCE_EXHAUSTED` shape and lifts the structured
    /// `retryDelay` value out of `details[]`. Returns
    ///   - `Some((friendly_message, retry_delay_secs))` when it recognises the
    ///     shape (Gemini free-tier quota exhausted), or
    ///   - `None` for any other body, in which case the caller should fall
    ///     back to its generic "API Error N: <body>" formatting.
    ///
    /// Example body Gemini returns:
    /// ```json
    /// {
    ///   "error": {
    ///     "code": 429,
    ///     "status": "RESOURCE_EXHAUSTED",
    ///     "message": "You exceeded your current quota...",
    ///     "details": [
    ///       { "@type": "type.googleapis.com/google.rpc.QuotaFailure", ... },
    ///       { "@type": "type.googleapis.com/google.rpc.RetryInfo",
    ///         "retryDelay": "23s" }
    ///     ]
    ///   }
    /// }
    /// ```
    fn parse_gemini_quota_error(body: &str) -> Option<(String, u64)> {
        let v: serde_json::Value = serde_json::from_str(body).ok()?;
        let err = v.get("error")?;
        let status = err.get("status").and_then(|s| s.as_str()).unwrap_or("");
        let code = err.get("code").and_then(|c| c.as_u64()).unwrap_or(0);
        if status != "RESOURCE_EXHAUSTED" && code != 429 {
            return None;
        }
        let details = err.get("details").and_then(|d| d.as_array());
        let mut retry_secs: u64 = 0;
        if let Some(details) = details {
            for d in details {
                let ty = d.get("@type").and_then(|t| t.as_str()).unwrap_or("");
                if ty.ends_with("RetryInfo") {
                    if let Some(delay) = d.get("retryDelay").and_then(|s| s.as_str()) {
                        // Format is like "23s" or "1.5s". Parse leading number.
                        let trimmed = delay.trim_end_matches('s');
                        if let Ok(f) = trimmed.parse::<f64>() {
                            retry_secs = f.ceil() as u64;
                        }
                    }
                }
            }
        }
        // Sane default: if no retryDelay extracted, give the user *some* hint.
        if retry_secs == 0 {
            retry_secs = 60;
        }
        let msg = format!(
            "Tu plan free de Gemini se agotó. Reintentá en ~{}s o agregá \
             billing en Google AI Studio para subir el límite.",
            retry_secs
        );
        Some((msg, retry_secs))
    }

    /// Tries the known provider-specific 429 / quota error shapes. Returns
    /// `(friendly_message, retry_delay_secs)` when recognised, else `None`.
    fn parse_quota_error(body: &str) -> Option<(String, u64)> {
        Self::parse_gemini_quota_error(body).or_else(|| Self::parse_groq_quota_error(body))
    }

    /// CORE-FIX: parses a Groq (OpenAI-compatible) 429 rate-limit body. Groq
    /// puts a human-readable hint in `error.message`, e.g.:
    /// "Rate limit reached for model `llama-3.3-70b-versatile` ... on tokens per
    ///  minute (TPM): Limit 12000, Used 11336, Requested 3836. Please try again
    ///  in 15.86s. ...". Returns `(friendly_message, retry_delay_secs)` or
    /// `None` when it isn't a Groq rate-limit shape.
    fn parse_groq_quota_error(body: &str) -> Option<(String, u64)> {
        let v: serde_json::Value = serde_json::from_str(body).ok()?;
        let err = v.get("error")?;
        let code = err.get("code").and_then(|c| c.as_str()).unwrap_or("");
        let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("");
        let is_rate_limit = code == "rate_limit_exceeded" || msg.contains("Rate limit");
        if !is_rate_limit {
            return None;
        }
        let retry_secs = Self::extract_retry_after_secs(msg).unwrap_or(60);
        let is_tpm = msg.contains("tokens per minute")
            || err.get("type").and_then(|t| t.as_str()) == Some("tokens");
        let friendly = if is_tpm {
            format!(
                "Alcanzaste el límite de tokens por minuto del modelo (plan free). \
                 Reintentá en ~{}s o subí el tier del proveedor.",
                retry_secs
            )
        } else {
            format!(
                "Alcanzaste el límite de requests del proveedor. Reintentá en ~{}s.",
                retry_secs
            )
        };
        Some((friendly, retry_secs))
    }

    /// Extracts the (ceiled) seconds from a "try again in 15.86s" style hint.
    fn extract_retry_after_secs(msg: &str) -> Option<u64> {
        let idx = msg.find("try again in ")?;
        let after = &msg[idx + "try again in ".len()..];
        let num: String = after
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let secs = num.parse::<f64>().ok()?;
        Some(secs.ceil() as u64)
    }

    /// Invokes the rate-limit callback (if set) to mark the current key as
    /// rate-limited until `delay_secs` from now, and logs it.
    fn mark_rate_limited(&self, delay_secs: u64) {
        if let Some(cb) = &self.on_rate_limited {
            let until = chrono::Utc::now() + chrono::Duration::seconds(delay_secs as i64);
            cb(until);
            tracing::warn!(
                model = %self.model_id,
                cooldown_secs = delay_secs,
                "CORE-267: 429 recibido — key marcada como rate-limited"
            );
        }
    }

    /// CORE-FIX (B2): Anthropic supports prompt caching via a `cache_control`
    /// marker on a content block. The marker has to live INSIDE the content
    /// (not as a top-level field), which means the message's `content` field
    /// must become an array of content blocks instead of a plain string.
    /// OpenRouter forwards this faithfully for Anthropic models.
    ///
    /// We apply it to the first `system` message we find — that's where the
    /// big stable prefix lives (chat_agent.md, persona, tool definitions). A
    /// 5-minute cache hit cuts Anthropic input token cost ~90% for that prefix.
    fn is_anthropic_compatible(api_url: &str, model_id: &str) -> bool {
        let url_lc = api_url.to_lowercase();
        if url_lc.contains("anthropic.com") {
            return true;
        }
        // OpenRouter exposes Anthropic models via the OpenAI-compatible API.
        // Detect them by the model prefix.
        url_lc.contains("openrouter.ai") && model_id.to_lowercase().contains("anthropic/")
    }

    fn apply_anthropic_prompt_caching(body: &mut serde_json::Value) {
        let Some(messages) = body.get_mut("messages").and_then(|m| m.as_array_mut()) else {
            return;
        };
        for msg in messages.iter_mut() {
            let is_system = msg.get("role").and_then(|r| r.as_str()) == Some("system");
            if !is_system {
                continue;
            }
            // Only wrap if content is currently a plain string. If something
            // else already wrote a content-block array, leave it alone.
            let plain_text = match msg.get("content").and_then(|c| c.as_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => continue,
            };
            let wrapped = serde_json::json!([{
                "type": "text",
                "text": plain_text,
                "cache_control": {"type": "ephemeral"}
            }]);
            msg["content"] = wrapped;
            break; // only the first system message
        }
    }

    async fn send_with_retry(
        &self,
        request_body: serde_json::Value,
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
                        // CORE-FIX (E): a 429 with a parseable retry hint (Gemini
                        // quota / Groq TPM) won't clear by retrying inside the
                        // cooldown window — mark the key rate-limited and bail
                        // immediately with the friendly message instead of burning
                        // MAX_RETRIES * backoff first.
                        if status.as_u16() == 429 {
                            if let Some((msg, secs)) = Self::parse_quota_error(&text) {
                                self.mark_rate_limited(secs);
                                return Err(SystemError::HardwareFailure(msg));
                            }
                        }
                        last_error = Some(SystemError::HardwareFailure(format!(
                            "API Error {}: {}",
                            status, text
                        )));
                        continue;
                    }

                    if status.as_u16() == 429 {
                        // CORE-FIX (E): peek at the body to honour the provider's
                        // structured retry hint (Gemini `retryDelay` / Groq "try
                        // again in Ns") instead of always using 60s. 429 is
                        // non-retryable at this point (past MAX_RETRIES) so we
                        // surface the friendly message to the user.
                        let body = response.text().await.unwrap_or_default();
                        let (delay_secs, user_msg) = match Self::parse_quota_error(&body) {
                            Some((msg, secs)) => (secs, Some(msg)),
                            None => (60u64, None),
                        };
                        self.mark_rate_limited(delay_secs);
                        return Err(SystemError::HardwareFailure(
                            user_msg.unwrap_or_else(|| format!("API Error {}: {}", status, body)),
                        ));
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
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
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
        messages: Vec<ChatMessage>,
        grammar: Option<Grammar>,
        tools: Option<Vec<serde_json::Value>>,
    ) -> GenerateStreamResult {
        let mut request_body = ChatCompletionRequest {
            model: self.model_id.clone(),
            messages,
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

        // Serialize once so we can post-process for provider-specific extensions
        // (Anthropic prompt caching today; future: Gemini system_instruction, etc.).
        let mut body_json = serde_json::to_value(&request_body).map_err(|e| {
            SystemError::HardwareFailure(format!("Failed to serialize request body: {}", e))
        })?;

        if Self::is_anthropic_compatible(&self.api_url, &self.model_id) {
            Self::apply_anthropic_prompt_caching(&mut body_json);
            tracing::debug!(
                model = %self.model_id,
                "CORE-FIX (B2): applied anthropic cache_control to system message"
            );
        }

        let response = self.send_with_retry(body_json).await?;

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
        let pending_tokens = std::collections::VecDeque::<String>::new();
        let state = (stream, String::new(), tool_calls_acc, pending_tokens);

        let parsed_stream = futures_util::stream::unfold(
            state,
            |(mut stream, mut buffer, mut tool_calls_acc, mut pending_tokens)| async move {
                loop {
                    // 1. Prioritize any pending tokens in the queue
                    if let Some(token) = pending_tokens.pop_front() {
                        return Some((Ok(token), (stream, buffer, tool_calls_acc, pending_tokens)));
                    }

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
                                        let token = format!("__TOOL_CALL__{}", tool_call_json);
                                        pending_tokens.push_back(token);
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
                                    if choice.finish_reason.as_deref() == Some("tool_calls")
                                        && !tool_calls_acc.is_empty()
                                    {
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
                                            let token = format!("__TOOL_CALL__{}", tool_call_json);
                                            pending_tokens.push_back(token);
                                        }
                                    }

                                    // Emit regular content tokens
                                    if let Some(content) = &choice.delta.content {
                                        if !content.is_empty() {
                                            return Some((
                                                Ok(content.clone()),
                                                (stream, buffer, tool_calls_acc, pending_tokens),
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
                                    (stream, buffer, tool_calls_acc, pending_tokens),
                                ));
                            }
                        }
                        Some(Err(e)) => {
                            return Some((
                                Err(ExecutionError::Interrupted(e.to_string())),
                                (stream, buffer, tool_calls_acc, pending_tokens),
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
                                    let token = format!("__TOOL_CALL__{}", tool_call_json);
                                    pending_tokens.push_back(token);
                                }
                            }
                            // If we just pushed tokens, we need another iteration to yield them
                            if !pending_tokens.is_empty() {
                                continue;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gemini_quota_error_extracts_retry_delay() {
        let body = r#"{
            "error": {
                "code": 429,
                "status": "RESOURCE_EXHAUSTED",
                "message": "Quota exceeded",
                "details": [
                    {"@type": "type.googleapis.com/google.rpc.QuotaFailure"},
                    {"@type": "type.googleapis.com/google.rpc.RetryInfo",
                     "retryDelay": "23s"}
                ]
            }
        }"#;
        let (msg, secs) = CloudProxyDriver::parse_gemini_quota_error(body)
            .expect("should recognise Gemini quota shape");
        assert_eq!(secs, 23);
        assert!(msg.contains("Gemini"));
        assert!(msg.contains("23"));
    }

    #[test]
    fn parse_gemini_quota_error_defaults_when_no_retry_info() {
        let body = r#"{
            "error": { "code": 429, "status": "RESOURCE_EXHAUSTED",
                       "message": "quota exhausted" }
        }"#;
        let (_, secs) =
            CloudProxyDriver::parse_gemini_quota_error(body).expect("recognised by status alone");
        assert_eq!(secs, 60);
    }

    #[test]
    fn parse_gemini_quota_error_returns_none_for_unrelated_body() {
        assert!(CloudProxyDriver::parse_gemini_quota_error("not json").is_none());
        assert!(CloudProxyDriver::parse_gemini_quota_error(
            r#"{"error":{"code":401,"status":"UNAUTHENTICATED"}}"#
        )
        .is_none());
    }

    #[test]
    fn parse_gemini_quota_error_handles_fractional_seconds() {
        let body = r#"{
            "error": {
                "code": 429, "status": "RESOURCE_EXHAUSTED",
                "details": [
                    {"@type": "type.googleapis.com/google.rpc.RetryInfo",
                     "retryDelay": "1.7s"}
                ]
            }
        }"#;
        let (_, secs) = CloudProxyDriver::parse_gemini_quota_error(body).unwrap();
        assert_eq!(secs, 2); // ceil(1.7) = 2
    }

    #[test]
    fn parse_groq_quota_error_extracts_tpm_retry() {
        // The exact shape from the smoke test (TPM limit on llama-3.3-70b).
        let body = r#"{"error":{"message":"Rate limit reached for model `llama-3.3-70b-versatile` in organization `org_x` service tier `on_demand` on tokens per minute (TPM): Limit 12000, Used 11336, Requested 3836. Please try again in 15.86s. Need more tokens?","type":"tokens","code":"rate_limit_exceeded"}}"#;
        let (msg, secs) = CloudProxyDriver::parse_groq_quota_error(body)
            .expect("should recognise Groq TPM rate-limit");
        assert_eq!(secs, 16); // ceil(15.86)
        assert!(msg.contains("tokens por minuto"));
    }

    #[test]
    fn parse_groq_quota_error_none_for_other_body() {
        assert!(CloudProxyDriver::parse_groq_quota_error("not json").is_none());
        assert!(CloudProxyDriver::parse_groq_quota_error(
            r#"{"error":{"message":"invalid api key","code":"invalid_api_key"}}"#
        )
        .is_none());
    }

    #[test]
    fn parse_quota_error_dispatches_to_both_providers() {
        let gemini = r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","details":[{"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"23s"}]}}"#;
        assert_eq!(CloudProxyDriver::parse_quota_error(gemini).unwrap().1, 23);
        let groq = r#"{"error":{"message":"Rate limit reached ... Please try again in 2.1s.","type":"tokens","code":"rate_limit_exceeded"}}"#;
        assert_eq!(CloudProxyDriver::parse_quota_error(groq).unwrap().1, 3);
    }

    #[test]
    fn extract_retry_after_secs_parses_hint() {
        assert_eq!(
            CloudProxyDriver::extract_retry_after_secs("Please try again in 15.86s. ok"),
            Some(16)
        );
        assert_eq!(
            CloudProxyDriver::extract_retry_after_secs("no hint here"),
            None
        );
    }
}

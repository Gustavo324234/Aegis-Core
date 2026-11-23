use super::sanitizer::{emit_stream_text_chunk, StreamTextSanitizer};
use super::*;

/// CORE-FIX (B): translate the raw `SystemError` we got from a failed provider
/// call into a one-sentence user-facing description. The chat layer uses this
/// to compose "No pude responder con X. <summary>" so the user understands
/// *why* every model failed instead of seeing silent empty output.
pub(super) fn summarise_provider_failure(err: &SystemError) -> String {
    let s = err.to_string();
    let lower = s.to_lowercase();
    if lower.contains("401") || lower.contains("unauthorized") {
        return "Las API keys configuradas no son válidas o no tienen acceso a \
                este modelo (HTTP 401)."
            .to_string();
    }
    if lower.contains("429")
        || lower.contains("resource_exhausted")
        || lower.contains("quota")
        || lower.contains("rate")
        || lower.contains("límite de tokens")
        || lower.contains("limite de tokens")
        || lower.contains("límite de solicitudes")
        || lower.contains("limite de solicitudes")
        || lower.contains("límite de cuota")
        || lower.contains("limite de cuota")
        || lower.contains("reintentá en ~")
        || lower.contains("reintenta en ~")
    {
        return "Te quedaste sin cuota gratuita o sos rate-limited (HTTP 429). \
                Esperá unos segundos o agregá billing al proveedor."
            .to_string();
    }
    if lower.contains("503")
        || lower.contains("502")
        || lower.contains("529")
        || lower.contains("overloaded")
        || lower.contains("service unavailable")
    {
        return "El proveedor está temporalmente sobrecargado o no disponible (HTTP 5xx)."
            .to_string();
    }
    if lower.contains("404") {
        return "El modelo no existe o el endpoint está mal configurado \
                (HTTP 404)."
            .to_string();
    }
    if lower.contains("400") || lower.contains("bad request") {
        return "El proveedor rechazó la solicitud (HTTP 400). Verificá que el modelo seleccionado sea compatible con chat y que la API key tenga permisos para invocarlo."
            .to_string();
    }
    if lower.contains("timeout") || lower.contains("timed out") {
        return "El proveedor no respondió a tiempo (timeout).".to_string();
    }
    if lower.contains("connection") || lower.contains("dns") {
        return "No pude alcanzar el endpoint del proveedor (problema de red).".to_string();
    }
    "El proveedor devolvió un error desconocido.".to_string()
}

/// CORE-FIX (F): is this a DETERMINISTIC failure that won't succeed on retry
/// with the same model? Two cases the smoke test exposed:
///   - HTTP 401 Unauthorized: the key isn't authorized for this model/provider.
///   - Gemini 429 with `limit: 0`: the model isn't on the key's tier at all
///     (e.g. gemini-2.5-pro on the free tier — quota is literally zero, not
///     "temporarily exhausted"). A normal 429 with retryDelay is NOT permanent.
///
/// When true, the caller trips the per-model circuit so the router stops
/// re-picking a model that can never work and rotates to one that can
/// (gemini-2.5-flash, gpt-oss:120b, …).
pub(super) fn is_permanent_model_failure(err: &SystemError) -> bool {
    let s = err.to_string();
    let lower = s.to_lowercase();
    if lower.contains("401") || lower.contains("unauthorized") {
        return true;
    }
    // HTTP 403 / subscription-gated model: the key authenticates fine but this
    // specific model isn't on its plan (e.g. Ollama free tier returns 403
    // "this model requires a subscription, upgrade for access" for the 671B
    // models, which the /models listing still advertises). Retrying never
    // helps — pin the model out so the router promotes a callable sibling.
    if lower.contains("403")
        || lower.contains("forbidden")
        || lower.contains("requires a subscription")
        || lower.contains("upgrade for access")
    {
        return true;
    }
    // Gemini tier-0: the body carries `"limit": 0` (possibly with spaces).
    // Only treat 429s this way — a 200 with limit:0 makes no sense.
    if lower.contains("429") || lower.contains("resource_exhausted") {
        let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.contains("\"limit\":0") || compact.contains("limit:0") {
            return true;
        }
    }
    // Deprecated/removed model IDs (e.g. Google AI Studio 404 "no longer available to new users")
    if lower.contains("404")
        && (lower.contains("not found")
            || lower.contains("no longer available")
            || lower.contains("not supported for generatecontent"))
    {
        return true;
    }
    false
}

/// Errors that belong to a specific model or credit quota on that model rather
/// than an outage of the entire provider (402 credits, 404 deprecated model,
/// 403 plan-gated model, 429 tier-0 limit:0). Recording these at model scope
/// avoids tripping `provider_failures` across healthy sibling models/keys.
pub(super) fn is_model_scoped_failure(err: &SystemError) -> bool {
    if is_permanent_model_failure(err) {
        let lower = err.to_string().to_lowercase();
        // Keep 401 unauthorized at key/provider scope unless explicitly per-model.
        if !(lower.contains("401") || lower.contains("unauthorized")) {
            return true;
        }
    }
    let lower = err.to_string().to_lowercase();
    lower.contains("402")
        || lower.contains("payment required")
        || lower.contains("requires more credits")
        || lower.contains("can only afford")
        || lower.contains("not a valid model")
}

/// Summarizes configured API keys cleanly for `manage_ai_provider("list")`:
/// deduplicates identical key configs into `key_count`, and filters catalog
/// previews on free-tier keys so paid models (e.g. `claude-sonnet-4-6`,
/// `openrouter/auto`) are never advertised as available on a free key.
pub(crate) fn summarize_configured_providers<I>(
    keys: I,
    catalog_models: &[crate::router::catalog::ModelEntry],
) -> Vec<serde_json::Value>
where
    I: IntoIterator<Item = crate::router::key_pool::ApiKeyEntry>,
{
    let mut summary: Vec<serde_json::Value> = Vec::new();
    let mut sig_to_idx: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for k in keys {
        let mut sorted_active = k.active_models.clone().unwrap_or_default();
        sorted_active.sort();
        let sig = format!("{}:{}:{:?}", k.provider, k.is_free_tier, sorted_active);
        if let Some(&idx) = sig_to_idx.get(&sig) {
            if let Some(obj) = summary.get_mut(idx).and_then(|v| v.as_object_mut()) {
                let next = obj.get("key_count").and_then(|c| c.as_u64()).unwrap_or(1) + 1;
                obj.insert("key_count".to_string(), serde_json::json!(next));
            }
            continue;
        }

        let mut matching_entries: Vec<&crate::router::catalog::ModelEntry> = catalog_models
            .iter()
            .filter(|m| {
                let matches_provider = if k.provider == "openrouter" {
                    m.provider == "openrouter" || (!m.is_local && m.model_id.contains('/'))
                } else {
                    m.provider.eq_ignore_ascii_case(&k.provider)
                };
                if !matches_provider {
                    return false;
                }
                if k.is_free_tier {
                    m.model_id != "openrouter/auto"
                        && (m.free_tier_eligible
                            || m.model_id.ends_with(":free")
                            || (m.cost_input_per_mtok == 0.0 && m.cost_output_per_mtok == 0.0))
                } else {
                    true
                }
            })
            .collect();

        matching_entries.sort_by(|a, b| {
            let a_or_free = a.model_id == "openrouter/free";
            let b_or_free = b.model_id == "openrouter/free";
            let a_free = a.free_tier_eligible || a.model_id.ends_with(":free");
            let b_free = b.free_tier_eligible || b.model_id.ends_with(":free");
            b_or_free
                .cmp(&a_or_free)
                .then_with(|| b_free.cmp(&a_free))
                .then_with(|| a.cost_input_per_mtok.total_cmp(&b.cost_input_per_mtok))
                .then_with(|| a.model_id.cmp(&b.model_id))
        });

        let matching_catalog: Vec<String> = matching_entries
            .into_iter()
            .map(|m| m.model_id.clone())
            .collect();
        let models_preview = match k.active_models {
            Some(ref list) if !list.is_empty() => {
                if k.is_free_tier && k.provider == "openrouter" {
                    let filtered: Vec<String> = list
                        .iter()
                        .filter(|id| {
                            let id_l = id.to_lowercase();
                            id_l == "openrouter/free" || id_l.ends_with(":free")
                        })
                        .cloned()
                        .collect();
                    if filtered.is_empty() {
                        matching_catalog.iter().take(15).cloned().collect()
                    } else {
                        filtered
                    }
                } else {
                    list.clone()
                }
            }
            _ => matching_catalog.iter().take(15).cloned().collect(),
        };

        sig_to_idx.insert(sig, summary.len());
        summary.push(serde_json::json!({
            "provider": k.provider,
            "models": models_preview,
            "catalog_total_models": matching_catalog.len(),
            "is_active": k.is_active,
            "is_free_tier": k.is_free_tier,
            "label": k.label,
            "key_count": 1u64,
        }));
    }

    summary
}

/// Returns true only for short conversational greetings/acknowledgments on a clean
/// root chat PCB so we skip injecting heavy tool schemas, VCM wrappers, and plugin docs.
pub(super) fn is_casual_chat_turn(pcb: &PCB) -> bool {
    if pcb.task_type != crate::pcb::TaskType::Chat
        || pcb.agent_id.is_some()
        || !pcb.memory_pointers.l2_context_refs.is_empty()
        || !pcb.memory_pointers.swap_refs.is_empty()
    {
        return false;
    }
    let cleaned: String = pcb
        .memory_pointers
        .l1_instruction
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| !matches!(c, '!' | '?' | '.' | ',' | '¡' | '¿'))
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || trimmed.len() > 28 {
        return false;
    }
    match trimmed {
        "hola" | "holi" | "holis" | "buenas" | "buen dia" | "buen día" | "buenos dias"
        | "buenos días" | "buenas tardes" | "buenas noches" | "hello" | "hi" | "hey"
        | "que tal" | "qué tal" | "como estas" | "cómo estás" | "todo bien" | "gracias"
        | "muchas gracias" | "chau" | "adios" | "adiós" => true,
        // Confirmations like "dale" / "ok" may approve a pending tool workflow when there is prior history.
        "ok" | "okey" | "dale" | "genial" | "perfecto" => pcb.message_history.is_empty(),
        _ => false,
    }
}

pub(super) fn collect_rendered_video_card_ids(
    tool_name: &str,
    result_json: &str,
    out: &mut Vec<String>,
) {
    if !matches!(
        tool_name,
        "video_render_mp4" | "video_produce_auto" | "video_create_project" | "video_get_overview"
    ) {
        return;
    }
    let Ok(val) = serde_json::from_str::<serde_json::Value>(result_json) else {
        return;
    };
    let mut push_if_rendered = |proj: &serde_json::Value| {
        let has_mp4 = proj
            .get("output_mp4")
            .and_then(|v| v.as_str())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if has_mp4 {
            if let Some(pid) = proj.get("id").and_then(|v| v.as_str()) {
                if !pid.trim().is_empty() && !out.iter().any(|existing| existing == pid) {
                    out.push(pid.to_string());
                }
            }
        }
    };
    if let Some(proj) = val.get("project") {
        push_if_rendered(proj);
    }
    if tool_name == "video_get_overview" {
        if let Some(arr) = val.get("projects").and_then(|v| v.as_array()) {
            // Attach only the latest rendered project to avoid flooding unrelated history.
            if let Some(first_rendered) = arr.iter().find(|p| {
                p.get("output_mp4")
                    .and_then(|v| v.as_str())
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false)
            }) {
                push_if_rendered(first_rendered);
            }
        }
    }
}

pub(super) fn format_model_error_meta_token(message: &str, retry_after_secs: u64) -> String {
    format!(
        "__MODEL_ERROR__{}",
        serde_json::json!({
            "code": "models_unavailable",
            "message": message,
            "retry_after_secs": retry_after_secs,
        })
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn consume_recovery_stream<S>(
    raw_stream: S,
    text_tx: &tokio::sync::mpsc::UnboundedSender<Result<String, ExecutionError>>,
    new_model_id: &str,
    new_provider: &str,
    warn_category: &str,
    warn_message: String,
    emit_model_event: bool,
    sanitizer: &mut StreamTextSanitizer,
    has_streamed_text: &mut bool,
    last_streamed_ends_newline: &mut bool,
    needs_paragraph_break: &mut bool,
    pid: &str,
    log_label: &str,
) -> (String, Vec<ToolCallRecord>)
where
    S: futures::Stream<Item = Result<String, ExecutionError>>,
{
    tokio::pin!(raw_stream);
    let mut rec_text = String::new();
    let mut rec_tool_calls: Vec<ToolCallRecord> = Vec::new();
    let mut announced = false;

    let announce = |announced: &mut bool| {
        if !*announced {
            *announced = true;
            if emit_model_event {
                let payload = serde_json::json!({
                    "model_id": new_model_id,
                    "provider": new_provider,
                });
                let _ = text_tx.send(Ok(format!("__MODEL_SELECTED__{}", payload)));
            }
            let wpayload = serde_json::json!({
                "category": warn_category,
                "provider": new_provider,
                "message": warn_message,
            });
            let _ = text_tx.send(Ok(format!("__WARNING__{}", wpayload)));
        }
    };

    while let Some(tr) = raw_stream.next().await {
        match tr {
            Ok(token) if token.starts_with("__TOOL_CALL__") => {
                announce(&mut announced);
                let _ = text_tx.send(Ok(token.clone()));
                let json_str = token.strip_prefix("__TOOL_CALL__").unwrap_or_default();
                if let Ok(tc) = serde_json::from_str::<DriverToolCallPayload>(json_str) {
                    rec_tool_calls.push(ToolCallRecord {
                        id: tc.id,
                        type_: "function".to_string(),
                        function: FunctionCallRecord {
                            name: tc.name,
                            arguments: tc.arguments.to_string(),
                        },
                    });
                }
            }
            Ok(text) => {
                let clean = sanitizer.push(&text);
                if !clean.is_empty() {
                    announce(&mut announced);
                    emit_stream_text_chunk(
                        text_tx,
                        clean,
                        true,
                        &mut rec_text,
                        has_streamed_text,
                        last_streamed_ends_newline,
                        needs_paragraph_break,
                    );
                }
            }
            Err(e) => {
                warn!(
                    pid = %pid,
                    model = %new_model_id,
                    provider = %new_provider,
                    error = %e,
                    "{} stream error during recovery",
                    log_label
                );
                break;
            }
        }
    }

    let tail = sanitizer.flush();
    if !tail.is_empty() {
        announce(&mut announced);
        emit_stream_text_chunk(
            text_tx,
            tail,
            true,
            &mut rec_text,
            has_streamed_text,
            last_streamed_ends_newline,
            needs_paragraph_break,
        );
    }

    (rec_text, rec_tool_calls)
}

#[cfg(test)]
mod summarise_tests {
    use super::*;

    #[test]
    fn detects_401() {
        let e =
            SystemError::HardwareFailure("Cloud API Error 401 Unauthorized: unauthorized".into());
        assert!(summarise_provider_failure(&e).contains("401"));
    }

    #[test]
    fn detects_429_and_quota() {
        for body in [
            "API Error 429 Too Many Requests",
            "RESOURCE_EXHAUSTED",
            "rate limit exceeded",
            "quota exhausted",
            "Alcanzaste el límite de tokens por minuto (TPM) del modelo. Reintentá en ~17s.",
            "Alcanzaste el límite de solicitudes (RPM) de tu plan. Reintentá en ~12s.",
        ] {
            let e = SystemError::HardwareFailure(body.into());
            assert!(
                summarise_provider_failure(&e).contains("429"),
                "expected 429 summary for {:?}",
                body
            );
        }
    }

    #[test]
    fn falls_back_to_unknown() {
        let e = SystemError::HardwareFailure("something weird happened".into());
        assert!(summarise_provider_failure(&e).contains("desconocido"));
    }

    #[test]
    fn permanent_failure_detects_401() {
        let e =
            SystemError::HardwareFailure("Cloud API Error 401 Unauthorized: unauthorized".into());
        assert!(is_permanent_model_failure(&e));
    }

    #[test]
    fn permanent_failure_detects_403_subscription() {
        // Ollama free tier serving a 671B model: key is valid but the model
        // is subscription-gated. Must be treated as permanent, not retried.
        let e = SystemError::HardwareFailure(
            "Cloud API Error 403 Forbidden: {\"error\":\"this model requires a \
             subscription, upgrade for access: https://ollama.com/upgrade\"}"
                .into(),
        );
        assert!(is_permanent_model_failure(&e));
    }

    #[test]
    fn permanent_failure_detects_gemini_tier_zero() {
        // The real shape from the smoke test: 429 RESOURCE_EXHAUSTED with limit: 0.
        let e = SystemError::HardwareFailure(
            "API Error 429 Too Many Requests: { \"error\": { \"code\": 429, \
             \"message\": \"Quota exceeded ... limit: 0, model: gemini-2.5-pro\", \
             \"status\": \"RESOURCE_EXHAUSTED\" } }"
                .into(),
        );
        assert!(is_permanent_model_failure(&e));
    }

    #[test]
    fn permanent_failure_ignores_transient_429() {
        // A normal rate limit (limit > 0, has retryDelay) is NOT permanent —
        // the model works, it's just throttled. Must not trip the model circuit.
        let e = SystemError::HardwareFailure(
            "API Error 429 Too Many Requests: Rate limit reached ... \
             Limit 12000, Used 10797. Please try again in 11.725s"
                .into(),
        );
        assert!(!is_permanent_model_failure(&e));
    }

    #[test]
    fn permanent_failure_ignores_plain_500() {
        let e = SystemError::HardwareFailure("API Error 500 Internal Server Error".into());
        assert!(!is_permanent_model_failure(&e));
    }
}

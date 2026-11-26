# CORE-361: Native Provider Endpoints, Universal OpenRouter Hub Fallback, and Base URL Normalization in CognitiveRouter

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Asignado a:** Kernel Engineer

## Description
1. `entry_api_url` in `ank-core/src/router/mod.rs` hardcoded `"anthropic" | "deepseek" | "mistral" | "qwen"` to `https://openrouter.ai/api/v1/chat/completions` and lacked `"xai"`, causing native API keys for those providers to be sent to OpenRouter (`401 Unauthorized`).
2. Conversely, when an operator registered an `openrouter` key (`provider: "openrouter"`), `KeyPool` only matched catalog entries whose `provider == "openrouter"`, preventing `openrouter` keys from acting as a universal hub fallback for `anthropic/*`, `openai/*`, `deepseek/*`, `mistralai/*`, `qwen/*`, or `x-ai/*` catalog models.
3. `normalize_chat_api_url` only rewrote `/api/chat` on Ollama, leaving base URLs like `https://openrouter.ai/api/v1`, `https://api.openai.com/v1`, or `http://host:11434` without `/chat/completions` (`404/405`).

## Acceptance Criteria
- [x] Route native keys for `anthropic`, `deepseek`, `mistral`, `xai`, and `qwen` to their official OpenAI-compatible `/chat/completions` endpoints.
- [x] Allow cloud `org/model` catalog entries to fall back transparently to an available `openrouter` key when no native provider key exists for the tenant, adjusting effective `provider`, `api_url`, and preserving full `org/model` in `bare_model_id`.
- [x] Extend `normalize_chat_api_url` to complete base URLs (`/v1`, `/v1beta/openai`, `/compatible-mode/v1`, and bare Ollama hosts) to their OpenAI-compatible `/chat/completions` path.

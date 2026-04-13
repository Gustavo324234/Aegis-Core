# CORE-084 — Fix: `models.yaml` tiene providers (`anthropic`, `google`, `deepseek`, `mistral`, `qwen`) sin soporte en el driver

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Kernel Engineer
**Prioridad:** 🟠 ALTA
**Estado:** TODO

---

## Contexto

`kernel/crates/ank-core/src/router/models.yaml` lista modelos de 9 providers:
`anthropic`, `openai`, `google`, `groq`, `deepseek`, `mistral`, `qwen`,
`openrouter`, `ollama`.

La función `entry_api_url()` en `router/mod.rs` solo tiene URLs para 6:

```rust
fn entry_api_url(entry: &ModelEntry) -> String {
    match entry.provider.as_str() {
        "anthropic" => "https://api.anthropic.com/v1/messages",  // ← protocolo incompatible (CORE-081)
        "openai"    => "https://api.openai.com/v1/chat/completions",
        "groq"      => "https://api.groq.com/openai/v1/chat/completions",
        "google"    => "https://generativelanguage.googleapis.com/v1beta/openai/...",
        "openrouter" => "https://openrouter.ai/api/v1/chat/completions",
        "ollama"    => "http://localhost:11434/v1/chat/completions",
        _           => "https://openrouter.ai/api/v1/chat/completions", // fallback
    }
}
```

**Providers en `models.yaml` sin URL explícita (usan el fallback OpenRouter):**

| Provider | Modelo en yaml | Problema |
|----------|---------------|---------|
| `deepseek` | `deepseek/deepseek-r1` | Fallback a OpenRouter — puede funcionar si se tiene key de OpenRouter |
| `mistral` | `mistralai/mistral-large` | Fallback a OpenRouter — mismo caso |
| `qwen` | `qwen/qwen-2.5-72b-instruct` | Fallback a OpenRouter — mismo caso |

**Problema adicional:** La UI (`ProvidersTab`) permite al usuario agregar
un provider `"google"` con su API key de Google directamente. El `CognitiveRouter`
intentará usar `entry_api_url("google")` = URL de Google. Pero `CloudProxyDriver`
usaría headers OpenAI contra la API de Google — que sí tiene compatibilidad
OpenAI via `generativelanguage.googleapis.com/v1beta/openai/...`, pero requiere
`Authorization: Bearer` con la API key de Google, no `x-goog-api-key`. Esto
puede funcionar según el endpoint exacto.

**Acción concreta de este ticket:** Agregar URLs explícitas para los providers
que están en `models.yaml` y alinear con la realidad de compatibilidad OpenAI.

---

## Cambios requeridos

**Archivo:** `kernel/crates/ank-core/src/router/mod.rs`

Actualizar `entry_api_url` con URLs correctas y comentarios de compatibilidad:

```rust
fn entry_api_url(entry: &ModelEntry) -> String {
    match entry.provider.as_str() {
        // Compatible OpenAI — requiere key propia
        "openai"    => "https://api.openai.com/v1/chat/completions",
        "groq"      => "https://api.groq.com/openai/v1/chat/completions",
        "ollama"    => "http://localhost:11434/v1/chat/completions",
        // Compatible OpenAI via OpenRouter — requiere key de OpenRouter
        "anthropic" | "deepseek" | "mistral" | "qwen"
                    => "https://openrouter.ai/api/v1/chat/completions",
        // Google: compatible OpenAI via endpoint beta
        "google"    => "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
        // OpenRouter: hub universal
        "openrouter" => "https://openrouter.ai/api/v1/chat/completions",
        // Fallback seguro
        _           => "https://openrouter.ai/api/v1/chat/completions",
    }
}
```

**Archivo:** `AEGIS_CONTEXT.md`

Actualizar `LIM-005` (creado en CORE-081):
```
LIM-005 | ank-core | Anthropic, DeepSeek, Mistral, Qwen se acceden via OpenRouter — key de OpenRouter requerida para estos providers
```

---

## Criterios de aceptación

- [ ] `entry_api_url` tiene una rama explícita para cada provider en `models.yaml`
- [ ] Ningún provider cae en el `_` wildcard sin que sea intencional
- [ ] `AEGIS_CONTEXT.md` lista `LIM-005` con los providers afectados
- [ ] `cargo build` pasa sin errores

---

## Dependencias

CORE-081 — alineado con la decisión de usar OpenRouter como proxy para Anthropic.
Implementar después de CORE-081 o en el mismo commit.

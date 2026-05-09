# BRIEFING — Kernel Engineer
## EPIC 51 / CORE-292: provider `ollama_cloud`
**Fecha:** 2026-05-08
**Branch sugerido:** `feat/core-292-ollama-cloud-provider`

---

## Contexto

Ollama lanzó una API Cloud directa: `https://ollama.com/api/chat` con auth via
`Authorization: Bearer OLLAMA_API_KEY`. Queremos que Aegis lo soporte como un
provider más, igual que OpenAI o Groq.

El kernel ya soporta Ollama local (`http://localhost:11434`). Este ticket agrega
el provider remoto `ollama_cloud` con 3 cambios quirúrgicos.

---

## Cambios requeridos

### 1. `kernel/crates/ank-core/src/router/mod.rs`

En la función `entry_api_url`, agregar una rama para el nuevo provider **antes** del fallback:

```rust
"ollama_cloud" => "https://ollama.com/api/chat".to_string(),
```

### 2. `kernel/crates/ank-http/src/routes/providers.rs`

**2a. Allowlist SSRF** — agregar `"ollama.com"` al array `ALLOWED_API_HOSTS`:

```rust
const ALLOWED_API_HOSTS: &[&str] = &[
    "api.openai.com",
    "api.anthropic.com",
    "api.groq.com",
    "openrouter.ai",
    "generativelanguage.googleapis.com",
    "api.together.xyz",
    "localhost",
    "127.0.0.1",
    "ollama.com",   // ← nuevo
];
```

**2b. Handler `list_provider_models`** — agregar rama `"ollama_cloud"` en el match:

```rust
"ollama_cloud" => {
    let client = reqwest::Client::new();
    let res = client
        .get("https://ollama.com/api/tags")
        .header("Authorization", format!("Bearer {}", req.api_key))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    let data: Value = res
        .json()
        .await
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;
    if let Some(list) = data.get("models").and_then(|m| m.as_array()) {
        for m in list {
            if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                models.push(name.to_string());
            }
        }
    }
}
```

### 3. `kernel/crates/ank-core/src/agents/tool_registry.rs`

**3a.** Agregar variante al enum `ProviderKind`:

```rust
OllamaCloud,
```

**3b.** Agregar arm en `from_string`:

```rust
"ollama_cloud" => Self::OllamaCloud,
```

---

## Criterios de aceptación (CI los valida)

- `cargo build --workspace` pasa sin errores ni warnings nuevos
- `entry_api_url("ollama_cloud")` devuelve `"https://ollama.com/api/chat"`
- `validate_api_url("https://ollama.com/api/chat")` no retorna error
- `ProviderKind::from_string("ollama_cloud")` devuelve `OllamaCloud`

## Commit message

```
feat(ank-core): CORE-292 add ollama_cloud provider with remote API support
```

---

**No correr tests manualmente. No pushear a main. Abrir PR hacia main.**

# CORE-292 — Kernel: provider `ollama_cloud` — URL remota + allowlist SSRF

## Metadata
- **Epic:** 51 — Model Intelligence
- **Tipo:** feat
- **Prioridad:** Alta
- **Asignado a:** Kernel Engineer
- **Estado:✅ Done

## Contexto

Ollama ofrece dos modos de acceso cloud (https://docs.ollama.com/cloud):

**Modo A — Cloud offload local:** el usuario corre `ollama` localmente, los modelos
con sufijo `-cloud` se ejecutan en servidores de Ollama transparentemente. El
endpoint sigue siendo `http://localhost:11434`. Aegis ya lo soporta sin cambios.

**Modo B — Cloud API directa:** Ollama.com actúa como provider remoto con endpoint
`https://ollama.com/api/chat` y autenticación via `Authorization: Bearer OLLAMA_API_KEY`.
Este modo permite usar modelos Ollama grandes sin tener Ollama instalado localmente.

Este ticket implementa el soporte para **Modo B**.

## Cambios requeridos

### 1. `kernel/crates/ank-core/src/router/mod.rs` — función `entry_api_url`

Agregar rama para el nuevo provider:

```rust
"ollama_cloud" => "https://ollama.com/api/chat".to_string(),
```

### 2. `kernel/crates/ank-http/src/routes/providers.rs` — SSRF allowlist

Agregar `"ollama.com"` al array `ALLOWED_API_HOSTS`:

```rust
const ALLOWED_API_HOSTS: &[&str] = &[
    // ... existentes ...
    "ollama.com",
];
```

### 3. `kernel/crates/ank-http/src/routes/providers.rs` — handler `list_provider_models`

Agregar rama `"ollama_cloud"` que consulta `https://ollama.com/api/tags` con el
API key del usuario (mismo formato que el endpoint local de Ollama):

```rust
"ollama_cloud" => {
    validate_api_url("https://ollama.com/api/tags")?;
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

### 4. `kernel/crates/ank-core/src/agents/tool_registry.rs` — enum `ProviderKind`

Agregar variante:

```rust
OllamaCloud,
```

Y en `from_string`:

```rust
"ollama_cloud" => Self::OllamaCloud,
```

## Notas de implementación

- El provider `ollama_cloud` es `is_local: false` — necesita API key registrada en KeyPool.
- El formato de API key de Ollama Cloud es `Bearer <token>` igual que OpenAI.
- La URL de chat es `https://ollama.com/api/chat` (no `/v1/chat/completions` — usa
  el protocolo nativo de Ollama, compatible con OpenAI via parámetro `stream`).

## Criterios de aceptación

- [ ] `cargo build --workspace` pasa sin errores ni warnings nuevos
- [ ] `entry_api_url("ollama_cloud")` devuelve `"https://ollama.com/api/chat"`
- [ ] `validate_api_url("https://ollama.com/api/chat")` no retorna error
- [ ] `ProviderKind::from_string("ollama_cloud")` devuelve `OllamaCloud`
- [ ] El handler `list_provider_models` para `"ollama_cloud"` consulta el endpoint remoto con Authorization header

## Dependencias

- Ninguna (cambio aislado en kernel)

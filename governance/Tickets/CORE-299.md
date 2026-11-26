# CORE-299 — Kernel: soporte de `model_override` en WebSocket chat

## Metadata
- **Epic:** 51 — Model Intelligence
- **Tipo:** feat
- **Prioridad:** Crítica
- **Asignado a:** Kernel Engineer
- **Estado:✅ Done

## Contexto

El WebSocket de chat recibe `{ prompt, task_type }` y el CMR elige el modelo
automáticamente. El usuario no puede elegir el modelo que quiere usar.

Este ticket agrega soporte para un campo opcional `model_override` en el payload
del WebSocket. Si está presente, el CMR lo usa directamente sin pasar por el
algoritmo de scoring.

## Cambios requeridos

### 1. `kernel/crates/ank-http/src/ws/chat.rs` — parseo del mensaje

Agregar `model_override` al struct de entrada:

```rust
#[derive(Deserialize)]
struct ChatMessage {
    prompt: String,
    #[serde(default)]
    task_type: Option<String>,
    #[serde(default)]
    model_override: Option<String>,  // ← nuevo
}
```

Al construir el PCB, setear el override si viene:

```rust
if let Some(model_id) = msg.model_override {
    pcb.model_override = Some(model_id);
}
```

### 2. `kernel/crates/ank-core/src/pcb.rs` — campo en PCB

Agregar campo opcional:

```rust
pub model_override: Option<String>,
```

Inicializar en `PCB::new()` como `None`.

### 3. `kernel/crates/ank-core/src/router/mod.rs` — CognitiveRouter::decide()

Al inicio de `decide()`, antes de `get_candidates`, verificar el override:

```rust
// Si el PCB tiene un model_override, bypassear el CMR y resolver directo
if let Some(ref model_id) = pcb.model_override {
    if let Some(entry) = self.catalog.find(model_id).await {
        let key = self.resolve_key(&entry, tenant_id).await.ok_or_else(|| {
            SystemError::HardwareFailure(format!(
                "No key available for model_override '{}'", model_id
            ))
        })?;
        return Ok(RoutingDecision {
            model_id: bare_model_id(&entry.model_id, &entry.provider),
            provider: entry.provider.clone(),
            api_url: key.api_url.clone().unwrap_or_else(|| entry_api_url(&entry)),
            api_key: key.api_key.clone(),
            key_id: Some(key.key_id.clone()),
            fallback_chain: vec![],
        });
    }
    // Si el model_id no está en el catálogo, continuar con CMR normal
    warn!("model_override '{}' not found in catalog, falling back to CMR", model_id);
}
```

### 4. `kernel/crates/ank-http/src/routes/chat.rs` (si existe endpoint REST)

Si hay un endpoint REST de chat además del WebSocket, aplicar el mismo cambio.

## Criterios de aceptación

- [ ] `cargo build --workspace` pasa
- [ ] WebSocket payload `{"prompt": "hola", "model_override": "nvidia/nemotron-3-super-120b-a12b:free"}` usa ese modelo
- [ ] Si `model_override` no está en el catálogo, el CMR elige normalmente (no error)
- [ ] `lastRoutingInfo` en el frontend refleja el modelo elegido por override
- [ ] Sin `model_override`, el comportamiento del CMR es idéntico al actual

## Dependencias

- Ninguna

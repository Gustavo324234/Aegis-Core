# BRIEFING — Kernel Engineer
## CORE-299: `model_override` en WebSocket chat
**Fecha:** 2026-05-13
**Branch:** `feat/core-299-model-override`

## Contexto

El usuario quiere elegir el modelo desde el chat. La Shell va a mandar
`model_override: "model_id"` en el payload del WebSocket. El kernel
tiene que respetarlo y bypassear el CMR.

## Cambios — 3 archivos

### 1. `kernel/crates/ank-core/src/pcb.rs`

Agregar campo al struct PCB:
```rust
pub model_override: Option<String>,
```
Inicializar en `PCB::new()` como `None`.

### 2. `kernel/crates/ank-http/src/ws/chat.rs`

En el struct de parseo del mensaje WS agregar:
```rust
#[serde(default)]
model_override: Option<String>,
```
Al construir el PCB:
```rust
pcb.model_override = msg.model_override;
```

### 3. `kernel/crates/ank-core/src/router/mod.rs`

Al inicio de `CognitiveRouter::decide()`, ANTES de `get_candidates`:
```rust
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
    warn!("model_override '{}' not found in catalog, falling back to CMR", model_id);
}
```

## Criterios de aceptación

- [ ] `cargo build --workspace` pasa
- [ ] Payload `{"prompt":"hola","model_override":"nvidia/nemotron-3-super-120b-a12b:free"}` usa ese modelo
- [ ] Sin `model_override`, el CMR se comporta igual que antes

## Commit
```
feat(ank-core): CORE-299 add model_override to PCB and WebSocket chat handler
```

**No correr tests. No pushear a main. Abrir PR.**

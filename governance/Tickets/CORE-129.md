# CORE-129 — Feature: Persona storage en enclave SQLCipher + endpoints HTTP

**Epic:** 38 — Agent Persona System
**Repo:** Aegis-Core — `kernel/`
**Crates:** `ank-core`, `ank-http`
**Tipo:** feat
**Prioridad:** Alta
**Asignado a:** Kernel Engineer
**Depende de:** CORE-128 (firma `build_prompt` con `Option<&str>`)

---

## Contexto

La Persona es el system prompt configurable por tenant que define la identidad
del agente desplegado por el operador. Debe persistir en el enclave SQLCipher
de cada tenant (clave `"agent_persona"` en `kv_store`) y ser inyectada en cada
inferencia via `build_prompt()`.

**ADR-039:** La Persona se almacena en el enclave del tenant. No requiere tabla nueva —
usa `set_kv` / `get_kv` del `TenantDB` existente. Máximo 4000 caracteres. Vacía por defecto.

---

## Cambios requeridos

### 1. `ank-core` — Métodos de Persona en `TenantDB`

Agregar en `kernel/crates/ank-core/src/enclave/mod.rs`:

```rust
/// Clave bajo la cual se persiste la persona del agente en kv_store.
const PERSONA_KEY: &str = "agent_persona";
/// Longitud máxima permitida para una Persona (caracteres UTF-8).
const PERSONA_MAX_LEN: usize = 4000;

impl TenantDB {
    /// Guarda o actualiza la persona del agente para este tenant.
    /// Retorna error si `persona` supera `PERSONA_MAX_LEN`.
    pub fn set_persona(&self, persona: &str) -> Result<()> {
        anyhow::ensure!(
            persona.len() <= PERSONA_MAX_LEN,
            "Persona exceeds maximum length of {} characters", PERSONA_MAX_LEN
        );
        self.set_kv(PERSONA_KEY, persona)
    }

    /// Recupera la persona del agente. Retorna `None` si no está configurada.
    pub fn get_persona(&self) -> Result<Option<String>> {
        self.get_kv(PERSONA_KEY)
    }

    /// Elimina la persona del agente, restaurando el comportamiento por defecto.
    pub fn delete_persona(&self) -> Result<()> {
        use anyhow::Context;
        self.connection
            .execute("DELETE FROM kv_store WHERE key = ?1", [PERSONA_KEY])
            .context("Failed to delete agent persona")?;
        Ok(())
    }
}
```

### 2. `ank-core` — Inyección de Persona en `CognitiveHAL`

En `kernel/crates/ank-core/src/chal/mod.rs`, modificar `route_and_execute()` y
`execute_with_decision()` para aceptar y pasar la persona.

Modificar la firma de `route_and_execute`:

```rust
pub async fn route_and_execute(
    &self,
    shared_pcb: SharedPCB,
    persona: Option<String>,
) -> Result<Pin<Box<dyn Stream<Item = Result<String, ExecutionError>> + Send>>, SystemError>
```

Modificar la firma de `execute_with_decision`:

```rust
async fn execute_with_decision(
    &self,
    decision: RoutingDecision,
    instruction: &str,
    pid: &str,
    persona: Option<&str>,
) -> Result<...>
```

En todos los puntos donde se llama `self.build_prompt(...)`, pasar `persona.as_deref()`.

### 3. `ank-http` — Leer Persona al recibir WebSocket

En `kernel/crates/ank-http/src/ws/chat.rs`, antes de llamar a `hal.route_and_execute()`,
intentar leer la persona del enclave del tenant:

```rust
// Leer Persona del enclave (best-effort — si falla, continuar sin persona)
let persona: Option<String> = {
    let session_key_hash = sha256_hex(&session_key);
    match ank_core::enclave::TenantDB::open(&tenant_id, &session_key_hash) {
        Ok(db) => db.get_persona().unwrap_or(None),
        Err(_) => None,
    }
};
// Pasar al HAL
hal.route_and_execute(shared_pcb, persona).await
```

> **Nota de implementación:** `TenantDB::open` requiere el hash SHA-256 de la passphrase,
> no la passphrase en texto plano. En el WebSocket handler, la `session_key` ya viene
> como texto plano desde el subprotocol — aplicar `sha256_hex` antes de abrir el enclave.
> Verificar que el helper `sha256_hex` ya existe en `ank-http` o reutilizarlo de `citadel.rs`.

### 4. `ank-http` — Nuevos endpoints REST de Persona

Crear `kernel/crates/ank-http/src/routes/persona_api.rs` con tres handlers:

#### `GET /api/persona`
Auth: `CitadelAuthenticated` (tenant_id + session_key via headers)

```rust
// Retorna la persona configurada o indicador de vacía
{ "persona": "...", "is_configured": true }
// Si no hay persona:
{ "persona": "", "is_configured": false }
```

#### `POST /api/persona`
Auth: `CitadelAuthenticated`
Body: `{ "persona": "Eres Eve, asistente de ACME Corp..." }`

```rust
// Éxito
{ "success": true }
// Error validación
{ "error": "Persona exceeds maximum length of 4000 characters" }
```

#### `DELETE /api/persona`
Auth: `CitadelAuthenticated`

```rust
{ "success": true }
```

Registrar el nuevo módulo en `routes/mod.rs`:

```rust
pub mod persona_api;
// En build_router():
.nest("/api/persona", persona_api::router())
```

### 5. Tests en `ank-core`

```rust
#[test]
fn test_persona_set_get_delete() -> anyhow::Result<()> {
    // Usa tempdir para no escribir en ./users/
    // Verificar: set → get devuelve valor, delete → get devuelve None
    // Verificar: persona de 4001 chars retorna error
}
```

---

## Criterios de aceptación

- [ ] `cargo build --workspace` sin errores ni warnings Clippy
- [ ] `cargo test -p ank-core` pasa — test de `set/get/delete` persona
- [ ] `GET /api/persona` retorna `is_configured: false` para tenant sin persona
- [ ] `POST /api/persona` con texto válido persiste y `GET` devuelve el mismo texto
- [ ] `POST /api/persona` con más de 4000 caracteres retorna error HTTP 400
- [ ] `DELETE /api/persona` limpia y `GET` vuelve a `is_configured: false`
- [ ] Chat WebSocket usa la persona persistida — el modelo se presenta con la identidad configurada
- [ ] Si el enclave no se puede abrir (best-effort), el chat continúa sin persona (no crashea)

---

## Dependencias

- CORE-128 (firma `build_prompt` con `Option<&str>`) — debe estar mergeado primero

## Tickets que desbloquea

- CORE-130 (UI de edición de Persona en React)

---

## Commit message

```
feat(ank-core,ank-http): CORE-129 agent persona — SQLCipher storage + REST endpoints
```

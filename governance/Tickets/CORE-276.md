# CORE-276 — Specialist: aprobación de paths externos por el usuario

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** EPIC 50 — Agent Inbox  
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/enclave/` (approved_paths en TenantDB)
- `kernel/crates/ank-core/src/chal/mod.rs` (`get_approved_paths` + flujo de aprobación)
- `kernel/crates/ank-http/src/routes/` (endpoint para aprobación desde shell)

---

## Contexto

CORE-275 implementa filesystem tools para specialists. Cuando un specialist
intenta acceder a un path fuera de su workspace (ej: `/home/tavo/Aegis-Core`),
recibe `path_requires_approval` y reporta el bloqueo a su supervisor.

Este ticket implementa el mecanismo por el cual el usuario puede aprobar ese
acceso — una sola vez, persistido en el enclave del tenant, disponible en
sesiones futuras.

---

## Flujo completo

```
Specialist intenta leer /home/tavo/Aegis-Core/src/main.rs
    ↓
resolve_path() → path fuera del workspace, no aprobado
    ↓
retorna { "error": "path_requires_approval", "path": "/home/tavo/Aegis-Core" }
    ↓
Specialist reporta el bloqueo a su Supervisor
    ↓
Supervisor invoca ask_user(
    question: "El specialist necesita acceder a /home/tavo/Aegis-Core. ¿Lo autorizás?",
    context: "El proyecto está clonado ahí y el specialist necesita leerlo."
)
    ↓
Usuario responde "sí" en el chat o en el hilo dedicado
    ↓
Chat Agent / CORE-271 entrega la respuesta al supervisor
    ↓
Supervisor detecta respuesta afirmativa → invoca approve_path("/home/tavo/Aegis-Core")
    ↓
Kernel persiste la aprobación en el enclave del tenant
    ↓
Supervisor re-despacha la tarea al specialist
    ↓
Specialist reintenta → resolve_path() verifica enclave → acceso permitido
```

---

## Cambios requeridos

### 1. `TenantDB` — persistir paths aprobados

En el enclave del tenant, agregar dos operaciones:

```rust
/// Retorna la lista de paths externos aprobados por el usuario.
pub fn get_approved_paths(&self) -> anyhow::Result<Vec<String>>;

/// Agrega un path a la lista de aprobados. Idempotente.
pub fn add_approved_path(&self, path: &str) -> anyhow::Result<()>;

/// Revoca la aprobación de un path.
pub fn remove_approved_path(&self, path: &str) -> anyhow::Result<()>;
```

Almacenar como JSON en la tabla de configuración del enclave (misma tabla
que usa `set_persona`, `set_onboarding_step`, etc.).

Clave: `"approved_paths"`, valor: `["path1", "path2"]`.

### 2. `chal/mod.rs` — completar `get_approved_paths`

Reemplazar el stub de CORE-275 con la implementación real:

```rust
async fn get_approved_paths(pcb: &PCB) -> Vec<String> {
    let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
    let session_key = pcb.session_key.as_deref().unwrap_or("");

    match ank_core::enclave::TenantDB::open(tenant_id, session_key) {
        Ok(db) => db.get_approved_paths().unwrap_or_default(),
        Err(_) => vec![],
    }
}
```

### 3. `tool_registry.rs` — herramienta `approve_path` para Supervisors

Los supervisores (no los specialists) pueden aprobar paths después de recibir
el OK del usuario via `ask_user`:

```rust
fn approve_path() -> ToolDefinition {
    ToolDefinition {
        name: "approve_path",
        description: "Approve an external path for filesystem access by specialists. Only call this after the user has explicitly authorized access to that path via ask_user.",
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to approve."
                }
            },
            "required": ["path"]
        }),
    }
}
```

Agregar a `ProjectSupervisor` y `Supervisor`:
```rust
AgentRole::ProjectSupervisor { .. } | AgentRole::Supervisor { .. } => {
    vec![
        Self::spawn_agent(),
        Self::query_agent(),
        Self::report(),
        Self::ask_user(),
        Self::approve_path(),   // nuevo
    ]
}
```

### 4. `chal/mod.rs` — arm `approve_path`

```rust
"approve_path" => {
    let path = args["path"].as_str().unwrap_or("").to_string();

    if path.is_empty() {
        return "{\"error\":\"empty_path\"}".to_string();
    }

    // Verificar que el path existe en el servidor
    if !std::path::Path::new(&path).exists() {
        return format!(
            "{{\"error\":\"path_not_found\",\"path\":\"{}\"}}",
            path
        );
    }

    let tenant_id = match &pcb.tenant_id {
        Some(t) => t.clone(),
        None => return "{\"error\":\"no_tenant_id\"}".to_string(),
    };
    let session_key = pcb.session_key.as_deref().unwrap_or("");

    match ank_core::enclave::TenantDB::open(&tenant_id, session_key) {
        Ok(db) => match db.add_approved_path(&path) {
            Ok(_) => serde_json::json!({
                "status": "approved",
                "path": path
            }).to_string(),
            Err(e) => format!("{{\"error\":\"persist_failed\",\"detail\":\"{}\"}}", e),
        },
        Err(e) => format!("{{\"error\":\"enclave_open_failed\",\"detail\":\"{}\"}}", e),
    }
}
```

### 5. Endpoint para revocar aprobaciones desde la shell (opcional en v1)

```
DELETE /api/tenant/approved-paths
Body: { "path": "/home/tavo/Aegis-Core" }
```

Permite al usuario revocar acceso desde la UI sin necesidad de un agente.
Implementar solo si hay tiempo — no es bloqueante para el launch.

---

## Criterios de aceptación

- [ ] `TenantDB` tiene `get_approved_paths`, `add_approved_path`, `remove_approved_path`
- [ ] `get_approved_paths` en `chal/mod.rs` lee del enclave real (no retorna `[]`)
- [ ] Supervisor puede invocar `approve_path("/some/external/path")`
- [ ] Después de `approve_path`, el specialist puede acceder al path sin error
- [ ] La aprobación persiste entre sesiones (sobrevive a restart del kernel)
- [ ] `approve_path` con path inexistente retorna error — no aprueba fantasmas
- [ ] `cargo build --workspace` sin errores

## Dependencias

- CORE-275 (filesystem tools — este ticket completa el stub `get_approved_paths`)
- CORE-263 ✅ (ask_user — el supervisor ya puede preguntar al usuario)

---

## Resolución (2026-06-12 — post-auditoría CORE-313)
Verificado ya implementado en main (falso negativo de la primera pasada de la auditoría, que buscó en syscalls/vcm cuando el flujo vive en chal+enclave). Evidencia: TenantDB get/add/remove_approved_paths (enclave/mod.rs:559-591), get_approved_paths real en chal/mod.rs:1778, arm approve_path con validación de existencia (chal/mod.rs:2124-2156), tool en registry para supervisores (tool_registry.rs:94/357) con test de regresión (':502 supervisores ahora tienen 6 tools'). El endpoint DELETE de revocación era explícitamente opcional y no bloqueante — queda fuera del scope cerrado.

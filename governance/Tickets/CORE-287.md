# CORE-287 — Kernel: fix project_id usa scope en lugar de nombre

**Tipo:** fix  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/agents/orchestrator.rs` (método `create_project`)
**Estado:** ✅ Completo

---

## Contexto

El método `create_project` tiene los argumentos invertidos:

```rust
pub async fn create_project(
    &self,
    name: String,    // "Aegis-Core"
    scope: String,   // "user wants to work on the Aegis project"
    _task_type: Option<TaskType>,
    tenant_id: Option<String>,
) -> anyhow::Result<AgentId> {
    self.activate_project(
        tenant_id.as_deref().unwrap_or("default"),
        &scope,   // ← BUG: scope como project_id
        &name,    // ← BUG: name como description
    ).await
}
```

El `project_id` debería ser el `name` ("Aegis-Core") — que es estable y corto.
Usando el `scope` como `project_id`, el path en filesystem es:

```
users/Tavo/projects/user wants to work on the Aegis project/
```

Eso tiene espacios, caracteres especiales, y longitud variable. Dos projectos
con el mismo nombre pero diferente scope generan project_ids distintos — el
sistema no los reconoce como el mismo proyecto al restaurar.

---

## Cambios requeridos

### `orchestrator.rs` — `create_project`

```rust
pub async fn create_project(
    &self,
    name: String,
    scope: String,
    _task_type: Option<crate::pcb::TaskType>,
    tenant_id: Option<String>,
) -> anyhow::Result<crate::agents::node::AgentId> {
    // CORE-287: project_id es el nombre sanitizado, no el scope
    let project_id = sanitize_project_id(&name);
    
    self.activate_project(
        tenant_id.as_deref().unwrap_or("default"),
        &project_id,  // ID estable basado en nombre
        &scope,       // description es el scope (lo que el usuario quiere hacer)
    ).await
}

/// Convierte un nombre de proyecto en un ID de filesystem válido.
/// "Aegis-Core" → "aegis-core"
/// "Mi Proyecto 2025!" → "mi-proyecto-2025"
fn sanitize_project_id(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
```

### Verificar `activate_project`

Confirmar que `activate_project` usa el primer argumento como `project_id`
para buscar en el filesystem. El orden debe ser:

```rust
pub async fn activate_project(
    &self,
    tenant_id: &str,     // "Tavo"
    project_id: &ProjectId,  // "aegis-core"
    project_description: &str, // "user wants to work on Aegis"
) -> anyhow::Result<AgentId>
```

Si el orden en `activate_project` también está invertido, corregirlo.

### Verificar deduplicación en CORE-281

CORE-281 busca supervisores por nombre con `name.to_lowercase() == project_name.to_lowercase()`.
Con este fix, la comparación es por `project_id` sanitizado, lo cual es más robusto.
Actualizar CORE-281 para usar `project_id` si ya está mergeado.

---

## Criterios de aceptación

- [x] `create_project("Aegis-Core", "user wants to work...", ...)` genera
  `project_id = "aegis-core"` en el filesystem
- [x] `create_project("Mi Proyecto!", "scope...", ...)` genera
  `project_id = "mi-proyecto"` sin caracteres especiales
- [x] Dos llamadas con el mismo nombre pero diferente scope producen el mismo `project_id`
  — el sistema los reconoce como el mismo proyecto
- [x] `sanitize_project_id` está testeado con strings vacíos, con espacios, con
  caracteres especiales
- [x] `cargo build --workspace` sin errores

## Dependencias

- CORE-281 (deduplicación usa nombre — verificar compatibilidad con project_id sanitizado)

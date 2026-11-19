# CORE-159 — feat: ProjectRegistry — Gestión de Proyectos y Supervisores Raíz

**Epic:** 43 — Hierarchical Multi-Agent Orchestration  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-155, CORE-156
**Estado:** ✅ Completo

---

## Contexto

Cuando el usuario dice "seguimos con Aegis y arrancamos la web app", el sistema necesita identificar proyectos por nombre e inferir cuáles son nuevos. El `ProjectRegistry` es el componente que mantiene ese conocimiento: qué proyectos existen, cuál es su `ProjectSupervisor` activo, y qué configuración tienen.

A diferencia del `AgentTree` (efímero por sesión), los metadatos de proyectos se **persisten en el enclave SQLCipher del tenant** para que Aegis recuerde proyectos entre sesiones.

---

## Cambios Requeridos

### Crear `ank-core/src/agents/project.rs`

```rust
use crate::agents::node::{AgentId, ProjectId};

/// Metadatos persistentes de un proyecto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub project_id: ProjectId,
    pub display_name: String,
    pub description: String,
    /// Última vez que tuvo un ProjectSupervisor activo
    pub last_active: chrono::DateTime<chrono::Utc>,
    /// System prompt base para el ProjectSupervisor de este proyecto
    pub supervisor_prompt: String,
    /// Dominios conocidos de este proyecto (se agregan dinámicamente)
    pub known_domains: Vec<String>,
}

/// Registro de proyectos activos e históricos del tenant.
pub struct ProjectRegistry {
    /// Proyectos cargados en memoria para la sesión actual
    known_projects: HashMap<ProjectId, ProjectMetadata>,
    /// Enclave del tenant para persistencia
    enclave: Arc<crate::enclave::CitadelEnclave>,
}

impl ProjectRegistry {
    pub fn new(enclave: Arc<crate::enclave::CitadelEnclave>) -> Self { ... }

    /// Carga proyectos desde el enclave al iniciar sesión.
    pub async fn load_from_enclave(&mut self, tenant_id: &str) -> anyhow::Result<()> { ... }

    /// Retorna el proyecto por ID. None si no existe.
    pub fn get(&self, project_id: &ProjectId) -> Option<&ProjectMetadata> { ... }

    /// Registra un proyecto nuevo o actualiza uno existente.
    pub async fn upsert(
        &mut self,
        metadata: ProjectMetadata,
        tenant_id: &str,
    ) -> anyhow::Result<()> { ... }

    /// Busca proyectos cuyo nombre contenga el query (case-insensitive).
    /// Usado para resolución de nombre → ProjectId desde el input del usuario.
    pub fn search_by_name(&self, query: &str) -> Vec<&ProjectMetadata> { ... }

    /// Actualiza last_active y known_domains de un proyecto.
    pub async fn touch(
        &mut self,
        project_id: &ProjectId,
        new_domain: Option<String>,
        tenant_id: &str,
    ) -> anyhow::Result<()> { ... }

    /// Lista todos los proyectos conocidos, ordenados por last_active desc.
    pub fn list_recent(&self) -> Vec<&ProjectMetadata> { ... }
}
```

### Tabla SQLCipher

El registro persiste en el enclave del tenant en una tabla nueva:

```sql
CREATE TABLE IF NOT EXISTS agent_projects (
    project_id      TEXT PRIMARY KEY,
    display_name    TEXT NOT NULL,
    description     TEXT NOT NULL DEFAULT '',
    last_active     TEXT NOT NULL,
    supervisor_prompt TEXT NOT NULL DEFAULT '',
    known_domains   TEXT NOT NULL DEFAULT '[]'  -- JSON array
);
```

La migración debe ser **additive** — no puede romper tenants existentes que no tienen esta tabla.

---

## Criterios de Aceptación

- [x] `ProjectRegistry::load_from_enclave()` no falla si la tabla no existe (crea la tabla si es nueva)
- [x] `ProjectRegistry::search_by_name("aegis")` encuentra proyectos con nombres como "Aegis OS", "AEGIS", "aegis-core"
- [x] `ProjectRegistry::upsert()` persiste en SQLCipher y actualiza `known_projects` en memoria
- [x] `ProjectRegistry::touch()` agrega el nuevo dominio a `known_domains` sin duplicados
- [x] La migración de tabla es additive — tenants existentes no se rompen
- [x] No hay `unwrap()` ni `expect()`
- [x] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*
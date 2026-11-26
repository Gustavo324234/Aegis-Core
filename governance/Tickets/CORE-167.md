# CORE-167 — feat: workspace_config — Tabla SQLCipher y Endpoint de Configuración

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-core + ank-http  
**Tipo:** feat  
**Prioridad:** Crítica (fundacional de la épica)  
**Asignado a:** Kernel Engineer  
**Dependencias:** ninguna
**Estado:** ✅ Completo

---

## Contexto

Antes de que el tenant pueda usar terminal, Git o Code Viewer, necesita configurar su workspace: dónde está el proyecto, cuál es el repo de GitHub, cuál es su token. Este ticket crea la tabla de configuración y el endpoint para leerla y escribirla.

---

## Cambios Requeridos

### 1. Migración SQLCipher — tabla `workspace_config`

En la lógica de migraciones del enclave Citadel del tenant:

```sql
CREATE TABLE IF NOT EXISTS workspace_config (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

Claves usadas por la Epic 44:

| key | descripción | ejemplo |
|---|---|---|
| `github_token` | PAT del tenant para GitHub | `ghp_...` |
| `project_root` | Ruta absoluta al repo en el servidor | `/home/tavo/Aegis-Core` |
| `github_repo` | `owner/repo` | `Gustavo324234/Aegis-Core` |
| `terminal_allowlist` | JSON array de comandos permitidos | `["cargo","npm","git"]` |
| `pr_merge_mode` | `"automatic"` o `"manual"` | `"manual"` |
| `pr_auto_fix_ci` | `"true"` o `"false"` | `"true"` |

**La migración es additive** — no puede romper tenants existentes.

### 2. Módulo `ank-core/src/workspace/config.rs`

```rust
pub struct WorkspaceConfig {
    enclave: Arc<CitadelEnclave>,
}

impl WorkspaceConfig {
    pub fn new(enclave: Arc<CitadelEnclave>) -> Self { ... }

    pub async fn get(&self, tenant_id: &str, key: &str) -> anyhow::Result<Option<String>> { ... }

    pub async fn set(&self, tenant_id: &str, key: &str, value: &str) -> anyhow::Result<()> { ... }

    /// Retorna la configuración completa del workspace como struct tipado.
    /// Los campos faltantes retornan sus defaults.
    pub async fn load_all(&self, tenant_id: &str) -> anyhow::Result<WorkspaceSettings> { ... }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceSettings {
    pub github_token: Option<String>,   // None si no configurado
    pub project_root: Option<String>,
    pub github_repo: Option<String>,
    pub terminal_allowlist: Vec<String>,
    pub pr_merge_mode: MergeMode,
    pub pr_auto_fix_ci: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum MergeMode {
    Automatic,
    #[default]
    Manual,
}
```

**Importante:** `github_token` NUNCA aparece en ningún log ni respuesta de API. Al serializar para el frontend, se reemplaza por `"configured"` o `null`.

### 3. Endpoints en `ank-http/src/routes/workspace.rs`

```
GET  /api/workspace/config
     → Retorna WorkspaceSettings (token → "configured" o null, nunca el valor real)
     → Requiere: Citadel auth

POST /api/workspace/config
     → Body: { key: string, value: string }
     → Guarda en workspace_config del enclave del tenant
     → Requiere: Citadel auth
     → Retorna 200 OK o 400 con mensaje de error
```

---

## Criterios de Aceptación

- [x] La tabla `workspace_config` se crea automáticamente si no existe (primera vez)
- [x] Tenants existentes sin la tabla no se rompen (migración additive)
- [x] `GET /api/workspace/config` nunca retorna el valor real de `github_token` — solo `"configured"` o `null`
- [x] `POST /api/workspace/config` con `key = "github_token"` guarda el token cifrado en el enclave
- [x] `WorkspaceConfig::load_all()` retorna defaults razonables cuando no hay configuración
- [x] `terminal_allowlist` default es `["cargo", "npm", "git", "python"]`
- [x] `pr_merge_mode` default es `Manual`
- [x] `pr_auto_fix_ci` default es `true`
- [x] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*
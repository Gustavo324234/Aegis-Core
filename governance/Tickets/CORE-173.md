# CORE-173 — feat: PR Manager — Ciclo de Vida de PRs con Polling de CI

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-core + ank-http  
**Tipo:** feat  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-171, CORE-167
**Estado:** ✅ Completo

---

## Contexto

El PR Manager es el componente que registra los PRs creados por Aegis, monitorea su estado de CI, ejecuta el merge automático si está configurado, y notifica al tenant por el chat cuando algo relevante ocurre.

---

## Cambios Requeridos

### 1. Tabla SQLCipher `managed_prs`

```sql
CREATE TABLE IF NOT EXISTS managed_prs (
    pr_number     INTEGER PRIMARY KEY,
    title         TEXT NOT NULL,
    branch        TEXT NOT NULL,
    base_branch   TEXT NOT NULL DEFAULT 'main',
    url           TEXT NOT NULL,
    merge_mode    TEXT NOT NULL DEFAULT 'manual',   -- 'automatic' | 'manual'
    auto_fix_ci   INTEGER NOT NULL DEFAULT 1,        -- 0 | 1
    status        TEXT NOT NULL DEFAULT 'open',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
```

### 2. Módulo `ank-core/src/pr_manager/mod.rs`

```rust
pub struct PrManager {
    enclave: Arc<CitadelEnclave>,
    git: Arc<GitHubBridge>,
    ws_tx: mpsc::Sender<WsEvent>,   // para notificar al tenant
    scheduler: Arc<CognitiveScheduler>,  // para disparar auto-fix (CORE-174)
}

impl PrManager {
    /// Registra un PR nuevo en la tabla managed_prs.
    /// Llamado inmediatamente después de GitHubBridge::create_pr().
    pub async fn register_pr(
        &self,
        tenant_id: &str,
        pr: &PullRequest,
        merge_mode: MergeMode,
        auto_fix_ci: bool,
    ) -> anyhow::Result<()> { ... }

    /// Actualiza el status y updated_at de un PR.
    pub async fn update_status(
        &self,
        tenant_id: &str,
        pr_number: u64,
        status: PrStatus,
    ) -> anyhow::Result<()> { ... }

    /// Retorna todos los PRs activos (no Merged ni Closed) del tenant.
    pub async fn list_active(&self, tenant_id: &str) -> anyhow::Result<Vec<ManagedPr>> { ... }

    /// Loop de polling — llamar en un tokio::spawn al iniciar el servidor.
    /// Cada 30 segundos consulta el estado de CI de todos los PRs activos
    /// de todos los tenants y reacciona según su configuración.
    pub async fn polling_loop(&self) { ... }

    /// Evalúa el estado de un PR y toma acción si corresponde.
    async fn evaluate_pr(&self, tenant_id: &str, pr: &ManagedPr) -> anyhow::Result<()> {
        let checks = self.git.get_pr_checks(pr.pr_number).await?;
        let all_success = checks.iter().all(|c| c.state == CiState::Success);
        let any_failure = checks.iter().any(|c| c.state == CiState::Failure);

        if any_failure {
            if pr.auto_fix_ci && pr.status != PrStatus::AutoFixInProgress {
                // Disparar CORE-174
                self.trigger_auto_fix(tenant_id, pr).await?;
                self.update_status(tenant_id, pr.pr_number, PrStatus::AutoFixInProgress).await?;
            } else {
                self.update_status(tenant_id, pr.pr_number, PrStatus::CiFailed).await?;
                // Notificar al tenant por WebSocket
                self.ws_tx.send(WsEvent::PrUpdate {
                    pr_number: pr.pr_number,
                    status: PrStatus::CiFailed,
                    url: pr.url.clone(),
                }).await.ok();
            }
        } else if all_success && !checks.is_empty() {
            self.update_status(tenant_id, pr.pr_number, PrStatus::CiPassed).await?;
            match pr.merge_mode {
                MergeMode::Automatic => {
                    self.merge_pr(tenant_id, pr).await?;
                }
                MergeMode::Manual => {
                    self.update_status(tenant_id, pr.pr_number, PrStatus::MergeReady).await?;
                    self.ws_tx.send(WsEvent::PrUpdate {
                        pr_number: pr.pr_number,
                        status: PrStatus::MergeReady,
                        url: pr.url.clone(),
                    }).await.ok();
                }
            }
        }

        // Verificar si fue mergeado externamente (por Tavo manualmente)
        if self.git.is_pr_merged(pr.pr_number).await? {
            self.update_status(tenant_id, pr.pr_number, PrStatus::Merged).await?;
            self.ws_tx.send(WsEvent::PrMerged {
                pr_number: pr.pr_number,
                title: pr.title.clone(),
            }).await.ok();
        }

        Ok(())
    }

    async fn merge_pr(&self, tenant_id: &str, pr: &ManagedPr) -> anyhow::Result<()> {
        // POST /repos/{owner}/{repo}/pulls/{pr_number}/merge via GitHub API
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/merge",
            self.git.repo_owner, self.git.repo_name, pr.pr_number
        );
        self.git.http.put(&url)
            .json(&serde_json::json!({ "merge_method": "squash" }))
            .send().await?.error_for_status()?;

        self.update_status(tenant_id, pr.pr_number, PrStatus::Merged).await?;
        self.ws_tx.send(WsEvent::PrMerged {
            pr_number: pr.pr_number,
            title: pr.title.clone(),
        }).await.ok();
        Ok(())
    }
}

// ── Tipos ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedPr {
    pub pr_number: u64,
    pub title: String,
    pub branch: String,
    pub base_branch: String,
    pub url: String,
    pub merge_mode: MergeMode,
    pub auto_fix_ci: bool,
    pub status: PrStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PrStatus {
    Open,
    CiRunning,
    CiPassed,
    CiFailed,
    AutoFixInProgress,
    MergeReady,
    Merged,
    Closed,
}
```

### 3. Endpoints HTTP en `ank-http/src/routes/prs.rs`

```
GET  /api/prs              → lista todos los PRs del tenant (activos e históricos)
POST /api/prs              → crea un PR y lo registra (body: { branch, base, title, body, merge_mode, auto_fix_ci })
PATCH /api/prs/{number}    → actualiza merge_mode o auto_fix_ci de un PR existente
POST /api/prs/{number}/merge → merge manual inmediato (para modo Manual cuando CI pasó)
```

### 4. Inicialización del polling loop

En el startup de `ank-http` (donde se construye el servidor Axum), agregar:

```rust
let pr_manager = Arc::new(PrManager::new(...));
tokio::spawn({
    let pm = pr_manager.clone();
    async move { pm.polling_loop().await }
});
```

---

## Criterios de Aceptación

- [x] `POST /api/prs` crea el PR en GitHub y lo registra en `managed_prs`
- [x] El polling loop se ejecuta cada 30 segundos y no crashea si GitHub API devuelve error
- [x] En modo `automatic`: cuando todos los CI checks pasan, el merge se ejecuta automáticamente sin intervención del tenant
- [x] En modo `manual`: cuando CI pasa, llega el evento `pr_update` con status `MergeReady` al WebSocket del tenant
- [x] `POST /api/prs/{number}/merge` permite al tenant mergear manualmente un PR en estado `MergeReady`
- [x] Si Tavo mergea el PR directamente en GitHub (sin pasar por Aegis), el polling lo detecta y envía el evento `pr_merged` al chat
- [x] La tabla `managed_prs` se crea automáticamente (migración additive)
- [x] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*
# CORE-174 — feat: Auto-fix CI — Proceso Cognitivo Disparado por Fallo de CI

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-173, CORE-171, CORE-158 (AgentOrchestrator)

---

## Contexto

Cuando un PR tiene `auto_fix_ci = true` y el CI falla, el PR Manager (CORE-173) dispara este flujo: se crea un proceso cognitivo normal (no un AgentNode completo) que recibe el error de CI como contexto, genera un fix, lo commitea a la misma branch, y pushea — disparando un nuevo CI run.

Según ADR-WS-005: el auto-fix usa un proceso cognitivo puntual, no toda la infraestructura de AgentNode de Epic 43. Es una inferencia simple con contexto enriquecido.

---

## Cambios Requeridos

### Función `trigger_auto_fix` en `PrManager`

```rust
async fn trigger_auto_fix(&self, tenant_id: &str, pr: &ManagedPr) -> anyhow::Result<()> {
    // 1. Obtener los logs/descripción del CI fallido
    let ci_error = self.git.get_failed_ci_logs(pr.pr_number).await
        .unwrap_or_else(|_| "CI check failed. No logs available.".to_string());

    // 2. Obtener el diff de la branch (qué cambios hay en el PR)
    let diff = self.git.git_output(&["diff", "main", &pr.branch]).await
        .unwrap_or_default();
    let diff_truncated = diff.chars().take(3000).collect::<String>(); // max 3000 chars

    // 3. Construir el prompt de auto-fix
    let prompt = format!(
        "El CI del PR #{} ('{}') ha fallado.\n\n\
         Error reportado:\n{}\n\n\
         Cambios en la branch:\n```diff\n{}\n```\n\n\
         Analizá el error, identificá la causa raíz en el diff, y:\n\
         1. Explicá brevemente qué está mal\n\
         2. Generá el fix exacto (archivos y cambios)\n\
         3. Usá SYS_EXEC para aplicar el fix si es posible\n\
         4. Usá SYS_GIT_COMMIT para commitear el fix\n\
         5. Usá SYS_GIT_PUSH para pushear\n\
         Intentá #{} de máximo 3.",
        pr.pr_number, pr.title,
        ci_error,
        diff_truncated,
        pr.auto_fix_attempts + 1
    );

    // 4. Crear un PCB normal en el CognitiveScheduler con este prompt
    // El proceso usa TaskType::Code para que el CMR asigne el mejor modelo de coding
    self.scheduler.enqueue_task(tenant_id, prompt, TaskType::Code).await?;

    // 5. Incrementar el contador de intentos en la DB
    // Si ya se intentaron 3 veces, marcar como CiFailed y no intentar más
    if pr.auto_fix_attempts >= 2 {
        self.update_status(tenant_id, pr.pr_number, PrStatus::CiFailed).await?;
        self.ws_tx.send(WsEvent::PrUpdate {
            pr_number: pr.pr_number,
            status: PrStatus::CiFailed,
            url: pr.url.clone(),
        }).await.ok();
        // Notificar que se agotaron los intentos
        self.ws_tx.send(WsEvent::ChatNotification {
            message: format!(
                "⚠️ Auto-fix agotó 3 intentos para PR #{} ('{}'). Revisión manual requerida. {}",
                pr.pr_number, pr.title, pr.url
            ),
        }).await.ok();
    }

    Ok(())
}
```

### Campo `auto_fix_attempts` en `managed_prs`

Agregar columna a la tabla (migración additive):

```sql
ALTER TABLE managed_prs ADD COLUMN auto_fix_attempts INTEGER NOT NULL DEFAULT 0;
```

Y al struct `ManagedPr`:

```rust
pub auto_fix_attempts: u32,
```

### Evento WebSocket para notificar el intento

```json
{
  "event": "ci_fix_attempt",
  "data": {
    "pr_number": 42,
    "attempt": 1,
    "max_attempts": 3,
    "error_summary": "CI check 'cargo fmt --check' failed."
  }
}
```

---

## Criterios de Aceptación

- [ ] Cuando CI falla y `auto_fix_ci = true`, se envía el evento `ci_fix_attempt` al WebSocket del tenant
- [ ] El proceso cognitivo de auto-fix recibe el error de CI y el diff del PR como contexto
- [ ] Si el fix tiene éxito (nuevo push → CI pasa), el flujo normal del PR Manager continúa (CORE-173)
- [ ] Si se superan 3 intentos, el PR queda en `CiFailed` y el tenant recibe notificación en el chat con el link al PR
- [ ] El contador `auto_fix_attempts` se incrementa correctamente en la DB
- [ ] Si el CI falla pero `auto_fix_ci = false`, no se dispara el auto-fix (solo notificación)
- [ ] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: Infraestructura completa de auto-fix CI: PrStatus::AutoFixInProgress, auto_fix_ci/auto_fix_attempts en pr_manager/mod.rs y enclave (columnas + pr_set_auto_fix_ci).

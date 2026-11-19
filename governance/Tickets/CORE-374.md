# CORE-374: Limpieza de Raíz del Repositorio y Actualización de `.gitignore`

- **Tipo:** chore
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 58
- **Asignado a:** DevOps Engineer

---

## Description
En la raíz del repositorio quedaron archivos temporales de debugging y CI (`ci_deny_run5.log`, `ci_deny_run6.log`, `ci_failed_job.log`, `ci_failed_run2.log`, `ci_failed_run3.log`, `ci_failed_run4.log`, `pr_body.txt`, `pr_url.txt`, `token_output.log`) que dificultan la lectura de la estructura del proyecto para nuevos usuarios y colaboradores. Asimismo, `.gitignore` bloqueaba la visibilidad pública de `governance/TICKETS_MASTER.md` y `governance/Tickets/` referenciados en `CONTRIBUTING.md`.

## Acceptance Criteria
- [x] Eliminar los archivos `.log` y `.txt` residuales de CI/PR de la raíz del repositorio y quitar `pr_url.txt` del índice de Git.
- [x] Actualizar `.gitignore` para ignorar `pr_url.txt`, `tools/agent_sandbox/logs/` y `tools/agent_sandbox/workspace/`, permitiendo el seguimiento de `governance/TICKETS_MASTER.md` y `governance/Tickets/CORE-*.md`.

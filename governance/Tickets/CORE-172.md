# CORE-172 — feat: SYS_GIT_* — Syscalls Git para Agentes

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-171, CORE-162 (patrón syscalls Epic 43)
**Estado:** ✅ Completo

---

## Contexto

Los agentes de Epic 43 necesitan poder crear branches, hacer commits y pushear su trabajo igual que Claude Code. Este ticket expone las operaciones del `GitHubBridge` como syscalls que el `run_agent_loop` del `AgentOrchestrator` puede detectar y ejecutar.

---

## Cambios Requeridos

### Tres syscalls nuevas

#### SYS_GIT_BRANCH
Crea una branch nueva desde la base indicada.

```json
{
  "syscall": "SYS_GIT_BRANCH",
  "params": {
    "branch_name": "feat/epic-43-agent-node",
    "from": "main"
  }
}
```

#### SYS_GIT_COMMIT
Stage de archivos y commit con mensaje Conventional Commits.

```json
{
  "syscall": "SYS_GIT_COMMIT",
  "params": {
    "files": ["kernel/crates/ank-core/src/agents/node.rs", "kernel/crates/ank-core/src/agents/mod.rs"],
    "message": "feat(ank-core): CORE-155 AgentNode tipos base"
  }
}
```

Retorna al agente: `{ "sha": "a1b2c3d4...", "success": true }`

#### SYS_GIT_PUSH
Push de la branch actual al remoto.

```json
{
  "syscall": "SYS_GIT_PUSH",
  "params": {
    "branch_name": "feat/epic-43-agent-node"
  }
}
```

### Handlers en `ank-core/src/syscalls/`

Agregar handlers `handle_sys_git_branch`, `handle_sys_git_commit`, `handle_sys_git_push` que:
1. Obtienen el `GitHubBridge` del `AgentOrchestrator` (compartido via `Arc`)
2. Llaman al método correspondiente del bridge
3. Retornan el resultado como contexto para la próxima inferencia del agente

### Integración en `run_agent_loop`

Extender el `match` de syscalls en CORE-158/CORE-162:

```rust
"SYS_GIT_BRANCH" => {
    let result = handle_sys_git_branch(params, git_bridge.clone()).await;
    additional_context.push(match result {
        Ok(_) => format!("Branch '{}' created successfully from '{}'", params.branch_name, params.from),
        Err(e) => format!("Branch creation failed: {}", e),
    });
}
"SYS_GIT_COMMIT" => {
    let result = handle_sys_git_commit(params, git_bridge.clone()).await;
    additional_context.push(match result {
        Ok(sha) => format!("Committed {} files. SHA: {}", params.files.len(), sha),
        Err(e) => format!("Commit failed: {}", e),
    });
}
"SYS_GIT_PUSH" => {
    let result = handle_sys_git_push(params, git_bridge.clone()).await;
    additional_context.push(match result {
        Ok(_) => format!("Branch '{}' pushed to origin", params.branch_name),
        Err(e) => format!("Push failed: {}", e),
    });
}
```

### Evento WebSocket al pushear

Cuando `SYS_GIT_PUSH` tiene éxito, además de retornar contexto al agente, enviar al frontend:

```json
{
  "event": "git_push",
  "data": {
    "agent_id": "spec-scheduler",
    "branch": "feat/epic-43-agent-node",
    "commit_sha": "a1b2c3d4"
  }
}
```

---

## Criterios de Aceptación

- [x] Un agente puede crear una branch via `SYS_GIT_BRANCH` y el resultado aparece en su contexto
- [x] Un agente puede hacer commit de archivos específicos via `SYS_GIT_COMMIT` con el SHA retornado
- [x] El commit aparece en `git log` con `author: Aegis OS <bot@aegis-os.dev>`
- [x] Un agente puede pushear via `SYS_GIT_PUSH` y el evento `git_push` llega al WebSocket
- [x] Si `SYS_GIT_COMMIT` falla (ej: nada que commitear), el agente recibe el error en contexto y no crashea
- [x] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*
# CORE-169 — feat: SYS_EXEC — Syscall de Terminal para Agentes

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-168, CORE-162 (patrón de syscalls de Epic 43)
**Estado:** ✅ Completo

---

## Contexto

Para que los agentes de Epic 43 puedan usar el `TerminalExecutor`, necesitan una syscall equivalente a `SYS_AGENT_SPAWN` (CORE-162) pero para ejecución de comandos. El agente incluye en su output JSON una instrucción `SYS_EXEC`, el `run_agent_loop` del `AgentOrchestrator` la detecta, la ejecuta, y el output queda disponible para la próxima inferencia del agente.

---

## Cambios Requeridos

### Formato de syscall en el output del LLM

El agente escribe en su output:

```json
{
  "syscall": "SYS_EXEC",
  "params": {
    "command": "cargo",
    "args": ["build", "--workspace"],
    "blocking": true
  }
}
```

- `blocking: true` → el agente espera el resultado antes de continuar su inferencia
- `blocking: false` → el agente continúa y el output llega por WebSocket sin bloquear

### Handler en `ank-core/src/syscalls/`

Agregar el handler `SYS_EXEC`:

```rust
pub async fn handle_sys_exec(
    params: SysExecParams,
    executor: Arc<TerminalExecutor>,
    ws_tx: mpsc::Sender<WsEvent>,   // para enviar terminal_output al frontend
    agent_id: AgentId,
) -> anyhow::Result<SysExecResult> {
    let (line_tx, mut line_rx) = mpsc::channel(256);
    
    // Spawnear la ejecución en background
    let exec_handle = tokio::spawn({
        let executor = executor.clone();
        let cmd = params.command.clone();
        let args = params.args.clone();
        async move { executor.exec(&cmd, &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(), line_tx).await }
    });
    
    // Forwardear cada línea al WebSocket como evento terminal_output
    let mut all_output = Vec::new();
    while let Some(line) = line_rx.recv().await {
        all_output.push(line.clone());
        let _ = ws_tx.send(WsEvent::TerminalOutput { agent_id, line }).await;
    }
    
    let exit_code = exec_handle.await??;
    
    Ok(SysExecResult {
        exit_code,
        output: all_output,
        success: exit_code == 0,
    })
}

#[derive(Debug, Deserialize)]
pub struct SysExecParams {
    pub command: String,
    pub args: Vec<String>,
    pub blocking: bool,
}

#[derive(Debug, Serialize)]
pub struct SysExecResult {
    pub exit_code: i32,
    pub output: Vec<TerminalLine>,
    pub success: bool,
}
```

### Integración en `run_agent_loop` (CORE-158)

En el parsing del output del LLM en el `AgentOrchestrator`, agregar el caso `SYS_EXEC`:

```rust
if let Some(syscall_block) = extract_syscall_json(&llm_output) {
    match syscall_block.syscall.as_str() {
        "SYS_AGENT_SPAWN" => { /* CORE-162 */ }
        "SYS_EXEC" => {
            let result = handle_sys_exec(syscall_block.params, executor.clone(), ws_tx.clone(), agent_id).await?;
            // Si blocking = true, inyectar el resultado como contexto adicional
            // para la próxima inferencia del agente
            if syscall_block.params.blocking {
                additional_context.push(format!(
                    "Command `{} {}` exited with code {}. Output:\n{}",
                    params.command,
                    params.args.join(" "),
                    result.exit_code,
                    result.output.iter().map(|l| &l.content).take(50).cloned().collect::<Vec<_>>().join("\n")
                ));
            }
        }
        _ => { /* syscall desconocida, ignorar */ }
    }
}
```

---

## Criterios de Aceptación

- [x] Un agente que emite `SYS_EXEC` en su output ve el resultado de la ejecución en su próxima inferencia (modo blocking)
- [x] El output de la ejecución llega al frontend como eventos `terminal_output` (CORE-175) en tiempo real
- [x] Si el comando no está en la allowlist, el agente recibe `exit_code: -1` y `success: false` en el contexto
- [x] Modo `blocking: false` no bloquea el loop del agente
- [x] El output inyectado en el contexto está truncado a máximo 50 líneas (no contaminar el contexto con output masivo)
- [x] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*
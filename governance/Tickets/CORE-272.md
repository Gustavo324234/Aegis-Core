# CORE-272 — Kernel: herramienta `get_project_ledger` para el chat_agent

**Tipo:** feat  
**Prioridad:** Media  
**Épica:** EPIC 50 — Agent Inbox  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/agents/tool_registry.rs`
- `kernel/crates/ank-core/src/chal/mod.rs`
- `kernel/config/agents/chat_agent.md`
**Estado:** ✅ Completo

---

## Contexto

El chat_agent no tiene acceso automático al estado del proyecto — lo cual es intencional
(el usuario decide cuándo pedirle contexto). Este ticket le agrega la herramienta
`get_project_ledger` para que el chat_agent pueda consultar el avance, decisiones, y
exchanges con supervisores **solo cuando el usuario lo pide explícitamente**.

Ejemplo de interacción objetivo:
```
Usuario: "¿Qué le contesté al supervisor de Aegis-Core?"
Chat agent: [llama get_project_ledger("Aegis-Core")]
Chat agent: "Le dijiste que podía ejecutar comandos y clonar el repositorio (2026-05-06 22:17)."

Usuario: "¿Qué falta hacer en ese proyecto?"
Chat agent: [llama get_project_ledger("Aegis-Core")]
Chat agent: "Según el registro, falta implementar AgentBadge (CORE-269) y los tests de auth."
```

**Corrección de diseño:** la versión anterior de este ticket proponía un
`AgentThreadHistory` en memoria en el `AgentOrchestrator` por `agent_id`. Ese diseño
era incorrecto — el historial de intercambios usuario↔supervisor pertenece al
`ProjectLedger` (disco, por `project_id`, implementado en CORE-273). El supervisor
es efímero. El proyecto no.

---

## Cambios requeridos

### 1. `tool_registry.rs` — herramienta `get_project_ledger` para ChatAgent

```json
{
  "type": "function",
  "function": {
    "name": "get_project_ledger",
    "description": "Obtener el registro de avance de un proyecto: decisiones tomadas, tareas completadas, trabajo pendiente, e intercambios con supervisores. Usar SOLO cuando el usuario pregunte explícitamente sobre el estado o historial de un proyecto.",
    "parameters": {
      "type": "object",
      "properties": {
        "project_name": {
          "type": "string",
          "description": "Nombre del proyecto a consultar"
        }
      },
      "required": ["project_name"]
    }
  }
}
```

### 2. `chal/mod.rs` — arm `get_project_ledger`

```rust
"get_project_ledger" => {
    let project_name = args["project_name"].as_str().unwrap_or("").to_string();

    let orchestrator_opt = self.agent_orchestrator.read().await.clone();
    let orchestrator = match orchestrator_opt {
        Some(o) => o,
        None => return "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
    };

    match orchestrator.get_project_ledger(&project_name).await {
        Some(ledger) => serde_json::to_string(&ledger)
            .unwrap_or_else(|_| "{\"error\":\"serialization_error\"}".to_string()),
        None => format!(
            "{{\"error\":\"no_ledger_found\",\"project\":\"{}\"}}",
            project_name
        ),
    }
}
```

### 3. Método `get_project_ledger` en `AgentOrchestrator`

```rust
/// CORE-272: Lee el ProjectLedger del proyecto por nombre (case-insensitive).
pub async fn get_project_ledger(&self, project_name: &str) -> Option<ProjectLedger> {
    // Encontrar el project_id que coincide con el nombre
    let project_id = {
        let tree = self.tree.read().await;
        tree.all_nodes()
            .iter()
            .filter(|n| matches!(&n.role, AgentRole::ProjectSupervisor { name, .. } if
                name.to_lowercase().contains(&project_name.to_lowercase())
            ))
            .map(|n| n.project_id.clone())
            .next()?
    };

    // Leer el ledger del tenant actual
    let tenant_id = /* obtener del contexto — ver cómo lo hacen otros métodos que usan persistence */;

    self.persistence
        .load_ledger(&tenant_id, &project_id)
        .ok()
        .flatten()
}
```

### 4. `chat_agent.md` — instrucción sobre `get_project_ledger`

Agregar al final de `kernel/config/agents/chat_agent.md`:

```markdown
## Consulta de estado de proyectos

Solo usá `get_project_ledger` cuando el usuario te pregunte explícitamente sobre
el estado, historial, o decisiones de un proyecto (ej: "¿qué falta hacer?",
"¿qué le respondí al supervisor?", "¿qué se decidió sobre X?").

No consultes ledgers proactivamente. El usuario sabe qué proyectos tiene activos.
Al presentar el ledger, resumí en lenguaje natural — no vuelques el JSON crudo.
```

---

## Criterios de aceptación

- [x] La herramienta `get_project_ledger` está registrada para `AgentRole::ChatAgent`
- [x] El arm en `execute_tool_call_internal` lee del `ProjectLedger` en disco (via CORE-273)
- [x] El retorno incluye decisiones, completado, pendiente, y user_exchanges
- [x] Si no hay ledger para ese proyecto, retorna `{"error": "no_ledger_found"}`
- [x] El chat_agent puede responder sobre avance y decisiones usando los datos retornados
- [x] `cargo build --workspace` sin errores

## Dependencias

- CORE-273 (ProjectLedger implementado — este ticket depende de él)
- CORE-271 (respuestas del usuario ya están en el ledger al llegar aquí)

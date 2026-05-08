# BRIEFING — Kernel: fixes arquitecturales del orchestrator

**Fecha:** 2026-05-07  
**Para:** Kernel Engineer (Claude Code)  
**Tickets:** CORE-285, CORE-286, CORE-287, CORE-288, CORE-289

---

## Branch

```
fix/core-285-289-orchestrator-arch
```

---

## Prerequisito

Leer los cinco tickets antes de implementar:
- `governance/Tickets/CORE-285.md`
- `governance/Tickets/CORE-286.md`
- `governance/Tickets/CORE-287.md`
- `governance/Tickets/CORE-288.md`
- `governance/Tickets/CORE-289.md`

**CORE-286 y CORE-288 modifican el mismo método `run_agent_loop`.**
Implementarlos juntos en un solo bloque para evitar conflictos.

---

## Orden de implementación

### 1. CORE-287 — Fix project_id (10 min, bajo riesgo)

**Archivo:** `kernel/crates/ank-core/src/agents/orchestrator.rs`

En el método `create_project`, los argumentos `name` y `scope` están invertidos
al llamar a `activate_project`. Corregir el orden y agregar la función
`sanitize_project_id(name: &str) -> String` que convierte nombres a IDs válidos
de filesystem (lowercase, guiones, sin espacios ni especiales).

Ver implementación exacta en `governance/Tickets/CORE-287.md`.

Agregar test unitario para `sanitize_project_id` con casos:
- String normal: "Aegis-Core" → "aegis-core"
- Con espacios: "Mi Proyecto" → "mi-proyecto"
- Con especiales: "Proyecto 2025!" → "proyecto-2025"
- String vacío: "" → ""

---

### 2. CORE-286 + CORE-288 — Timeout + fix síntesis (juntos, 45 min)

**Archivo:** `kernel/crates/ank-core/src/agents/orchestrator.rs`

Estos dos tickets modifican `run_agent_loop` — implementarlos en el mismo bloque:

**De CORE-286:**
- Cambiar `while let Some(msg) = rx.recv().await` por `loop` con `tokio::select!`
- Agregar arm de timeout (`AGENT_IDLE_TIMEOUT = 300s`) que marca el agente como
  Failed y remueve el canal
- Fix de `spawn_loop`: insertar el nuevo `tx` en el mapa ANTES de hacer spawn del task

**De CORE-288:**
- Agregar flag `synthesis_done: bool = false` al inicio del loop
- En el arm `Report`: `if all_done && !synthesis_done { synthesis_done = true; /* send synth */ }`
- En el arm `Dispatch`: si el agente está `Complete && synthesis_done`, hacer `continue`
- ProjectSupervisor (sin parent_tx): hacer `break` después de `Complete`

Ver código completo en ambos tickets.

---

### 3. CORE-289 — get_agent_status tool (20 min)

**Archivo A:** `kernel/crates/ank-core/src/agents/tool_registry.rs`

Agregar `get_agent_status()` tool definition y registrarlo para `AgentRole::ChatAgent`.

**Archivo B:** `kernel/crates/ank-core/src/chal/mod.rs`

Agregar arm `"get_agent_status"` en `execute_tool_call_internal`. Lee del
`orchestrator.tree_snapshot()`, filtra por `project_name` si se proveyó,
y retorna JSON con estado de cada agente.

**Archivo C:** `kernel/config/agents/chat_agent.md`

Agregar sección "Verificar estado antes de spawner" con instrucciones sobre
cuándo usar `get_agent_status` y cómo interpretar los resultados.

Ver código completo en `governance/Tickets/CORE-289.md`.

---

### 4. CORE-285 — Configuración de modelo en installer (30 min)

**Archivo:** `installer/install.sh`

Agregar en `show_main_menu()` una sección para configurar el proveedor de IA.
El usuario elige entre OpenRouter, OpenAI, Anthropic, o "configurar después".
Si ingresa API key, se guarda en `ENV_FILE`.

En el kernel (`ank-server/src/main.rs` o donde se inicializa el router/catalog):
leer `AEGIS_DEFAULT_PROVIDER`, `AEGIS_DEFAULT_MODEL`, `AEGIS_DEFAULT_API_KEY`
de variables de entorno. Si existen, registrarlas como modelo default con prioridad
sobre `openrouter/free`. Si no existen, loguear WARNING al startup.

Ver implementación completa en `governance/Tickets/CORE-285.md`.

---

## Verificación

```bash
cargo build --workspace
cargo test --workspace -- agents::  # tests del módulo de agentes
bash -n installer/install.sh
shellcheck installer/install.sh
```

---

## Commit y PR

**Commit message:**
```
fix(agents,installer): CORE-285-289 orchestrator arch fixes + AI provider config
```

**PR title:**
```
fix(agents,installer): CORE-285/286/287/288/289 — orchestrator fixes + provider config
```

**PR description:**
```
## CORE-285 — Configuración obligatoria de modelo en installer
- Installer pregunta por proveedor y API key antes de instalar
- Vars AEGIS_DEFAULT_PROVIDER/MODEL/API_KEY escritas en ENV_FILE
- Kernel loguea WARNING al startup si no hay API key

## CORE-286 — Timeout en run_agent_loop + fix canal
- Agentes idle >5min se auto-terminan y se remueven del mapa
- spawn_loop inserta tx en el mapa antes del tokio::spawn (fix race condition)
- terminate marca nodos como Failed si no estaban Complete

## CORE-287 — Fix project_id usa scope en lugar de nombre
- create_project corregido: project_id = sanitize(name), description = scope
- sanitize_project_id convierte nombres a IDs de filesystem válidos
- Tests unitarios para sanitize_project_id

## CORE-288 — Fix síntesis de reportes hijos
- Flag synthesis_done previene síntesis múltiples
- Dispatches y Reports tardíos ignorados después de Complete
- ProjectSupervisor sin padre termina loop después de Complete

## CORE-289 — get_agent_status para chat_agent
- Tool get_agent_status en ToolRegistry para ChatAgent
- Arm en execute_tool_call_internal lee tree_snapshot()
- chat_agent.md: instrucciones sobre cuándo usar get_agent_status

## Verificación
cargo build --workspace ✅
cargo test --workspace -- agents:: ✅
shellcheck installer/install.sh ✅
```

**Target branch:** `main`

---

*Briefing creado por Arquitecto IA — 2026-05-07*

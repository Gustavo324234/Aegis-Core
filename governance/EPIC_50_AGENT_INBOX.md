# EPIC 50 — Agent Inbox: Comunicación Directa Usuario ↔ Supervisores + Specialist Tools

**Estado:** 📥 Planned  
**Prioridad:** Alta  
**Responsable planning:** Arquitecto IA  
**Ingenieros:** Kernel Engineer + Shell Engineer  
**Depende de:** EPIC 49 — CORE-262 ✅, CORE-263 ✅

---

## Visión

Dos objetivos combinados en este epic:

**1. Specialists pueden hacer trabajo real**
Hoy los specialists solo tienen `report`. Con CORE-275/276/277 pueden leer y
escribir archivos, acceder a paths externos con aprobación del usuario, y buscar
en la web. El árbol de agentes pasa de ser decorativo a ser funcional.

**2. Los supervisores tienen presencia visible en la UI**
Cuando un supervisor hace una pregunta, el usuario lo ve (badge), puede responder
en el chat principal o en un hilo dedicado, y el historial queda en el proyecto —
no en el supervisor efímero.

---

## Principios de diseño

**El supervisor es efímero. El proyecto no.**
El `ProjectLedger` (`project.json`) acumula entradas en texto libre — agnóstico
al dominio. Funciona igual para software, un curso, o una peluquería.

**Los specialists operan en su workspace. Paths externos requieren permiso explícito.**
`{AEGIS_DATA_DIR}/users/{tenant}/workspace/` es zona libre. Fuera de ahí,
el supervisor pide autorización al usuario via `ask_user` y llama `approve_path`.
La aprobación persiste en el enclave.

**`run_command` deshabilitado hasta post-launch.**
Con filesystem + web_search se cubre el 80% de los casos. Ejecución de comandos
requiere sandboxing — es post-launch debt.

---

## Arquitectura del flujo completo

```
Usuario: "trabajemos en el proyecto X"
    ↓
Chat Agent: spawn_agent(project_supervisor, "X", scope)
    ↓
ProjectSupervisor analiza el scope y decide la estructura:
    ├── spawn_agent(supervisor, "Área A")
    └── spawn_agent(supervisor, "Área B")
    ↓
Cada Supervisor descompone su área:
    └── spawn_agent(specialist, "tarea atómica")
    ↓
Specialist intenta leer archivo externo:
    → path_requires_approval
    ↓
Supervisor: ask_user("¿Autorizo acceso a /home/tavo/repo?")
    ├── Usuario responde en chat principal (CORE-263)
    └── Usuario responde en hilo dedicado (CORE-270 + CORE-271)
    ↓
Supervisor: approve_path("/home/tavo/repo")
Specialist reintenta → lee el archivo → ejecuta → report
    ↓
Supervisor consolida y reporta al ProjectSupervisor
ProjectSupervisor consolida y reporta al Chat Agent
Chat Agent informa al usuario en lenguaje natural
    ↓
Entradas clave se registran en ProjectLedger (project.json)
```

---

## Tickets

| ID | Título | Tipo | Asignado | Prioridad |
|---|---|---|---|---|
| **CORE-273** | Kernel: `ProjectLedger` — registro libre y persistente por proyecto | feat | KE | Crítica |
| **CORE-274** | WebSocket: contrato del envelope `agent_event` | feat | KE + SE | Crítica |
| **CORE-275** | Specialist: tools `read_file`, `write_file`, `list_files` | feat | KE | Crítica |
| **CORE-276** | Specialist: aprobación de paths externos por el usuario | feat | KE | Alta |
| **CORE-277** | Specialist: tool `web_search` | feat | KE | Alta |
| **CORE-268** | Kernel: emitir `AgentEvent` por WebSocket | feat | KE | Crítica |
| **CORE-269** | Shell: `AgentInbox` store + `AgentBadge` en nav | feat | SE | Crítica |
| **CORE-270** | Shell: ruta `/chat/agents` + componente `AgentThread` | feat | SE | Alta |
| **CORE-271** | Kernel: endpoint `POST /api/agents/:id/reply` | feat | KE | Alta |
| **CORE-272** | Kernel: herramienta `get_project_ledger` para chat_agent | feat | KE | Media |

## Orden de implementación

```
Bloque A — independientes entre sí, empezar en paralelo:
  CORE-273  ProjectLedger
  CORE-274  Contrato WS
  CORE-275  Specialist filesystem tools
    ↓
Bloque B — dependen de bloque A:
  CORE-276  Aprobación paths externos    (depende de CORE-275)
  CORE-277  web_search                   (depende de CORE-275)
  CORE-268  Kernel emite AgentEvents     (depende de CORE-273 + CORE-274)
    ↓
Bloque C:
  CORE-269  Shell badge                  (depende de CORE-268 + CORE-274) ← MVP UI
  CORE-271  Respuesta directa            (depende de CORE-268)
    ↓
Bloque D:
  CORE-270  Hilo dedicado UI             (depende de CORE-269 + CORE-271)
  CORE-272  chat_agent lee ledger        (depende de CORE-273)
```

## MVP funcional mínimo

**CORE-273 + CORE-275 + CORE-276** = specialists pueden trabajar en proyectos reales
con aprobación de paths externos.

**+ CORE-268 + CORE-269** = el usuario ve cuando los supervisores tienen preguntas.

El resto mejora la experiencia pero no bloquea el trabajo real.

---

## Post-launch debt (fuera de scope)

- `run_command` para specialists (requiere sandboxing)
- Persistencia del historial de chat entre sesiones (LanceDB / DT-003)
- Notificaciones push cuando el usuario está offline
- Historial del ledger exportable

---

*Creado: 2026-05-06 — Arquitecto IA*
*Rev 3: CORE-275/276/277 agregados (specialist tools). run_command → post-launch.*

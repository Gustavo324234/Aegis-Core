# EPIC 48 — Shell Observability: Chat con feedback real + Dashboard con datos reales

**Estado:** 📥 Planned  
**Prioridad:** Crítica  
**Responsable planning:** Arquitecto IA  
**Ingenieros:** Shell Engineer (todos los tickets salvo CORE-248 que es Kernel + Shell)

---

## Contexto

El smoke test del 2026-05-03 reveló dos problemas de observabilidad críticos:

**Problema 1 — Chat silencioso:** Cuando el kernel falla (cualquier error), el WebSocket no envía el `StatusUpdate` que desbloquea el stream. El usuario no ve nada — ni respuesta ni error. El chat queda en silencio indefinido. (Causa raíz ya documentada en CORE-244.)

Secundariamente, incluso cuando el modelo responde bien, el chat no le da al usuario ningún feedback sobre qué está pasando internamente: qué modelo se eligió, qué provider, si hubo un rate limit, si el proceso está en cola.

**Problema 2 — Dashboard con datos fake:** El `Dashboard.tsx` contiene datos hardcodeados que nunca se reemplazan con datos reales:
- `MOCK_TICKETS` — tickets de Aegis quemados en el código, no los del tenant
- `FinancialWidget` — gastos ficticios ($1,240.50, AWS, OpenAI)
- `Chronos` — eventos de calendario ficticios
- `"Welcome back, Operator"` — sin nombre real del tenant
- CPU Load — sí viene de `system_metrics` real, pero el barra está ahí sin contexto
- `"All enclaves secured"` — texto hardcodeado sin verificación real

Esto da una impresión de "demo permanente" que es incompatible con un lanzamiento público serio.

---

## Tickets del Epic

| ID | Tipo | Título | Asignado a | Prioridad |
|---|---|---|---|---|
| CORE-244 | fix | HAL Runner: StatusUpdate en path de error (ya creado) | Kernel Engineer | Crítica |
| CORE-248 | feat | Chat: indicador de estado enriquecido (modelo, provider, cola, error amigable) | Shell Engineer | Crítica |
| CORE-249 | feat | Dashboard: reemplazar MOCK_TICKETS con Kanban real del tenant | Shell Engineer | Alta |
| CORE-250 | feat | Dashboard: FinancialWidget conectado a Ledger Plugin real | Shell Engineer | Media |
| CORE-251 | feat | Dashboard: Chronos widget conectado a calendario real del tenant | Shell Engineer | Media |
| CORE-252 | feat | Dashboard: header con nombre real del tenant y estado del sistema real | Shell Engineer | Alta |

---

## Detalle de tickets

---

### CORE-248 — Chat: indicador de estado enriquecido

**Asignado a:** Shell Engineer  
**Componentes:** `shell/ui/src/components/ChatTerminal.tsx`, `shell/ui/src/store/useAegisStore.ts`

**Contexto:**
El chat actualmente muestra solo un dot de estado (`idle`, `thinking`, etc.) sin ningún detalle de qué está pasando. Cuando hay un error, el usuario ve silencio total. El store ya tiene `lastRoutingInfo` (model_id, provider, latency_ms) que nunca se muestra.

**Cambios requeridos:**

**1. Panel de estado activo** — visible debajo del header mientras `status === 'thinking'`, reemplaza el spinner estático actual:

```
┌──────────────────────────────────────────────────────────┐
│  ⚡ Procesando...   groq / llama-3.3-70b    ~1.2s        │
└──────────────────────────────────────────────────────────┘
```

Usar `lastRoutingInfo` del store para mostrar provider + modelo. Si `lastRoutingInfo` es null, mostrar solo "Procesando...".

**2. Mensaje de error amigable en el chat** — cuando `status === 'error'` y hay un nuevo error, inyectar un mensaje `role: 'assistant', type: 'error'` directamente en el hilo del chat con texto amigable:

```
❌ No pude procesar tu mensaje. El motor de IA no respondió.
   Intentá de nuevo en unos segundos.
```

No mostrar detalles técnicos al usuario. El error técnico sigue en el log del servidor.

**3. Estado en cola** — cuando el WebSocket envía `event: 'status'` con "Submitting task to ANK...", mostrar brevemente un badge "En cola..." antes de que arranque la inferencia.

**4. Eliminar el timeout de silencio** — si después de N segundos en estado `thinking` no llegó ningún token ni StatusUpdate, mostrar el mensaje de error amigable automáticamente (timeout de 30s). Esto es el safety net para CORE-244 mientras no esté deployado.

**Criterios de aceptación:**
- [ ] Mientras el modelo procesa, se ve provider + modelo en un banner compacto
- [ ] Si hay un error, aparece un mensaje amigable en el hilo del chat (no silencio)
- [ ] Si pasan 30s sin respuesta, se muestra el mensaje de error automáticamente
- [ ] El dot de estado (`StatusBadge`) sigue funcionando igual

---

### CORE-249 — Dashboard: Kanban real del tenant

**Asignado a:** Shell Engineer  
**Componente:** `shell/ui/src/components/Dashboard.tsx` — `KanbanColumn` + `MOCK_TICKETS`

**Contexto:**
El `MOCK_TICKETS` tiene 6 tickets hardcodeados de Aegis. El Kanban debería mostrar las tareas reales del tenant — los proyectos activos del `AgentOrchestrator` y sus estados.

**Cambios requeridos:**

Reemplazar `MOCK_TICKETS` con datos del store. El store ya tiene `activeProjects` (de `GET /api/agents/projects`). Cada proyecto puede tener un estado (`active`, `archived`) y un `root_agent_id`.

Mapear proyectos a cards de Kanban:
- Proyecto con `status: 'active'` y agente en estado `Running` → columna "Active Tasks"
- Proyecto con `status: 'active'` y agente en estado `Idle` o sin agente → columna "Backlog"
- Proyecto con `status: 'archived'` → columna "Verified"

Si no hay proyectos activos, mostrar un estado vacío con CTA: "Iniciá un proyecto diciéndole a tu agente qué querés hacer."

Eliminar completamente el array `MOCK_TICKETS` y la data hardcodeada.

**Criterios de aceptación:**
- [ ] El Kanban muestra proyectos reales del tenant, no datos ficticios
- [ ] Si no hay proyectos, se ve el estado vacío con el CTA
- [ ] El array `MOCK_TICKETS` no existe más en el código

---

### CORE-250 — Dashboard: FinancialWidget con datos reales

**Asignado a:** Shell Engineer  
**Componente:** `shell/ui/src/components/Dashboard.tsx` — `FinancialWidget`

**Contexto:**
El widget muestra $1,240.50, AWS y OpenAI hardcodeados. El Ledger Plugin (`SYS_CALL_PLUGIN("ledger", ...)`) existe pero no tiene un endpoint REST para consultar resumen.

**Dos opciones — elegir la pragmática:**

**Opción A (recomendada para MVP):** Reemplazar `FinancialWidget` con un widget de "API Cost" que muestra el costo real de tokens consumidos, disponible via `GET /api/status` (que ya tiene datos de telemetría). Mostrar tokens totales del período y costo estimado basado en el proveedor activo.

**Opción B (completa):** Crear `GET /api/ledger/summary` en el kernel que devuelva el balance/gastos del Ledger Plugin si está activo, y mostrarlo. Si el plugin no está activo, mostrar un placeholder "Plugin Ledger inactivo".

Implementar la opción que no requiera nuevo endpoint si los datos de telemetría son suficientes. Si no, implementar Opción B con un endpoint simple.

En cualquier caso: eliminar los valores ficticios ($1,240.50, AWS, OpenAI) del código.

**Criterios de aceptación:**
- [ ] El widget no muestra valores ficticios hardcodeados
- [ ] Si hay datos reales disponibles, se muestran
- [ ] Si no hay datos, se muestra un estado vacío o "Plugin inactivo" según corresponda

---

### CORE-251 — Dashboard: Chronos widget con eventos reales

**Asignado a:** Shell Engineer  
**Componente:** `shell/ui/src/components/Dashboard.tsx` — widget Chronos

**Contexto:**
El widget muestra "Sync Project Context IN 15 MIN" y "Backup Ring 0 Identity IN 2 HOURS" hardcodeados. El Plugin Chronos existe para gestión de calendario pero no hay endpoint para consultar eventos.

**Cambios requeridos:**

Opción pragmática para MVP: Si `GET /api/chronos/events` no existe, reemplazar el widget falso por un estado vacío honesto:

```
┌─────────────────────────────────────────────┐
│  🗓 Neural Schedule                          │
│  Sin eventos próximos.                       │
│  Pedile a tu agente que agende algo.         │
└─────────────────────────────────────────────┘
```

Si el endpoint existe o se crea como parte de este ticket, consumirlo y mostrar hasta 3 eventos próximos reales.

En cualquier caso: los eventos ficticios ("Sync Project Context", "Backup Ring 0 Identity") se eliminan del código.

**Criterios de aceptación:**
- [ ] No existen eventos hardcodeados en el código
- [ ] Si hay eventos reales disponibles, se muestran
- [ ] Si no hay eventos, se muestra el estado vacío con el mensaje descriptivo

---

### CORE-252 — Dashboard: header con nombre y estado reales

**Asignado a:** Shell Engineer  
**Componente:** `shell/ui/src/components/Dashboard.tsx` — header y welcome section

**Contexto:**
El header muestra "Welcome back, Operator" y "All enclaves secured" hardcodeados. El `tenantId` ya está disponible en el store. El nombre real del tenant (el configurado en onboarding) está en `GET /api/persona`.

**Cambios requeridos:**

**1. Saludo personalizado** — reemplazar "Welcome back, Operator" con el nombre del tenant:

```tsx
// Leer persona del tenant al montar el Dashboard
// GET /api/persona → { name: "Tavo", prompt: "..." }
// Mostrar: "Welcome back, Tavo" o si no hay nombre: "Welcome back, {tenantId}"
```

**2. Estado real del sistema** — reemplazar "All enclaves secured" con estado real. El `systemState` ya está en el store (`STATE_OPERATIONAL` / `STATE_INITIALIZING` / `UNKNOWN`). Mapear a texto legible:
- `STATE_OPERATIONAL` → "Kernel Operational // All systems nominal"
- `STATE_INITIALIZING` → "Kernel Initializing // Please wait"
- `UNKNOWN` → "System Status Unknown"

**3. CPU Load en el header** — el barra de CPU ya usa `system_metrics.cpu_load` real. Agregar el valor numérico (ej. "23%") junto a la barra para que sea legible de un vistazo.

**Criterios de aceptación:**
- [ ] El saludo muestra el nombre real del tenant (o tenantId como fallback)
- [ ] El texto de estado del sistema refleja `systemState` real del store
- [ ] El header de CPU muestra valor numérico además de la barra
- [ ] No hay strings hardcodeados de estado en el componente

---

## Orden de ejecución sugerido

1. **CORE-244** (ya creado — Kernel Engineer) — desbloquea el smoke test
2. **CORE-248** (Shell) — chat usable en cualquier escenario
3. **CORE-252** (Shell) — cambio mínimo, alto impacto visual
4. **CORE-249** (Shell) — Kanban real
5. **CORE-250** (Shell) — Financial widget honesto
6. **CORE-251** (Shell) — Chronos honesto

CORE-244 y CORE-248 son Críticos y deben ir primero en el mismo PR si es posible. El resto puede ir en PRs separados.

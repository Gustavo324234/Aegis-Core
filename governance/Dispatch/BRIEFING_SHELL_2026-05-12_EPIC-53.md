# BRIEFING — Shell Engineer
## EPIC 53 — Stabilization: Fase 2 + Fase 3 + Fase 4 (Shell)
**Fecha:** 2026-05-12
**Branch base:** `feat/epic-53-stabilization-shell`

---

## Contexto

Primera sesión real de uso productivo detectó bugs en observabilidad del dashboard,
timeouts desincronizados y widgets que no cargan. Este briefing cubre todos los
tickets de shell del EPIC 53.

**Orden sugerido:** CORE-299 → CORE-301 → CORE-247 → CORE-249 → CORE-252 →
CORE-245 → CORE-246 → CORE-256 → CORE-250 → CORE-251 → CORE-294

Cada ticket va en su propio branch y PR.

---

## CORE-299 — Timeout del cliente desincronizado con el ReAct loop
**Branch:** `fix/core-299-chat-timeout-alignment`
**Prioridad:** Alta — implementar primero

El usuario ve "El motor tardó demasiado en responder" pero el kernel completó
el proceso exitosamente ~7-30 segundos después. El ReAct loop con herramientas
tarda entre 20-60 segundos. El cliente corta antes.

### Cambios

**1. Aumentar el timeout de espera a 120 segundos**

Localizar la constante de timeout en el store o service de chat:
```typescript
// Buscar algo como:
const CHAT_TIMEOUT_MS = 20_000; // o similar

// Cambiar a:
const CHAT_TIMEOUT_MS = 120_000; // 2 minutos
```

**2. Indicador visual progresivo**

```typescript
// Estados del mensaje mientras espera:
// 0-5s   → nada (respuesta rápida normal)
// 5-120s → spinner + "Procesando..." en el mensaje pendiente
// >120s  → error "El motor tardó demasiado en responder"
```

Construir sobre lo implementado en CORE-248 (indicador de estado enriquecido).

**3. Verificar si hay timeout en el fetch/WebSocket del chat**

Si se usa `fetch` con `signal: AbortSignal.timeout(X)`, aumentar X a 130s
(siempre mayor que el timeout visual para evitar race condition).

### Criterios
- [ ] ReAct loop de hasta 2 minutos no muestra error de timeout
- [ ] Después de 5s sin respuesta aparece indicador visual
- [ ] El error solo aparece si realmente supera los 2 minutos
- [ ] `npm run build` pasa

### Commit
```
fix(ui): CORE-299 align chat timeout with ReAct loop — 120s + progressive loading indicator
```

---

## CORE-301 — AgentTreeWidget unavailable
**Branch:** `fix/core-301-agent-tree-widget`
**Prioridad:** Alta

El dashboard muestra `AGENTTREEWIDGET UNAVAILABLE` siempre, incluso cuando
no hay error real — simplemente no hay agentes activos.

### Cambios

**1. Separar estados correctamente**

```typescript
type AgentTreeState =
  | { status: 'connecting' }
  | { status: 'connected'; agents: AgentNode[] }
  | { status: 'empty' }               // conectado, sin agentes — NO es error
  | { status: 'error'; message: string } // fallo real de conexión

// Render según estado:
// 'connecting' → spinner
// 'empty'      → "Sin agentes activos. Iniciá un proyecto para ver el árbol."
// 'error'      → "UNAVAILABLE" + RETRY
```

**2. Retry con backoff antes de mostrar error**

```typescript
const MAX_RETRIES = 3;
let retryCount = 0;

const connect = () => {
  const ws = new WebSocket(`/ws/agents/${tenantId}`);
  ws.onerror = () => {
    if (retryCount < MAX_RETRIES) {
      retryCount++;
      setTimeout(connect, 1000 * Math.pow(2, retryCount - 1));
    } else {
      setState({ status: 'error', message: 'No se pudo conectar al stream de agentes' });
    }
  };
  ws.onopen = () => { retryCount = 0; setState({ status: 'connecting' }); };
  ws.onmessage = (e) => {
    const agents = JSON.parse(e.data);
    setState(agents.length > 0
      ? { status: 'connected', agents }
      : { status: 'empty' }
    );
  };
};
```

**3. El botón RETRY reconecta efectivamente**

```typescript
const handleRetry = () => {
  retryCount = 0;
  setState({ status: 'connecting' });
  connect();
};
```

### Criterios
- [ ] Con agentes activos: muestra el árbol
- [ ] Sin agentes: muestra mensaje informativo (no UNAVAILABLE)
- [ ] Error real: muestra UNAVAILABLE + RETRY tras 3 reintentos
- [ ] RETRY reconecta efectivamente
- [ ] `npm run build` pasa

### Commit
```
fix(ui): CORE-301 AgentTreeWidget — differentiate empty vs error state, retry with backoff
```

---

## CORE-247 — Historial de chat persistente
**Branch:** `feat/core-247-persistent-chat-history`
**Prioridad:** CRÍTICA

El historial de chat se pierde al refrescar o al acceder desde una URL diferente
(IP vs Cloudflare). Ver ticket `governance/Tickets/CORE-247.md` para detalles.

Al conectar, cargar el historial existente del tenant desde el kernel.
Unificar la identidad del tenant independientemente de si accede por IP o tunnel.

### Commit
```
feat(ui): CORE-247 load persistent chat history on connect, unify IP and Cloudflare identity
```

---

## CORE-249 — Dashboard: Kanban real del tenant
**Branch:** `feat/core-249-dashboard-real-kanban`
**Prioridad:** Alta

Reemplazar `MOCK_TICKETS` con los proyectos y tareas reales del tenant.
El endpoint `GET /api/agents/projects` ya existe y devuelve los proyectos activos.
Mapear proyectos → columnas Backlog / Active / Verified según el estado del supervisor.

### Criterios
- [ ] El Kanban muestra proyectos reales del tenant
- [ ] No hay datos hardcodeados o mock visibles
- [ ] Si no hay proyectos: muestra estado vacío con call-to-action
- [ ] `npm run build` pasa

### Commit
```
feat(ui): CORE-249 replace MOCK_TICKETS with real tenant project Kanban
```

---

## CORE-252 — Dashboard: header con datos reales
**Branch:** `feat/core-252-dashboard-real-header`
**Prioridad:** Alta

El header del dashboard muestra datos hardcodeados. Reemplazar con:
- Nombre real del tenant (desde el contexto de autenticación)
- Estado real del kernel (`GET /api/system/status` o `/health`)

### Criterios
- [ ] El nombre del tenant en el header es el real, no hardcodeado
- [ ] El estado "KERNEL OPERATIONAL" refleja el health check real
- [ ] `npm run build` pasa

### Commit
```
feat(ui): CORE-252 dashboard header — real tenant name and kernel status
```

---

## CORE-245 — Admin: toggle habilitar/deshabilitar provider
**Branch:** `feat/core-245-provider-toggle`
**Prioridad:** Alta

Agregar toggle en el panel de admin para habilitar/deshabilitar un provider
sin eliminarlo. El provider deshabilitado no aparece en el CMR ni en el
CatalogViewer del tenant.

Ver ticket `governance/Tickets/CORE-245.md`.

### Commit
```
feat(ui): CORE-245 admin provider toggle — enable/disable without deleting
```

---

## CORE-246 — Tenant: modelos disponibles por provider en tab Motor
**Branch:** `feat/core-246-tenant-model-catalog`
**Prioridad:** Alta

El tab Motor del tenant debe mostrar los modelos disponibles agrupados por
provider, con sus capacidades (supports_tools, context_window, cost).

Ver ticket `governance/Tickets/CORE-246.md`.

### Commit
```
feat(ui): CORE-246 tenant motor tab — model catalog grouped by provider
```

---

## CORE-256 — Admin: tab Sistema — gestión del servicio
**Branch:** `feat/core-256-system-tab`
**Prioridad:** Alta — coordinar schema con Kernel Engineer primero

UI para gestionar el servicio desde el panel de admin. El Kernel Engineer
implementa los endpoints en paralelo.

**Coordinar schema antes de implementar:**
```typescript
// GET /api/system/service/status
interface ServiceStatus {
  status: 'running' | 'stopped';
  uptime_secs: number;
  pid: number;
}
```

Mostrar en un tab "Sistema" del panel de admin:
- Estado actual (running/stopped con badge de color)
- Uptime
- Botones: Restart / Stop (con confirmación modal)

### Commit
```
feat(ui): CORE-256 admin system tab — service status, restart and stop controls
```

---

## CORE-250 + CORE-251 — Dashboard: widgets honestos
**Branch:** `feat/core-250-251-dashboard-honest-widgets`
**Prioridad:** Media

**CORE-250 — FinancialWidget:** mostrar costo real de API de la sesión actual.
Si el kernel no tiene datos de costo disponibles, mostrar "Sin datos de consumo
para esta sesión" (ya es correcto) pero sin el mensaje "Activá el Plugin Ledger"
si Ledger no existe como plugin real.

**CORE-251 — Chronos widget:** mostrar solo eventos reales del scheduler.
Si no hay eventos programados, mostrar "Sin eventos próximos" limpio, sin
sugerir acciones ficticias.

### Commit
```
feat(ui): CORE-250-251 dashboard financial and chronos widgets — honest empty states
```

---

## CORE-294 — CatalogViewer: columna Benchmark + badges
**Branch:** `feat/core-294-catalog-bench-column`
**Prioridad:** Alta

**Nota importante:** El CMR ya usa los `task_scores` internamente con peso 40%.
Este ticket solo expone visualmente lo que el router ya sabe.

Los datos ya están en `GET /api/providers/models` → campo `task_scores`.
No se necesita ningún cambio en el kernel.

### Columna "Bench"

```typescript
const avgScore = (scores: TaskScores): number => {
  const fields = ['chat', 'coding', 'planning', 'analysis', 'summarization', 'extraction'];
  const values = fields.map(f => scores[f] ?? 0);
  return Math.round((values.reduce((a, b) => a + b, 0) / fields.length) * 10) / 10;
};

// Render: barra de 5 segmentos
// 1-2: rojo · 3: amarillo · 4: verde claro · 5: verde
// Si promedio === 0: mostrar "—"
```

### Badges

```typescript
// provider === "ollama_cloud" → chip "☁ Cloud"
// is_local === true           → chip "⚡ Local"
```

### Tooltip

```
Score calculado desde PinchBench (benchmark de agentes reales)
Actualizado: <last_synced>
Usado por el router para selección automática de modelos.
```

### Criterios
- [ ] Columna "Bench" visible en tab Motor
- [ ] Modelos sin score muestran "—"
- [ ] Barra de color correcta por rango
- [ ] Badges Local y Cloud visibles
- [ ] Tooltip en hover
- [ ] UI no rompe si `task_scores` es null
- [ ] `npm run build` pasa

### Commit
```
feat(ui): CORE-294 CatalogViewer bench score column + local/cloud badges
```

---

**No pushear a main. Un PR por ticket.**

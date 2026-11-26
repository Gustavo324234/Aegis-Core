# CORE-166 — feat: Agent Tree Widget — Visualización del Árbol de Agentes en Dashboard del Tenant

**Epic:** 43 — Hierarchical Multi-Agent Orchestration  
**Repo:** Aegis-Core / shell/ui  
**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Alta  
**Asignado a:** Shell Engineer  
**Dependencias:** CORE-163 (backend HTTP /api/agents/tree) — puede usarse con mock hasta que esté listo

---

## Contexto

El tenant tiene su propio `Dashboard.tsx` con un grid de 12 columnas que ya incluye `FinancialWidget`, `Neural Schedule` y el `Kanban Board`. Esta visualización va como una **nueva sección en ese mismo Dashboard**, debajo del Kanban, con el mismo estilo visual del resto del componente.

El widget permite al usuario ver en tiempo real qué agentes cognitivos están activos trabajando en sus proyectos, su estado y su jerarquía.

---

## Cambios Requeridos

### 1. Nuevo archivo: `shell/ui/src/components/AgentTreeWidget.tsx`

Componente standalone que encapsula toda la lógica del widget.

#### Types

```typescript
type AgentRole = 'ProjectSupervisor' | 'DomainSupervisor' | 'Specialist';
type AgentStateKind = 'Idle' | 'Running' | 'WaitingReport' | 'Complete' | 'Failed';

interface AgentNodeDto {
  agent_id: string;
  role: AgentRole;
  project_id: string;
  domain: string;
  parent_id: string | null;
  children: string[];
  state: AgentStateKind | string;
  task_type: string;
  created_at: string;
}

interface AgentTreeDto {
  nodes: AgentNodeDto[];
  roots: string[];
  total_agents: number;
}
```

#### Mock data para desarrollo independiente

```typescript
const MOCK_TREE: AgentTreeDto = {
  total_agents: 6,
  roots: ['proj-aegis'],
  nodes: [
    { agent_id: 'proj-aegis', role: 'ProjectSupervisor', project_id: 'aegis', domain: 'Aegis OS', parent_id: null, children: ['dom-kernel', 'dom-shell'], state: 'WaitingReport', task_type: 'PLANNING', created_at: new Date().toISOString() },
    { agent_id: 'dom-kernel', role: 'DomainSupervisor', project_id: 'aegis', domain: 'Kernel Engineer', parent_id: 'proj-aegis', children: ['spec-scheduler', 'spec-auth'], state: 'WaitingReport', task_type: 'CODE', created_at: new Date().toISOString() },
    { agent_id: 'dom-shell', role: 'DomainSupervisor', project_id: 'aegis', domain: 'Shell Engineer', parent_id: 'proj-aegis', children: ['spec-ui'], state: 'Running', task_type: 'CODE', created_at: new Date().toISOString() },
    { agent_id: 'spec-scheduler', role: 'Specialist', project_id: 'aegis', domain: 'scheduler.rs', parent_id: 'dom-kernel', children: [], state: 'Complete', task_type: 'CODE', created_at: new Date().toISOString() },
    { agent_id: 'spec-auth', role: 'Specialist', project_id: 'aegis', domain: 'citadel auth', parent_id: 'dom-kernel', children: [], state: 'Running', task_type: 'CODE', created_at: new Date().toISOString() },
    { agent_id: 'spec-ui', role: 'Specialist', project_id: 'aegis', domain: 'AgentTreeWidget.tsx', parent_id: 'dom-shell', children: [], state: 'Running', task_type: 'CODE', created_at: new Date().toISOString() },
  ]
};
```

#### Fetch con polling

```typescript
// Polling cada 2 segundos a GET /api/agents/tree
// Headers: x-citadel-tenant + x-citadel-key (mismo patrón que SystemTab en AdminDashboard)
// Si el fetch falla (endpoint aún no existe), usar MOCK_TREE silenciosamente
// Cleanup del interval en return del useEffect
const fetchTree = async () => {
  try {
    const res = await fetch('/api/agents/tree', {
      headers: {
        'x-citadel-tenant': tenantId!,
        'x-citadel-key': sessionKey!,
      }
    });
    if (res.ok) setTree(await res.json());
    else setTree(MOCK_TREE); // fallback a mock
  } catch {
    setTree(MOCK_TREE); // fallback a mock
  }
};
```

#### Lógica de renderizado

El widget construye el árbol de forma recursiva a partir de la lista plana de nodos:

```typescript
// Construye un mapa id → nodo y renderiza recursivamente desde los roots
const nodeMap = useMemo(() => {
  const map = new Map<string, AgentNodeDto>();
  tree.nodes.forEach(n => map.set(n.agent_id, n));
  return map;
}, [tree]);
```

#### Indicadores de estado

```typescript
const STATE_CONFIG: Record<string, { color: string; pulse: boolean; label: string }> = {
  'Idle':          { color: 'text-white/30',      pulse: false, label: 'IDLE' },
  'Running':       { color: 'text-aegis-cyan',    pulse: true,  label: 'RUNNING' },
  'WaitingReport': { color: 'text-yellow-400',    pulse: true,  label: 'WAITING' },
  'Complete':      { color: 'text-green-400',     pulse: false, label: 'DONE' },
  'Failed':        { color: 'text-red-400',       pulse: false, label: 'FAILED' },
};
```

#### Iconos por rol

```
ProjectSupervisor  → 🏢  o <Building2> de lucide-react
DomainSupervisor   → 🔧  o <Wrench> de lucide-react
Specialist         → <Zap> de lucide-react (ya importado en el proyecto)
```

#### Layout visual del nodo

```
[icono-rol] [domain name]  ········  [task_type badge]  [estado dot + label]
```

Para nodos con hijos: línea vertical izquierda tipo tree-view usando `border-l border-white/10 ml-3 pl-3`.

#### Estado vacío

Cuando `total_agents === 0`:
```
[ icono Bot ]
No hay agentes activos
Los agentes se activarán cuando inicies una tarea compleja
```

#### Props del componente

```typescript
interface AgentTreeWidgetProps {
  tenantId: string | null;
  sessionKey: string | null;
}
```

---

### 2. Modificar `shell/ui/src/components/Dashboard.tsx`

**Cambios mínimos:**

a) Agregar import:
```typescript
import AgentTreeWidget from './AgentTreeWidget';
```

b) Agregar la sección al final de `<main>`, después del bloque del Kanban Board, dentro del mismo `grid grid-cols-12 gap-8`:

```tsx
{/* Agent Tree — Epic 43 */}
<div className="col-span-12 space-y-6">
  <div className="flex items-center gap-3">
    <Bot className="w-5 h-5 text-aegis-cyan" />  {/* Bot de lucide-react */}
    <h2 className="text-xl font-bold uppercase tracking-widest">Active Agents</h2>
    <span className="text-[10px] font-mono text-white/20 uppercase tracking-widest ml-2">— Hierarchical Orchestration</span>
  </div>
  <AgentTreeWidget tenantId={tenantId} sessionKey={sessionKey} />
</div>
```

c) Obtener `tenantId` y `sessionKey` del store en el componente `Dashboard`:
```typescript
const { setCurrentView, system_metrics, tenantId, sessionKey } = useAegisStore();
```

d) Agregar `Bot` al import de lucide-react existente.

---

## Comportamiento esperado

- Si no hay agentes activos → muestra estado vacío (no un error)
- Si el backend aún no tiene CORE-163 → cae silenciosamente al mock y muestra el árbol de ejemplo
- El polling se detiene cuando el componente se desmonta
- Los nodos `Running` y `WaitingReport` tienen animación de pulso en su indicador de estado
- Nodos indentados visualmente según su nivel en la jerarquía (ProjectSupervisor → DomainSupervisor → Specialist)
- El widget tiene el mismo estilo `glass` + `border border-white/10` + `rounded-2xl` que el resto del Dashboard

---

## Criterios de Aceptación

- [ ] `AgentTreeWidget` renderiza sin errores con `total_agents === 0`
- [ ] `AgentTreeWidget` renderiza correctamente el mock de 6 agentes en 3 niveles
- [ ] La indentación visual respeta la jerarquía: cada nivel tiene más margen izquierdo
- [ ] Los estados `Running` y `WaitingReport` tienen dot animado (animate-pulse)
- [ ] El estado `Complete` muestra el dot en verde sin animación
- [ ] El fetch usa los headers `x-citadel-tenant` y `x-citadel-key` correctos
- [ ] El interval del polling se limpia al desmontar el componente
- [ ] La nueva sección aparece en `Dashboard.tsx` debajo del Kanban Board
- [ ] `npm run build` pasa sin errores TypeScript ni warnings de ESLint

---

*Arquitecto IA — 2026-04-24*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: AgentTreeView.tsx existe y se renderiza en el Dashboard.

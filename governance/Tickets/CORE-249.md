# CORE-249 — Dashboard: reemplazar MOCK_TICKETS con Kanban real del tenant

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** EPIC 48 — Shell Observability  
**Asignado a:** Shell Engineer  
**Componente:** `shell/ui/src/components/Dashboard.tsx`

**Estado:** ✅ Done
---

## Contexto

El `Dashboard.tsx` contiene un array `MOCK_TICKETS` con 6 tickets hardcodeados de Aegis-Core. El Kanban muestra siempre esos mismos datos sin importar quién es el tenant ni qué proyectos tiene. Esto hace que el dashboard parezca permanentemente una demo.

El store ya tiene `activeProjects: ProjectSummary[]` y `agentTree: AgentNodeSummary[]` con datos reales del tenant.

---

## Cambios requeridos

### Eliminar `MOCK_TICKETS`

Borrar completamente el array `MOCK_TICKETS` y el tipo `Ticket` de `Dashboard.tsx`.

### Mapear proyectos reales a cards de Kanban

Usar `activeProjects` y `agentTree` del store para generar las cards dinámicamente.

Lógica de columna por proyecto:
- Proyecto con `status: 'active'` cuyo `root_agent_id` apunta a un agente con estado `Running` → **Active Tasks**
- Proyecto con `status: 'active'` sin agente activo o agente en `Idle` → **Backlog**
- Proyecto con `status: 'archived'` → **Verified/Done**

Estructura de cada card (adaptada del diseño existente):

```tsx
interface ProjectCard {
  id: string;          // project_id
  title: string;       // project name
  description: string | null;
  status: 'active' | 'archived';
  agentState: AgentNodeState | null;  // estado del root_agent si existe
  model: string | null;               // modelo que está usando el agente
}
```

Mostrar en la card:
- Nombre del proyecto (title)
- Descripción truncada a 2 líneas si existe
- Badge del estado del agente raíz (Running → cyan pulsante, Idle → gris, Complete → verde)
- Modelo del agente si está disponible (texto pequeño debajo)

### Estado vacío con CTA

Si `activeProjects` está vacío o cargando, mostrar en cada columna un estado descriptivo:

```tsx
// En KanbanColumn, cuando no hay items:
<div className="flex flex-col items-center justify-center py-12 gap-3 text-center">
  <p className="text-[10px] font-mono text-white/20 uppercase tracking-widest">Sin proyectos</p>
  <p className="text-[11px] font-mono text-white/30 max-w-[200px] leading-relaxed">
    Iniciá un proyecto diciéndole a tu agente qué querés construir.
  </p>
</div>
```

### Actualización en tiempo real

`agentTree` ya se actualiza via WebSocket (`connectAgentStream`). El Kanban debe re-renderizar automáticamente cuando cambian los estados de los agentes — esto funciona gratis con Zustand si el componente suscribe a `agentTree` y `activeProjects`.

---

## Criterios de aceptación

- [x] `MOCK_TICKETS` y el tipo `Ticket` eliminados del código.
- [x] El Kanban muestra proyectos reales del tenant.
- [x] El estado de cada card refleja el estado real del agente raíz del proyecto.
- [x] Si no hay proyectos, cada columna muestra el estado vacío con CTA.
- [x] Los cambios de estado de agentes se reflejan en tiempo real (sin recarga).
- [x] TypeScript sin errores de compilación.

## Dependencias

- `activeProjects` y `agentTree` en el store (ya implementados, CORE-203/CORE-204)

# CORE-164 — feat: AgentTreeView + AgentCard — Visualización del Árbol en Shell

**Epic:** 43 — Hierarchical Multi-Agent Orchestration  
**Repo:** Aegis-Core / shell/ui  
**Tipo:** feat  
**Prioridad:** Media  
**Asignado a:** Shell Engineer  
**Dependencias:** CORE-163
**Estado:** ✅ Completo

---

## Contexto

El usuario debe poder **ver qué está pasando** en la jerarquía de agentes mientras trabaja. Este ticket implementa el componente de visualización en la Shell: un árbol colapsable que muestra en tiempo real qué agentes están activos, en qué estado y qué están haciendo.

La transparencia del sistema es un valor de Aegis (ADR-AGENTS-007).

---

## Cambios Requeridos

### 1. Nuevo componente: `shell/ui/src/components/AgentTreeView.tsx`

Árbol colapsable con los siguientes requerimientos de UX:
- Los nodos se agrupan por proyecto (un bloque por `project_id`)
- Dentro de cada proyecto, se muestran los `DomainSupervisors` y sus `Specialists`
- El estado de cada agente se muestra con un indicador visual:
  - `Idle` → gris
  - `Running` → azul pulsante (animación similar a TheOrb)
  - `WaitingReport` → amarillo
  - `Complete` → verde
  - `Failed` → rojo
- El árbol se actualiza via polling cada 2 segundos al endpoint `GET /api/agents/tree`
- Nodos colapsables con click (ocultar/mostrar hijos)

### 2. Nuevo componente: `shell/ui/src/components/AgentCard.tsx`

Card individual para un agente con:
- Nombre del dominio + icono según rol (🏢 Supervisor de Proyecto, 🔧 Supervisor de Dominio, ⚡ Especialista)
- Modelo asignado (si disponible desde el tree DTO)
- Estado actual
- Tiempo transcurrido desde creación

### 3. Integración en el layout

Agregar `AgentTreeView` como panel lateral o pestaña en el `TelemetryDashboard.tsx` existente. No debe reemplazar la telemetría actual sino complementarla.

### 4. Fetch helper

```typescript
// src/services/agentService.ts

export interface AgentNodeDto {
  agent_id: string;
  role: 'ProjectSupervisor' | 'DomainSupervisor' | 'Specialist';
  project_id: string;
  domain: string;
  parent_id: string | null;
  children: string[];
  state: 'Idle' | 'Running' | 'WaitingReport' | 'Complete' | string;
  task_type: string;
  created_at: string;
}

export interface AgentTreeDto {
  nodes: AgentNodeDto[];
  roots: string[];
  total_agents: number;
}

export async function fetchAgentTree(sessionKey: string): Promise<AgentTreeDto> { ... }
```

---

## Criterios de Aceptación

- [x] `AgentTreeView` renderiza sin errores cuando `total_agents === 0` (estado vacío con mensaje "No hay agentes activos")
- [x] El polling se detiene cuando el componente se desmonta (cleanup del `useEffect`)
- [x] Los nodos se agrupan correctamente por `project_id`
- [x] Hacer click en un nodo padre colapsa/expande sus hijos
- [x] El indicador de estado `Running` tiene la animación de pulso
- [x] El componente está integrado en `TelemetryDashboard` sin romper la telemetría existente
- [x] `npm run build` pasa sin errores TypeScript

---

*Arquitecto IA — 2026-04-24*
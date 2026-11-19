# CORE-203 — feat: AgentTreeView + ProjectList — panel en Dashboard

**Epic:** 45 — Cognitive Agent Architecture  
**Repo:** Aegis-Core / shell/ui  
**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Media  
**Asignado a:** Shell Engineer  
**Dependencias:** CORE-204 (agentTree + activeProjects en store)

---

## Contexto

El Dashboard existente (`TelemetryDashboard.tsx`) muestra métricas del sistema.
Este ticket agrega dos nuevos paneles al Dashboard:

1. **ProjectList** — lista de proyectos activos del tenant con estado resumido
2. **AgentTreeView** — árbol completo de agentes de un proyecto seleccionado,
   con modelo asignado, estado y último reporte. Drawer al hacer click en un nodo.

Primero leer `TelemetryDashboard.tsx` para entender la estructura de paneles
existente y mantener consistencia visual.

---

## Nuevo archivo: `shell/ui/src/components/ProjectList.tsx`

```tsx
import React from 'react';
import { FolderOpen, FolderArchive, Circle, Loader2 } from 'lucide-react';
import { useAegisStore, ProjectSummary } from '../store/useAegisStore';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface ProjectListProps {
    onSelectProject: (projectId: string) => void;
    selectedProjectId: string | null;
}

export const ProjectList: React.FC<ProjectListProps> = ({ onSelectProject, selectedProjectId }) => {
    const { activeProjects, agentTree } = useAegisStore();

    if (activeProjects.length === 0) {
        return (
            <div className="flex flex-col items-center justify-center py-8 gap-2">
                <FolderOpen className="w-6 h-6 text-white/10" />
                <p className="text-[10px] font-mono text-white/20 uppercase tracking-widest">No active projects</p>
            </div>
        );
    }

    const getProjectAgentCount = (project: ProjectSummary) => {
        if (!project.root_agent_id) return 0;
        // Contar todos los nodos que pertenecen a este proyecto (raíz + descendientes)
        const countDescendants = (nodeId: string): number => {
            const node = agentTree.find(n => n.id === nodeId);
            if (!node) return 0;
            return 1 + node.children.reduce((acc, childId) => acc + countDescendants(childId), 0);
        };
        return countDescendants(project.root_agent_id);
    };

    const getProjectStatus = (project: ProjectSummary): 'active' | 'idle' | 'archived' => {
        if (project.status === 'archived') return 'archived';
        const agentCount = getProjectAgentCount(project);
        if (agentCount > 0) {
            const root = agentTree.find(n => n.id === project.root_agent_id);
            if (root && (root.state === 'Running' || root.state === 'WaitingReport')) return 'active';
        }
        return 'idle';
    };

    return (
        <div className="flex flex-col gap-1">
            {activeProjects.map(project => {
                const status = getProjectStatus(project);
                const agentCount = getProjectAgentCount(project);
                const isSelected = project.project_id === selectedProjectId;

                return (
                    <button
                        key={project.project_id}
                        onClick={() => onSelectProject(project.project_id)}
                        className={cn(
                            "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg transition-all text-left",
                            isSelected
                                ? "bg-aegis-cyan/10 border border-aegis-cyan/20"
                                : "hover:bg-white/5 border border-transparent"
                        )}
                    >
                        {status === 'archived'
                            ? <FolderArchive className="w-4 h-4 text-white/20 shrink-0" />
                            : <FolderOpen className={cn("w-4 h-4 shrink-0", isSelected ? "text-aegis-cyan" : "text-white/40")} />
                        }

                        <div className="flex-1 min-w-0">
                            <p className={cn(
                                "text-[11px] font-mono truncate",
                                isSelected ? "text-aegis-cyan" : "text-white/70"
                            )}>
                                {project.name}
                            </p>
                            {project.description && (
                                <p className="text-[9px] font-mono text-white/20 truncate mt-0.5">
                                    {project.description}
                                </p>
                            )}
                        </div>

                        <div className="flex items-center gap-2 shrink-0">
                            {agentCount > 0 && (
                                <span className="text-[9px] font-mono text-white/30">
                                    {agentCount} agent{agentCount !== 1 ? 's' : ''}
                                </span>
                            )}
                            {status === 'active' && (
                                <Loader2 className="w-3 h-3 text-aegis-cyan animate-spin" />
                            )}
                            {status === 'idle' && (
                                <Circle className="w-2 h-2 text-green-400/50 fill-green-400/30" />
                            )}
                            {status === 'archived' && (
                                <Circle className="w-2 h-2 text-white/10" />
                            )}
                        </div>
                    </button>
                );
            })}
        </div>
    );
};
```

---

## Nuevo archivo: `shell/ui/src/components/AgentTreeView.tsx`

```tsx
import React, { useState } from 'react';
import { ChevronRight, ChevronDown, CheckCircle2, Circle, Loader2, AlertCircle, X } from 'lucide-react';
import { useAegisStore, AgentNodeSummary, AgentRole } from '../store/useAegisStore';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

function roleName(role: AgentRole): string {
    switch (role.type) {
        case 'ChatAgent': return 'Chat Agent';
        case 'ProjectSupervisor': return role.project_id;
        case 'Supervisor': return role.name;
        case 'Specialist': return role.scope.length > 40 ? role.scope.slice(0, 40) + '…' : role.scope;
    }
}

function roleLabel(role: AgentRole): string {
    switch (role.type) {
        case 'ChatAgent': return 'Chat';
        case 'ProjectSupervisor': return 'Project Supervisor';
        case 'Supervisor': return 'Supervisor';
        case 'Specialist': return 'Specialist';
    }
}

function stateColor(state: AgentNodeSummary['state']): string {
    if (state === 'Running' || state === 'WaitingReport' || state === 'WaitingQuery') return 'text-aegis-cyan';
    if (state === 'Complete') return 'text-green-400';
    if (typeof state === 'object' && 'Failed' in state) return 'text-red-400';
    return 'text-white/20';
}

function StateIcon({ state }: { state: AgentNodeSummary['state'] }) {
    if (state === 'Running' || state === 'WaitingReport' || state === 'WaitingQuery') {
        return <Loader2 className="w-3.5 h-3.5 animate-spin shrink-0 text-aegis-cyan" />;
    }
    if (state === 'Complete') return <CheckCircle2 className="w-3.5 h-3.5 shrink-0 text-green-400" />;
    if (typeof state === 'object' && 'Failed' in state) return <AlertCircle className="w-3.5 h-3.5 shrink-0 text-red-400" />;
    return <Circle className="w-3.5 h-3.5 shrink-0 text-white/20" />;
}

// Drawer de detalle de un nodo
const NodeDetailDrawer: React.FC<{
    node: AgentNodeSummary;
    onClose: () => void;
}> = ({ node, onClose }) => (
    <div className="fixed inset-0 z-50 flex justify-end">
        <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />
        <div className="relative w-full max-w-sm h-full bg-black border-l border-white/10 flex flex-col overflow-hidden">
            <div className="flex items-center justify-between px-4 py-3 border-b border-white/10 shrink-0">
                <div>
                    <p className="text-[11px] font-mono text-white/80">{roleName(node.role)}</p>
                    <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mt-0.5">{roleLabel(node.role)}</p>
                </div>
                <button onClick={onClose} className="p-1.5 rounded hover:bg-white/10 text-white/40 transition-colors">
                    <X className="w-4 h-4" />
                </button>
            </div>
            <div className="flex-1 overflow-y-auto p-4 space-y-4">
                <div>
                    <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mb-1">State</p>
                    <div className={cn("flex items-center gap-2 text-[11px] font-mono", stateColor(node.state))}>
                        <StateIcon state={node.state} />
                        {typeof node.state === 'string' ? node.state : `Failed: ${node.state.Failed.reason}`}
                    </div>
                </div>
                {node.model && (
                    <div>
                        <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mb-1">Model</p>
                        <p className="text-[11px] font-mono text-aegis-purple/80">{node.model}</p>
                    </div>
                )}
                {node.activity && (
                    <div>
                        <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mb-1">Current Activity</p>
                        <p className="text-[11px] font-mono text-aegis-cyan/60 italic">{node.activity}</p>
                    </div>
                )}
                {node.last_report && (
                    <div>
                        <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mb-2">Last Report</p>
                        <div className="bg-white/5 rounded-lg p-3 border border-white/10">
                            <p className="text-[11px] font-mono text-white/60 whitespace-pre-wrap">{node.last_report}</p>
                        </div>
                    </div>
                )}
            </div>
        </div>
    </div>
);

// Fila del árbol
const TreeRow: React.FC<{
    node: AgentNodeSummary;
    allNodes: AgentNodeSummary[];
    depth: number;
    onSelect: (node: AgentNodeSummary) => void;
}> = ({ node, allNodes, depth, onSelect }) => {
    const [expanded, setExpanded] = useState(true);
    const children = allNodes.filter(n => node.children.includes(n.id));
    const hasChildren = children.length > 0;

    return (
        <div>
            <div
                className="flex items-center gap-2 py-1.5 rounded-lg px-2 group transition-colors cursor-pointer hover:bg-white/5"
                style={{ paddingLeft: `${depth * 16 + 8}px` }}
                onClick={(e) => {
                    if (hasChildren) {
                        e.stopPropagation();
                        setExpanded(ex => !ex);
                    }
                    onSelect(node);
                }}
            >
                <span className="w-3.5 h-3.5 shrink-0 text-white/20">
                    {hasChildren
                        ? expanded ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />
                        : null
                    }
                </span>

                <StateIcon state={node.state} />

                <div className="flex-1 min-w-0">
                    <span className={cn("text-[11px] font-mono truncate block", stateColor(node.state))}>
                        {roleName(node.role)}
                    </span>
                    {node.activity && (
                        <span className="text-[9px] font-mono text-aegis-cyan/40 italic truncate block">
                            {node.activity}
                        </span>
                    )}
                </div>

                <div className="flex items-center gap-2 shrink-0">
                    <span className="text-[8px] font-mono text-white/20">{roleLabel(node.role)}</span>
                    {node.model && (
                        <span className="text-[8px] font-mono text-aegis-purple/40">
                            {node.model.split('/').pop()}
                        </span>
                    )}
                </div>
            </div>

            {hasChildren && expanded && children.map(child => (
                <TreeRow
                    key={child.id}
                    node={child}
                    allNodes={allNodes}
                    depth={depth + 1}
                    onSelect={onSelect}
                />
            ))}
        </div>
    );
};

interface AgentTreeViewProps {
    projectId: string;
}

export const AgentTreeView: React.FC<AgentTreeViewProps> = ({ projectId }) => {
    const { agentTree, activeProjects } = useAegisStore();
    const [selectedNode, setSelectedNode] = useState<AgentNodeSummary | null>(null);

    const project = activeProjects.find(p => p.project_id === projectId);
    const root = agentTree.find(n =>
        n.role.type === 'ProjectSupervisor' &&
        (n.role as { type: 'ProjectSupervisor'; project_id: string }).project_id === projectId
    );

    if (!root) {
        return (
            <div className="flex flex-col items-center justify-center py-8 gap-2">
                <Circle className="w-6 h-6 text-white/10" />
                <p className="text-[10px] font-mono text-white/20">No active agents for this project</p>
            </div>
        );
    }

    return (
        <div className="flex flex-col gap-2">
            {project && (
                <div className="px-2 mb-2">
                    <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest">Project</p>
                    <p className="text-[12px] font-mono text-white/70 mt-0.5">{project.name}</p>
                </div>
            )}

            <div className="overflow-y-auto max-h-[400px] scrollbar-hide">
                <TreeRow
                    node={root}
                    allNodes={agentTree}
                    depth={0}
                    onSelect={setSelectedNode}
                />
            </div>

            {selectedNode && (
                <NodeDetailDrawer
                    node={selectedNode}
                    onClose={() => setSelectedNode(null)}
                />
            )}
        </div>
    );
};
```

---

## Integración en `TelemetryDashboard.tsx`

Leer el archivo completo antes de modificarlo. Agregar un nuevo panel/tab
"Projects" al Dashboard existente que muestre `ProjectList` y `AgentTreeView` side by side.

```tsx
// Imports a agregar
import { ProjectList } from './ProjectList';
import { AgentTreeView } from './AgentTreeView';

// Estado local a agregar en TelemetryDashboard
const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);

// Panel a agregar en el JSX del Dashboard
// (buscar la sección de panels/tabs existente y agregar uno nuevo "Agents")
<div className="grid grid-cols-2 gap-4">
    <div>
        <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mb-2">Projects</p>
        <ProjectList
            onSelectProject={setSelectedProjectId}
            selectedProjectId={selectedProjectId}
        />
    </div>
    <div>
        {selectedProjectId
            ? <AgentTreeView projectId={selectedProjectId} />
            : <p className="text-[10px] font-mono text-white/20 pt-8 text-center">Select a project</p>
        }
    </div>
</div>
```

El Shell Engineer debe leer TelemetryDashboard.tsx completo para determinar
exactamente dónde insertar el panel respetando el diseño existente.

---

## Criterios de Aceptación

- [ ] `ProjectList` muestra todos los proyectos activos del tenant
- [ ] Al seleccionar un proyecto, `AgentTreeView` muestra su árbol de agentes
- [ ] El árbol muestra estado en tiempo real (se actualiza sin recargar)
- [ ] Click en un nodo abre el drawer con estado, modelo, actividad y último reporte
- [ ] El drawer cierra al hacer click fuera o en la X
- [ ] El árbol es scrollable si tiene muchos nodos
- [ ] El layout no rompe el Dashboard existente
- [ ] `npm run build` sin errores TypeScript

---

## Commit message

```
feat(shell): CORE-203 AgentTreeView + ProjectList — panel en Dashboard con árbol en tiempo real
```

---

*Arquitecto IA — 2026-04-26*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: AgentTreeView.tsx + ProjectList.tsx existen y se renderizan en Dashboard.

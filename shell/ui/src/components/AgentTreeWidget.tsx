import React, { useEffect, useMemo, useState } from 'react';
import { Building2, Wrench, Zap, Bot } from 'lucide-react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

// ── Types ────────────────────────────────────────────────────────────────────

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

// ── Mock data (fallback cuando backend no está listo) ────────────────────────

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
    ],
};

// ── State config ─────────────────────────────────────────────────────────────

const STATE_CONFIG: Record<string, { color: string; pulse: boolean; label: string }> = {
    'Idle':          { color: 'text-white/30',   pulse: false, label: 'IDLE' },
    'Running':       { color: 'text-aegis-cyan', pulse: true,  label: 'RUNNING' },
    'WaitingReport': { color: 'text-yellow-400', pulse: true,  label: 'WAITING' },
    'Complete':      { color: 'text-green-400',  pulse: false, label: 'DONE' },
    'Failed':        { color: 'text-red-400',    pulse: false, label: 'FAILED' },
};

function getStateConfig(state: string) {
    const key = Object.keys(STATE_CONFIG).find(k => state.startsWith(k));
    return key ? STATE_CONFIG[key] : { color: 'text-white/30', pulse: false, label: state.toUpperCase() };
}

// ── Role icon ────────────────────────────────────────────────────────────────

function RoleIcon({ role }: { role: AgentRole }) {
    switch (role) {
        case 'ProjectSupervisor':
            return <Building2 className="w-4 h-4 text-aegis-cyan shrink-0" />;
        case 'DomainSupervisor':
            return <Wrench className="w-4 h-4 text-aegis-purple shrink-0" />;
        case 'Specialist':
        default:
            return <Zap className="w-4 h-4 text-yellow-400 shrink-0" />;
    }
}

// ── Agent node row ────────────────────────────────────────────────────────────

interface AgentRowProps {
    node: AgentNodeDto;
    nodeMap: Map<string, AgentNodeDto>;
    depth: number;
}

function AgentRow({ node, nodeMap, depth }: AgentRowProps) {
    const cfg = getStateConfig(node.state);

    return (
        <>
            <div
                className={cn(
                    'flex items-center gap-3 py-2 px-3 rounded-xl transition-colors hover:bg-white/[0.03]',
                    depth > 0 && 'border-l border-white/10 ml-6 pl-3',
                )}
                style={{ marginLeft: depth > 0 ? `${depth * 24}px` : undefined }}
            >
                {/* Vertical connector for non-root */}
                <RoleIcon role={node.role} />

                {/* Domain name */}
                <span className="text-xs font-medium text-white/80 flex-1 truncate">
                    {node.domain}
                </span>

                {/* Task type badge */}
                <span className="text-[9px] font-mono px-2 py-0.5 rounded bg-white/5 text-white/30 uppercase tracking-widest shrink-0">
                    {node.task_type.replace('Coding', 'CODE').replace('Planning', 'PLAN')}
                </span>

                {/* State indicator */}
                <div className="flex items-center gap-1.5 shrink-0">
                    <div
                        className={cn(
                            'w-1.5 h-1.5 rounded-full',
                            cfg.color.replace('text-', 'bg-'),
                            cfg.pulse && 'animate-pulse',
                        )}
                    />
                    <span className={cn('text-[9px] font-mono', cfg.color)}>{cfg.label}</span>
                </div>
            </div>

            {/* Render children recursively */}
            {node.children.map(childId => {
                const child = nodeMap.get(childId);
                return child ? (
                    <AgentRow key={childId} node={child} nodeMap={nodeMap} depth={depth + 1} />
                ) : null;
            })}
        </>
    );
}

// ── Main widget ───────────────────────────────────────────────────────────────

interface AgentTreeWidgetProps {
    tenantId: string | null;
    sessionKey: string | null;
}

const AgentTreeWidget: React.FC<AgentTreeWidgetProps> = ({ tenantId, sessionKey }) => {
    const [tree, setTree] = useState<AgentTreeDto>({ nodes: [], roots: [], total_agents: 0 });

    const fetchTree = async () => {
        if (!tenantId || !sessionKey) {
            setTree(MOCK_TREE);
            return;
        }
        try {
            const res = await fetch('/api/agents/tree', {
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
            });
            if (res.ok) {
                setTree(await res.json());
            } else {
                setTree(MOCK_TREE);
            }
        } catch {
            setTree(MOCK_TREE);
        }
    };

    useEffect(() => {
        void fetchTree();
        const interval = setInterval(() => void fetchTree(), 2000);
        return () => clearInterval(interval);
    // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [tenantId, sessionKey]);

    const nodeMap = useMemo(() => {
        const map = new Map<string, AgentNodeDto>();
        tree.nodes.forEach(n => map.set(n.agent_id, n));
        return map;
    }, [tree]);

    const rootNodes = useMemo(
        () => tree.roots.map(id => nodeMap.get(id)).filter((n): n is AgentNodeDto => !!n),
        [tree.roots, nodeMap],
    );

    return (
        <div className="glass p-6 rounded-2xl border border-white/10">
            {tree.total_agents === 0 ? (
                <div className="flex flex-col items-center justify-center py-12 gap-3 text-white/20">
                    <Bot className="w-10 h-10" />
                    <p className="text-xs font-mono uppercase tracking-widest">No hay agentes activos</p>
                    <p className="text-[10px] text-white/10 text-center max-w-xs">
                        Los agentes se activarán cuando inicies una tarea compleja
                    </p>
                </div>
            ) : (
                <div className="flex flex-col gap-1">
                    {rootNodes.map(root => (
                        <AgentRow key={root.agent_id} node={root} nodeMap={nodeMap} depth={0} />
                    ))}
                </div>
            )}
        </div>
    );
};

export default AgentTreeWidget;

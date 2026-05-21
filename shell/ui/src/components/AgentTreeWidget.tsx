import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Building2, Wrench, Zap, Bot, AlertCircle, Loader2 } from 'lucide-react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

// ── Types ────────────────────────────────────────────────────────────────────
// Matches the backend contract from GET /api/agents/tree (routes/agents.rs:
// AgentNodeDto / AgentTreeDto). The tree is reconstructed client-side from
// parent_id — the backend sends a flat node list, not a nested structure.

interface AgentNodeDto {
    agent_id: string;
    role: string; // display label, e.g. "ProjectSupervisor"
    project_id: string;
    parent_id: string | null;
    state: string;
    model: string;
    task_type: string;
    is_restored: boolean;
    last_report: string | null;
}

interface AgentTreeDto {
    nodes: AgentNodeDto[];
    total_agents: number;
}

type AgentTreeState =
    | { status: 'connecting' }
    | { status: 'connected'; nodes: AgentNodeDto[] }
    | { status: 'empty' }
    | { status: 'error'; message: string };

const POLL_INTERVAL_MS = 3000;
const MAX_RETRIES = 3;

// ── State config ─────────────────────────────────────────────────────────────

const STATE_CONFIG: Record<string, { color: string; pulse: boolean; label: string }> = {
    'Idle':          { color: 'text-white/30',   pulse: false, label: 'IDLE' },
    'Running':       { color: 'text-aegis-cyan', pulse: true,  label: 'RUNNING' },
    'WaitingReport': { color: 'text-yellow-400', pulse: true,  label: 'WAITING' },
    'WaitingUser':   { color: 'text-yellow-400', pulse: true,  label: 'ASKING' },
    'Complete':      { color: 'text-green-400',  pulse: false, label: 'DONE' },
    'Failed':        { color: 'text-red-400',    pulse: false, label: 'FAILED' },
};

function getStateConfig(state: string) {
    const key = Object.keys(STATE_CONFIG).find(k => state.startsWith(k));
    return key ? STATE_CONFIG[key] : { color: 'text-white/30', pulse: false, label: state.toUpperCase() };
}

// ── Role icon ────────────────────────────────────────────────────────────────
// Tolerant matching: role is a free-form display label from the backend
// (role.display_name()), so we match on substring rather than exact equality.

function RoleIcon({ role }: { role: string }) {
    const r = role.toLowerCase();
    if (r.includes('project')) {
        return <Building2 className="w-4 h-4 text-aegis-cyan shrink-0" />;
    }
    if (r.includes('domain')) {
        return <Wrench className="w-4 h-4 text-aegis-purple shrink-0" />;
    }
    return <Zap className="w-4 h-4 text-yellow-400 shrink-0" />;
}

// ── Agent node row ────────────────────────────────────────────────────────────

interface AgentRowProps {
    node: AgentNodeDto;
    childrenMap: Map<string, AgentNodeDto[]>;
    depth: number;
}

function AgentRow({ node, childrenMap, depth }: AgentRowProps) {
    const cfg = getStateConfig(node.state);
    const children = childrenMap.get(node.agent_id) ?? [];

    return (
        <>
            <div
                className={cn(
                    'flex items-center gap-3 py-2 px-3 rounded-xl transition-colors hover:bg-white/[0.03]',
                    depth > 0 && 'border-l border-white/10 ml-6 pl-3',
                )}
                style={{ marginLeft: depth > 0 ? `${depth * 24}px` : undefined }}
            >
                <RoleIcon role={node.role} />
                <span className="text-xs font-medium text-white/80 truncate">
                    {node.role}
                </span>
                <span className="text-[9px] font-mono text-white/25 truncate flex-1">
                    {node.model}
                </span>
                <span className="text-[9px] font-mono px-2 py-0.5 rounded bg-white/5 text-white/30 uppercase tracking-widest shrink-0">
                    {node.task_type.replace('Coding', 'CODE').replace('Planning', 'PLAN')}
                </span>
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
            {children.map(child => (
                <AgentRow key={child.agent_id} node={child} childrenMap={childrenMap} depth={depth + 1} />
            ))}
        </>
    );
}

// ── Main widget ───────────────────────────────────────────────────────────────

interface AgentTreeWidgetProps {
    tenantId: string | null;
    sessionKey: string | null;
}

const AgentTreeWidget: React.FC<AgentTreeWidgetProps> = ({ tenantId, sessionKey }) => {
    const [treeState, setTreeState] = useState<AgentTreeState>({ status: 'connecting' });
    const failCountRef = useRef(0);
    const mountedRef = useRef(true);

    const fetchTree = useCallback(async () => {
        if (!tenantId || !sessionKey) {
            setTreeState({ status: 'empty' });
            return;
        }
        try {
            const res = await fetch('/api/agents/tree', {
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
            });
            if (!res.ok) throw new Error(`HTTP ${res.status}`);
            const tree = (await res.json()) as AgentTreeDto;
            if (!mountedRef.current) return;
            failCountRef.current = 0;
            setTreeState(
                tree.total_agents > 0
                    ? { status: 'connected', nodes: tree.nodes }
                    : { status: 'empty' },
            );
        } catch (err) {
            if (!mountedRef.current) return;
            failCountRef.current += 1;
            // Tolerate transient blips; only surface an error after repeated misses.
            if (failCountRef.current >= MAX_RETRIES) {
                setTreeState({
                    status: 'error',
                    message: err instanceof Error ? err.message : 'No se pudo cargar el árbol de agentes',
                });
            }
        }
    }, [tenantId, sessionKey]);

    const handleRetry = () => {
        failCountRef.current = 0;
        setTreeState({ status: 'connecting' });
        void fetchTree();
    };

    useEffect(() => {
        mountedRef.current = true;
        void fetchTree();
        const timer = setInterval(() => void fetchTree(), POLL_INTERVAL_MS);
        return () => {
            mountedRef.current = false;
            clearInterval(timer);
        };
    }, [fetchTree]);

    const { childrenMap, rootNodes } = useMemo(() => {
        if (treeState.status !== 'connected') {
            return { childrenMap: new Map<string, AgentNodeDto[]>(), rootNodes: [] as AgentNodeDto[] };
        }
        const ids = new Set(treeState.nodes.map(n => n.agent_id));
        const cmap = new Map<string, AgentNodeDto[]>();
        const roots: AgentNodeDto[] = [];
        for (const n of treeState.nodes) {
            // A node is a root when it has no parent, or its parent is not part
            // of this snapshot (orphan → surface it at the top instead of hiding it).
            if (!n.parent_id || !ids.has(n.parent_id)) {
                roots.push(n);
            } else {
                const siblings = cmap.get(n.parent_id) ?? [];
                siblings.push(n);
                cmap.set(n.parent_id, siblings);
            }
        }
        return { childrenMap: cmap, rootNodes: roots };
    }, [treeState]);

    if (treeState.status === 'connecting') {
        return (
            <div className="glass p-6 rounded-2xl border border-white/10 flex items-center justify-center gap-3 min-h-[120px]">
                <Loader2 className="w-4 h-4 animate-spin text-aegis-cyan/40" />
                <span className="text-[10px] font-mono text-white/20 uppercase tracking-widest">Conectando...</span>
            </div>
        );
    }

    if (treeState.status === 'error') {
        return (
            <div className="glass p-6 rounded-2xl border border-red-500/20 flex flex-col items-center justify-center gap-3 min-h-[120px]">
                <AlertCircle className="w-5 h-5 text-red-500/50" />
                <p className="text-[10px] font-mono text-white/30 uppercase tracking-widest">AgentTreeWidget unavailable</p>
                <p className="text-[9px] font-mono text-white/20 text-center">{treeState.message}</p>
                <button
                    onClick={handleRetry}
                    className="text-[9px] font-mono text-white/20 hover:text-white/50 uppercase tracking-widest underline"
                >
                    retry
                </button>
            </div>
        );
    }

    if (treeState.status === 'empty') {
        return (
            <div className="glass p-6 rounded-2xl border border-white/10">
                <div className="flex flex-col items-center justify-center py-12 gap-3 text-white/20">
                    <Bot className="w-10 h-10" />
                    <p className="text-xs font-mono uppercase tracking-widest">Sin agentes activos</p>
                    <p className="text-[10px] text-white/10 text-center max-w-xs">
                        Iniciá un proyecto para ver el árbol de agentes
                    </p>
                </div>
            </div>
        );
    }

    return (
        <div className="glass p-6 rounded-2xl border border-white/10">
            <div className="flex flex-col gap-1">
                {rootNodes.map(root => (
                    <AgentRow key={root.agent_id} node={root} childrenMap={childrenMap} depth={0} />
                ))}
            </div>
        </div>
    );
};

export default AgentTreeWidget;

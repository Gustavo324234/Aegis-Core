import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Building2, Wrench, Zap, Bot, AlertCircle, Loader2 } from 'lucide-react';
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

type AgentTreeState =
    | { status: 'connecting' }
    | { status: 'connected'; agents: AgentTreeDto }
    | { status: 'empty' }
    | { status: 'error'; message: string };

const MAX_RETRIES = 3;

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
                <RoleIcon role={node.role} />
                <span className="text-xs font-medium text-white/80 flex-1 truncate">
                    {node.domain}
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
    const [treeState, setTreeState] = useState<AgentTreeState>({ status: 'connecting' });
    const wsRef = useRef<WebSocket | null>(null);
    const retryCountRef = useRef(0);
    const retryTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    const connect = () => {
        if (!tenantId || !sessionKey) {
            setTreeState({ status: 'empty' });
            return;
        }

        setTreeState({ status: 'connecting' });
        const protocol = window.location.protocol === 'https:' ? 'wss' : 'ws';
        const ws = new WebSocket(`${protocol}://${window.location.host}/ws/agents/${tenantId}`);
        wsRef.current = ws;

        ws.onopen = () => {
            retryCountRef.current = 0;
        };

        ws.onmessage = (e) => {
            try {
                const tree = JSON.parse(e.data as string) as AgentTreeDto;
                setTreeState(
                    tree.total_agents > 0
                        ? { status: 'connected', agents: tree }
                        : { status: 'empty' }
                );
            } catch {
                // ignore malformed frames
            }
        };

        ws.onerror = () => {
            ws.close();
        };

        ws.onclose = () => {
            wsRef.current = null;
            if (retryCountRef.current < MAX_RETRIES) {
                const delay = 1000 * Math.pow(2, retryCountRef.current);
                retryCountRef.current++;
                retryTimerRef.current = setTimeout(connect, delay);
            } else {
                setTreeState({ status: 'error', message: 'No se pudo conectar al stream de agentes' });
            }
        };
    };

    const handleRetry = () => {
        retryCountRef.current = 0;
        if (retryTimerRef.current) {
            clearTimeout(retryTimerRef.current);
            retryTimerRef.current = null;
        }
        if (wsRef.current) {
            wsRef.current.onclose = null;
            wsRef.current.close();
            wsRef.current = null;
        }
        connect();
    };

    useEffect(() => {
        connect();
        return () => {
            if (retryTimerRef.current) clearTimeout(retryTimerRef.current);
            if (wsRef.current) {
                wsRef.current.onclose = null;
                wsRef.current.close();
            }
        };
    // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [tenantId, sessionKey]);

    const { nodeMap, rootNodes } = useMemo(() => {
        if (treeState.status !== 'connected') return { nodeMap: new Map(), rootNodes: [] };
        const map = new Map<string, AgentNodeDto>();
        treeState.agents.nodes.forEach(n => map.set(n.agent_id, n));
        const roots = treeState.agents.roots
            .map(id => map.get(id))
            .filter((n): n is AgentNodeDto => !!n);
        return { nodeMap: map, rootNodes: roots };
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
                    <AgentRow key={root.agent_id} node={root} nodeMap={nodeMap} depth={0} />
                ))}
            </div>
        </div>
    );
};

export default AgentTreeWidget;

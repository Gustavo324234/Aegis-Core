import React, { useState, useEffect, Component, type ReactNode } from 'react';
import { motion } from 'framer-motion';
import {
    LayoutDashboard,
    Trello,
    Zap,
    ArrowLeft,
    Plus,
    MoreVertical,
    CheckCircle2,
    Clock,
    AlertCircle,
    TrendingUp,
    Bot,
    Code2,
} from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';
import type { ProjectSummary, AgentNodeSummary, AgentNodeState, RoutingInfo } from '../store/useAegisStore';
import AgentTreeWidget from './AgentTreeWidget';
import { ProjectList } from './ProjectList';
import { AgentTreeView } from './AgentTreeView';
import TerminalPanel from './workspace/TerminalPanel';
import CodeViewer from './workspace/CodeViewer';
import GitTimeline from './workspace/GitTimeline';
import PRManagerPanel from './workspace/PRManagerPanel';
import WorkspaceSettings from './workspace/WorkspaceSettings';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

class WidgetErrorBoundary extends Component<
    { name: string; children: ReactNode },
    { hasError: boolean }
> {
    constructor(props: { name: string; children: ReactNode }) {
        super(props);
        this.state = { hasError: false };
    }
    static getDerivedStateFromError() {
        return { hasError: true };
    }
    componentDidCatch(error: Error) {
        console.error(`[Widget: ${this.props.name}]`, error);
    }
    render() {
        if (this.state.hasError) {
            return (
                <div className="glass p-6 rounded-2xl border border-red-500/20 flex flex-col items-center justify-center gap-3 min-h-[120px]">
                    <AlertCircle className="w-5 h-5 text-red-500/50" />
                    <p className="text-[10px] font-mono text-white/30 uppercase tracking-widest">{this.props.name} unavailable</p>
                    <button
                        onClick={() => this.setState({ hasError: false })}
                        className="text-[9px] font-mono text-white/20 hover:text-white/50 uppercase tracking-widest underline"
                    >
                        retry
                    </button>
                </div>
            );
        }
        return this.props.children;
    }
}

interface ProjectCard {
    id: string;
    title: string;
    description: string | null;
    status: 'active' | 'archived';
    agentState: AgentNodeState | null;
    model: string | null;
}

function mapProjectsToCards(
    projects: ProjectSummary[],
    agentTree: AgentNodeSummary[]
): { backlog: ProjectCard[]; active: ProjectCard[]; verified: ProjectCard[] } {
    const backlog: ProjectCard[] = [];
    const active: ProjectCard[] = [];
    const verified: ProjectCard[] = [];

    for (const project of projects) {
        const rootAgent = project.root_agent_id
            ? agentTree.find(n => n.id === project.root_agent_id) ?? null
            : null;

        const card: ProjectCard = {
            id: project.project_id,
            title: project.name,
            description: project.description,
            status: project.status,
            agentState: rootAgent?.state ?? null,
            model: rootAgent?.model ?? null,
        };

        if (project.status === 'archived') {
            verified.push(card);
        } else if (rootAgent?.state === 'Running') {
            active.push(card);
        } else {
            backlog.push(card);
        }
    }

    return { backlog, active, verified };
}

const AgentStateBadge: React.FC<{ state: AgentNodeState | null }> = ({ state }) => {
    if (!state) return null;

    if (state === 'Running') {
        return (
            <span className="flex items-center gap-1.5">
                <span className="w-1.5 h-1.5 rounded-full bg-aegis-cyan animate-pulse shadow-[0_0_6px_rgba(0,255,255,0.6)]" />
                <span className="text-[9px] font-mono text-aegis-cyan uppercase">Running</span>
            </span>
        );
    }
    if (state === 'Complete') {
        return (
            <span className="flex items-center gap-1.5">
                <span className="w-1.5 h-1.5 rounded-full bg-green-500" />
                <span className="text-[9px] font-mono text-green-500 uppercase">Complete</span>
            </span>
        );
    }
    if (state === 'Idle' || state === 'WaitingReport') {
        return (
            <span className="flex items-center gap-1.5">
                <span className="w-1.5 h-1.5 rounded-full bg-white/30" />
                <span className="text-[9px] font-mono text-white/30 uppercase">{state === 'WaitingReport' ? 'Waiting' : 'Idle'}</span>
            </span>
        );
    }
    return (
        <span className="flex items-center gap-1.5">
            <span className="w-1.5 h-1.5 rounded-full bg-red-500" />
            <span className="text-[9px] font-mono text-red-400 uppercase">Failed</span>
        </span>
    );
};

const KanbanColumn: React.FC<{
    title: string;
    projects: ProjectCard[];
    icon: React.ReactNode;
    color: string;
}> = ({ title, projects, icon, color }) => {
    return (
        <div className="flex flex-col gap-4 w-full min-w-[300px]">
            <div className="flex items-center justify-between px-2">
                <div className="flex items-center gap-2">
                    <div className={cn("p-1.5 rounded-lg bg-opacity-20", color)}>
                        {icon}
                    </div>
                    <h3 className="text-xs font-mono font-bold uppercase tracking-widest text-white/70">{title}</h3>
                    <span className="text-[10px] font-mono bg-white/5 px-2 py-0.5 rounded-full text-white/40">{projects.length}</span>
                </div>
                <button className="text-white/20 hover:text-white transition-colors">
                    <Plus className="w-4 h-4" />
                </button>
            </div>

            <div className="flex flex-col gap-3 min-h-[500px] rounded-2xl bg-white/[0.02] border border-white/[0.05] p-3">
                {projects.length === 0 ? (
                    <div className="flex flex-col items-center justify-center py-12 gap-3 text-center">
                        <p className="text-[10px] font-mono text-white/20 uppercase tracking-widest">Sin proyectos</p>
                        <p className="text-[11px] font-mono text-white/30 max-w-[200px] leading-relaxed">
                            Iniciá un proyecto diciéndole a tu agente qué querés construir.
                        </p>
                    </div>
                ) : (
                    projects.map((project, idx) => (
                        <motion.div
                            key={project.id}
                            initial={{ opacity: 0, y: 10 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ delay: idx * 0.05 }}
                            className="glass p-4 rounded-xl border border-white/10 hover:border-aegis-cyan/30 transition-all cursor-pointer group"
                        >
                            <div className="flex justify-between items-start mb-2">
                                <AgentStateBadge state={project.agentState} />
                                <button className="text-white/10 group-hover:text-white/40">
                                    <MoreVertical className="w-3.5 h-3.5" />
                                </button>
                            </div>
                            <h4 className="text-xs font-medium text-white/90 leading-relaxed mb-2">{project.title}</h4>
                            {project.description && (
                                <p className="text-[10px] text-white/40 leading-relaxed line-clamp-2 mb-2">{project.description}</p>
                            )}
                            {project.model && (
                                <p className="text-[9px] font-mono text-white/20">{project.model}</p>
                            )}
                        </motion.div>
                    ))
                )}
            </div>
        </div>
    );
};

// CORE-250 — ApiCostWidget: muestra telemetría real de inferencia del store
const ApiCostWidget: React.FC<{ lastRoutingInfo: RoutingInfo | null }> = ({ lastRoutingInfo }) => {
    return (
        <div className="glass p-6 rounded-2xl border border-white/10 h-full flex flex-col gap-6">
            <div className="flex items-center gap-3">
                <div className="p-2 rounded-xl bg-aegis-cyan/10 text-aegis-cyan">
                    <Zap className="w-5 h-5" />
                </div>
                <div>
                    <h3 className="text-sm font-bold text-white">Uso de API</h3>
                    <p className="text-[10px] text-white/40 font-mono uppercase">Sesión actual</p>
                </div>
            </div>

            {!lastRoutingInfo ? (
                <div className="flex-1 flex flex-col items-center justify-center gap-3 text-center py-4">
                    <p className="text-xs font-mono text-white/30 leading-relaxed">
                        Sin datos de consumo disponibles para esta sesión.
                    </p>
                    <p className="text-[10px] font-mono text-white/20 max-w-[220px] leading-relaxed">
                        Activá el Plugin Ledger para seguimiento de gastos.
                    </p>
                </div>
            ) : (
                <div className="space-y-3">
                    <div className="flex items-center justify-between p-3 rounded-xl bg-white/[0.03] border border-white/[0.05]">
                        <span className="text-[10px] font-mono text-white/40 uppercase tracking-wider">Proveedor</span>
                        <span className="text-xs font-bold text-aegis-cyan">{lastRoutingInfo.provider}</span>
                    </div>
                    <div className="flex items-center justify-between p-3 rounded-xl bg-white/[0.03] border border-white/[0.05]">
                        <span className="text-[10px] font-mono text-white/40 uppercase tracking-wider">Modelo</span>
                        <span className="text-xs font-bold text-white truncate max-w-[180px] text-right font-mono">{lastRoutingInfo.model_id}</span>
                    </div>
                    <div className="flex items-center justify-between p-3 rounded-xl bg-white/[0.03] border border-white/[0.05]">
                        <span className="text-[10px] font-mono text-white/40 uppercase tracking-wider">Latencia</span>
                        <span className="text-xs font-bold text-white font-mono">{lastRoutingInfo.latency_ms} ms</span>
                    </div>
                    <div className="flex items-center justify-between p-3 rounded-xl bg-white/[0.03] border border-white/[0.05]">
                        <span className="text-[10px] font-mono text-white/40 uppercase tracking-wider">Tipo de tarea</span>
                        <span className="text-xs font-bold text-white/70 font-mono">{lastRoutingInfo.task_type}</span>
                    </div>
                </div>
            )}
        </div>
    );
};

interface ChronosEvent {
    title: string;
    time: string;
    urgency?: string;
}

function formatRelativeTime(iso: string): string {
    const diff = new Date(iso).getTime() - Date.now();
    if (diff <= 0) return 'pasado';
    const mins = Math.floor(diff / 60000);
    if (mins < 60) return `en ${mins} min`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `en ${hours}h`;
    return `en ${Math.floor(hours / 24)}d`;
}

// CORE-251 — ChronosWidget: intenta fetch a /api/chronos/events, muestra estado vacío honesto si no existe
const ChronosWidget: React.FC<{ tenantId: string | null; sessionKey: string | null }> = ({ tenantId, sessionKey }) => {
    const [events, setEvents] = useState<ChronosEvent[]>([]);
    const [loaded, setLoaded] = useState(false);

    useEffect(() => {
        if (!tenantId || !sessionKey) { setLoaded(true); return; }
        fetch('/api/chronos/events?limit=3&from=now', {
            headers: { 'x-citadel-tenant': tenantId, 'x-citadel-key': sessionKey },
        })
            .then(r => {
                if (!r.ok) { setLoaded(true); return null; }
                return r.json() as Promise<{ events?: ChronosEvent[] }>;
            })
            .then(data => {
                setEvents(data?.events ?? []);
                setLoaded(true);
            })
            .catch(() => setLoaded(true));
    }, [tenantId, sessionKey]);

    return (
        <div className="glass p-6 rounded-2xl border border-white/10 h-full flex flex-col gap-6">
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                    <div className="p-2 rounded-xl bg-aegis-purple/10 text-aegis-purple">
                        <Clock className="w-5 h-5" />
                    </div>
                    <div>
                        <h3 className="text-sm font-bold text-white">Neural Schedule</h3>
                        <p className="text-[10px] text-white/40 font-mono uppercase">Chronos Plugin Active</p>
                    </div>
                </div>
            </div>

            {!loaded ? (
                <div className="flex-1 flex items-center justify-center">
                    <p className="text-[10px] font-mono text-white/20 uppercase tracking-widest animate-pulse">Cargando...</p>
                </div>
            ) : events.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-8 gap-3 text-center">
                    <Clock className="w-8 h-8 text-white/10" />
                    <p className="text-xs font-mono text-white/30">Sin eventos próximos.</p>
                    <p className="text-[10px] font-mono text-white/20 max-w-[220px] leading-relaxed">
                        Pedile a tu agente que agende algo: "Anotá una reunión para mañana a las 10".
                    </p>
                </div>
            ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                    {events.map((event, i) => (
                        <div key={i} className="p-4 rounded-xl bg-white/[0.03] border border-white/[0.05] space-y-2">
                            <div className="flex justify-between items-start">
                                <span className="text-[10px] font-mono text-aegis-cyan uppercase">
                                    {formatRelativeTime(event.time)}
                                </span>
                                {event.urgency === 'high' && <AlertCircle className="w-3 h-3 text-aegis-cyan" />}
                            </div>
                            <h4 className="text-xs font-medium">{event.title}</h4>
                        </div>
                    ))}
                </div>
            )}
        </div>
    );
};

const Dashboard: React.FC = () => {
    const { setCurrentView, system_metrics, tenantId, sessionKey, isAgentStreamConnected, systemState, activeProjects, agentTree, lastRoutingInfo } = useAegisStore();
    const { backlog, active, verified } = mapProjectsToCards(activeProjects, agentTree);

    const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
    const [tenantName, setTenantName] = useState<string | null>(null);

    React.useEffect(() => {
        if (!tenantId || !sessionKey) return;
        fetch('/api/persona', {
            headers: { 'x-citadel-tenant': tenantId, 'x-citadel-key': sessionKey }
        })
            .then(r => r.ok ? r.json() : null)
            .then((data: { name?: string } | null) => setTenantName(data?.name ?? tenantId))
            .catch(() => setTenantName(tenantId));
    }, [tenantId, sessionKey]);

    const systemStatusText = ({
        'STATE_OPERATIONAL': 'Kernel Operational // All systems nominal',
        'STATE_INITIALIZING': 'Kernel Initializing // Please wait',
        'UNKNOWN': 'System Status Unknown',
    } as Record<string, string>)[systemState] ?? 'Checking system status...';

    const systemStatusColor = systemState === 'STATE_OPERATIONAL' ? 'text-white/40'
        : systemState === 'STATE_INITIALIZING' ? 'text-yellow-500/60'
        : 'text-red-500/60';

    const isOperational = systemState === 'STATE_OPERATIONAL';

    return (
        <div className="h-full w-full flex flex-col bg-black text-white overflow-hidden">
            {/* Header */}
            <header className="shrink-0 border-b border-white/5 flex items-center justify-between px-8 bg-black/40 backdrop-blur-3xl z-50" style={{ height: '56px' }}>
                <div className="flex items-center gap-6">
                    <button
                        onClick={() => setCurrentView('chat')}
                        className="group flex items-center gap-2 text-white/40 hover:text-aegis-cyan transition-colors"
                    >
                        <ArrowLeft className="w-4 h-4 group-hover:-translate-x-1 transition-transform" />
                        <span className="text-[10px] font-mono uppercase tracking-widest">Back to Chat</span>
                    </button>
                    <div className="h-4 w-px bg-white/10" />
                    <div className="flex items-center gap-2">
                        <LayoutDashboard className="w-4 h-4 text-aegis-cyan" />
                        <h1 className="text-[10px] font-mono tracking-[0.4em] text-white font-bold uppercase">System Dashboard</h1>
                    </div>
                </div>

                <div className="flex items-center gap-6">
                    <div className="flex flex-col items-end">
                        <div className="flex items-center justify-between gap-2 w-24">
                            <span className="text-[8px] font-mono text-white/20 uppercase tracking-widest">CPU</span>
                            <span className="text-[9px] font-mono text-white/40">{system_metrics.cpu_load.toFixed(0)}%</span>
                        </div>
                        <div className="w-24 h-1 bg-white/5 rounded-full overflow-hidden mt-1">
                            <motion.div
                                initial={{ width: 0 }}
                                animate={{ width: `${system_metrics.cpu_load}%` }}
                                className="h-full bg-aegis-cyan"
                            />
                        </div>
                    </div>
                    <div className="h-8 w-px bg-white/10" />
                    <div className="flex items-center gap-3">
                        <div className={cn(
                            "w-2 h-2 rounded-full",
                            isOperational ? "bg-green-500 animate-pulse"
                            : systemState === 'STATE_INITIALIZING' ? "bg-yellow-500"
                            : "bg-red-500"
                        )} />
                        <span className="text-[10px] font-mono text-white/40 uppercase">
                            {isOperational ? 'Kernel Operational' : systemState === 'STATE_INITIALIZING' ? 'Kernel Initializing' : 'System Unknown'}
                        </span>
                    </div>
                </div>
            </header>

            {/* Main Content */}
            <main className="flex-1 overflow-y-auto p-8 space-y-8 scrollbar-hide">
                <div className="grid grid-cols-12 gap-8">
                    {/* Welcome / Stats */}
                    <div className="col-span-12 flex flex-col gap-2">
                        <h2 className="text-3xl font-bold tracking-tight">Welcome back, <span className="text-aegis-cyan">{tenantName ?? tenantId ?? 'Operator'}</span></h2>
                        <p className={cn("font-mono text-xs uppercase tracking-widest", systemStatusColor)}>{systemStatusText}</p>
                    </div>

                    {/* API Cost Widget — CORE-250 */}
                    <div className="col-span-12 lg:col-span-4 h-full">
                        <ApiCostWidget lastRoutingInfo={lastRoutingInfo} />
                    </div>

                    {/* Chronos / Neural Schedule — CORE-251 */}
                    <div className="col-span-12 lg:col-span-8">
                        <ChronosWidget tenantId={tenantId} sessionKey={sessionKey} />
                    </div>

                    {/* Kanban Board */}
                    <div className="col-span-12 space-y-6">
                        <div className="flex items-center gap-3">
                            <Trello className="w-5 h-5 text-aegis-cyan" />
                            <h2 className="text-xl font-bold uppercase tracking-widest">Cognitive Backlog</h2>
                        </div>

                        <div className="flex gap-8 overflow-x-auto pb-4 scrollbar-hide">
                            <KanbanColumn
                                title="Backlog"
                                projects={backlog}
                                icon={<Clock className="w-4 h-4" />}
                                color="bg-white/20 text-white"
                            />
                            <KanbanColumn
                                title="Active Tasks"
                                projects={active}
                                icon={<TrendingUp className="w-4 h-4" />}
                                color="bg-aegis-cyan/20 text-aegis-cyan"
                            />
                            <KanbanColumn
                                title="Verified"
                                projects={verified}
                                icon={<CheckCircle2 className="w-4 h-4" />}
                                color="bg-green-500/20 text-green-500"
                            />
                        </div>
                    </div>

                    {/* Agent Tree — Epic 43 */}
                    <div className="col-span-12 space-y-6">
                        <div className="flex items-center gap-3">
                            <Bot className="w-5 h-5 text-aegis-cyan" />
                            <h2 className="text-xl font-bold uppercase tracking-widest">Active Agents</h2>
                            <span className="text-[10px] font-mono text-white/20 uppercase tracking-widest ml-2">— Hierarchical Orchestration</span>
                        </div>
                        <WidgetErrorBoundary name="AgentTreeWidget">
                            <AgentTreeWidget tenantId={tenantId} sessionKey={sessionKey} />
                        </WidgetErrorBoundary>
                    </div>

                    {/* Projects & Agents — CORE-203 */}
                    <div className="col-span-12 space-y-6">
                        <div className="flex items-center gap-3">
                            <Bot className="w-5 h-5 text-aegis-purple" />
                            <h2 className="text-xl font-bold uppercase tracking-widest">Projects</h2>
                            <span className="text-[10px] font-mono text-white/20 uppercase tracking-widest ml-2">
                                — Cognitive Agent Architecture
                            </span>
                            {isAgentStreamConnected
                                ? <span className="text-[9px] font-mono text-green-400/50 uppercase tracking-widest">● live</span>
                                : <span className="text-[9px] font-mono text-white/20 uppercase tracking-widest">○ connecting</span>
                            }
                        </div>
                        <div className="glass rounded-2xl border border-white/10 p-6">
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
                        </div>
                    </div>

                    {/* Developer Workspace — Epic 44 */}
                    <div className="col-span-12 space-y-6">
                        <div className="flex items-center gap-3">
                            <Code2 className="w-5 h-5 text-aegis-cyan" />
                            <h2 className="text-xl font-bold uppercase tracking-widest">Developer Workspace</h2>
                            <span className="text-[10px] font-mono text-white/20 uppercase tracking-widest ml-2">— Epic 44</span>
                        </div>

                        {/* Row 1: Terminal + Git Timeline */}
                        <div className="grid grid-cols-12 gap-6">
                            <div className="col-span-12 lg:col-span-6">
                                <WidgetErrorBoundary name="TerminalPanel">
                                    <TerminalPanel />
                                </WidgetErrorBoundary>
                            </div>
                            <div className="col-span-12 lg:col-span-6">
                                <WidgetErrorBoundary name="GitTimeline">
                                    <GitTimeline />
                                </WidgetErrorBoundary>
                            </div>
                        </div>

                        {/* Row 2: Code Viewer (full width) */}
                        <WidgetErrorBoundary name="CodeViewer">
                            <CodeViewer />
                        </WidgetErrorBoundary>

                        {/* Row 3: PR Manager + Workspace Settings */}
                        <div className="grid grid-cols-12 gap-6">
                            <div className="col-span-12 lg:col-span-7">
                                <WidgetErrorBoundary name="PRManagerPanel">
                                    <PRManagerPanel />
                                </WidgetErrorBoundary>
                            </div>
                            <div className="col-span-12 lg:col-span-5">
                                <WidgetErrorBoundary name="WorkspaceSettings">
                                    <WorkspaceSettings />
                                </WidgetErrorBoundary>
                            </div>
                        </div>
                    </div>
                </div>
            </main>
        </div>
    );
};

export default Dashboard;

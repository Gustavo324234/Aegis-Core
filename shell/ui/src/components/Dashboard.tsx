import React from 'react';
import { motion } from 'framer-motion';
import {
    LayoutDashboard,
    Trello,
    Wallet,
    ArrowLeft,
    Plus,
    MoreVertical,
    CheckCircle2,
    Clock,
    AlertCircle,
    TrendingUp,
    Calendar,
    ChevronRight,
    Bot,
} from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';
import AgentTreeWidget from './AgentTreeWidget';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface Ticket {
    id: string;
    title: string;
    type: 'feat' | 'fix' | 'task';
    priority: 'Crítica' | 'Alta' | 'Media' | 'Baja';
    status: 'Todo' | 'In Progress' | 'Done';
}

const MOCK_TICKETS: Ticket[] = [
    { id: 'CORE-150', title: 'Sandbox de Scripts (Maker Capability)', type: 'feat', priority: 'Crítica', status: 'Todo' },
    { id: 'CORE-151', title: 'Integración de Contexto de Proyecto (Git/VCM)', type: 'feat', priority: 'Alta', status: 'In Progress' },
    { id: 'CORE-153', title: 'Dashboard Dinámico & Kanban UI', type: 'feat', priority: 'Alta', status: 'In Progress' },
    { id: 'CORE-148', title: 'Natural Conversational Tone (Prompt)', type: 'fix', priority: 'Alta', status: 'In Progress' },
    { id: 'CORE-145', title: 'Conversational Onboarding (Name/Persona)', type: 'feat', priority: 'Crítica', status: 'Done' },
    { id: 'CORE-149', title: 'Neuronal Memory (L3) & Semantic Retrieval', type: 'feat', priority: 'Crítica', status: 'Done' },
];

const KanbanColumn: React.FC<{ 
    title: string; 
    status: Ticket['status']; 
    tickets: Ticket[];
    icon: React.ReactNode;
    color: string;
}> = ({ title, status, tickets, icon, color }) => {
    const columnTickets = tickets.filter(t => t.status === status);

    return (
        <div className="flex flex-col gap-4 w-full min-w-[300px]">
            <div className="flex items-center justify-between px-2">
                <div className="flex items-center gap-2">
                    <div className={cn("p-1.5 rounded-lg bg-opacity-20", color)}>
                        {icon}
                    </div>
                    <h3 className="text-xs font-mono font-bold uppercase tracking-widest text-white/70">{title}</h3>
                    <span className="text-[10px] font-mono bg-white/5 px-2 py-0.5 rounded-full text-white/40">{columnTickets.length}</span>
                </div>
                <button className="text-white/20 hover:text-white transition-colors">
                    <Plus className="w-4 h-4" />
                </button>
            </div>

            <div className="flex flex-col gap-3 min-h-[500px] rounded-2xl bg-white/[0.02] border border-white/[0.05] p-3">
                {columnTickets.map((ticket, idx) => (
                    <motion.div
                        key={ticket.id}
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ delay: idx * 0.05 }}
                        className="glass p-4 rounded-xl border border-white/10 hover:border-aegis-cyan/30 transition-all cursor-pointer group"
                    >
                        <div className="flex justify-between items-start mb-2">
                            <span className={cn(
                                "text-[9px] font-mono px-2 py-0.5 rounded uppercase tracking-tighter",
                                ticket.type === 'feat' ? "bg-aegis-cyan/10 text-aegis-cyan" : "bg-aegis-purple/10 text-aegis-purple"
                            )}>
                                {ticket.id}
                            </span>
                            <button className="text-white/10 group-hover:text-white/40">
                                <MoreVertical className="w-3.5 h-3.5" />
                            </button>
                        </div>
                        <h4 className="text-xs font-medium text-white/90 leading-relaxed mb-3">{ticket.title}</h4>
                        <div className="flex items-center justify-between">
                            <div className="flex items-center gap-2">
                                <div className={cn(
                                    "w-1.5 h-1.5 rounded-full",
                                    ticket.priority === 'Crítica' ? "bg-red-500 shadow-[0_0_8px_rgba(239,68,68,0.5)]" :
                                    ticket.priority === 'Alta' ? "bg-orange-500" :
                                    ticket.priority === 'Media' ? "bg-yellow-500" : "bg-green-500"
                                )} />
                                <span className="text-[9px] font-mono text-white/30 uppercase">{ticket.priority}</span>
                            </div>
                            <div className="flex -space-x-2">
                                <div className="w-5 h-5 rounded-full bg-aegis-cyan/20 border border-black flex items-center justify-center text-[8px] font-bold text-aegis-cyan">A</div>
                            </div>
                        </div>
                    </motion.div>
                ))}
            </div>
        </div>
    );
};

const FinancialWidget = () => {
    return (
        <div className="glass p-6 rounded-2xl border border-white/10 h-full flex flex-col gap-6">
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                    <div className="p-2 rounded-xl bg-green-500/10 text-green-500">
                        <Wallet className="w-5 h-5" />
                    </div>
                    <div>
                        <h3 className="text-sm font-bold text-white">Monthly Expenses</h3>
                        <p className="text-[10px] text-white/40 font-mono uppercase">Ledger Plugin Active</p>
                    </div>
                </div>
                <div className="text-right">
                    <span className="text-2xl font-bold text-white">$1,240.50</span>
                    <div className="flex items-center justify-end gap-1 text-[10px] text-green-500">
                        <TrendingUp className="w-3 h-3" />
                        <span>-12% vs last month</span>
                    </div>
                </div>
            </div>

            <div className="space-y-4">
                <div className="flex items-center justify-between p-3 rounded-xl bg-white/[0.03] border border-white/[0.05]">
                    <div className="flex items-center gap-3">
                        <div className="w-8 h-8 rounded-lg bg-blue-500/20 flex items-center justify-center text-blue-400">
                            <Calendar className="w-4 h-4" />
                        </div>
                        <div>
                            <p className="text-xs font-medium text-white/80">AWS Cloud Services</p>
                            <p className="text-[9px] text-white/30 font-mono">Infrastructure</p>
                        </div>
                    </div>
                    <span className="text-xs font-bold text-white">-$85.20</span>
                </div>
                <div className="flex items-center justify-between p-3 rounded-xl bg-white/[0.03] border border-white/[0.05]">
                    <div className="flex items-center gap-3">
                        <div className="w-8 h-8 rounded-lg bg-aegis-purple/20 flex items-center justify-center text-aegis-purple">
                            <CheckCircle2 className="w-4 h-4" />
                        </div>
                        <div>
                            <p className="text-xs font-medium text-white/80">OpenAI API Tier 1</p>
                            <p className="text-[9px] text-white/30 font-mono">Core Intelligence</p>
                        </div>
                    </div>
                    <span className="text-xs font-bold text-white">-$120.00</span>
                </div>
            </div>

            <button className="mt-auto w-full py-2.5 rounded-xl bg-white/5 hover:bg-white/10 text-[10px] font-bold uppercase tracking-[0.2em] transition-all flex items-center justify-center gap-2">
                Open Full Ledger <ChevronRight className="w-3 h-3" />
            </button>
        </div>
    );
};

const Dashboard: React.FC = () => {
    const { setCurrentView, system_metrics, tenantId, sessionKey } = useAegisStore();

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
                        <span className="text-[8px] font-mono text-white/20 uppercase tracking-widest">CPU LOAD</span>
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
                        <div className="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
                        <span className="text-[10px] font-mono text-white/40 uppercase">Kernel Operational</span>
                    </div>
                </div>
            </header>

            {/* Main Content */}
            <main className="flex-1 overflow-y-auto p-8 space-y-8 scrollbar-hide">
                <div className="grid grid-cols-12 gap-8">
                    {/* Welcome / Stats */}
                    <div className="col-span-12 flex flex-col gap-2">
                        <h2 className="text-3xl font-bold tracking-tight">Welcome back, <span className="text-aegis-cyan">Operator</span></h2>
                        <p className="text-white/40 font-mono text-xs uppercase tracking-widest">System status: Optimal // All enclaves secured</p>
                    </div>

                    {/* Financial Widget */}
                    <div className="col-span-12 lg:col-span-4 h-full">
                        <FinancialWidget />
                    </div>

                    {/* Chronos / Upcoming */}
                    <div className="col-span-12 lg:col-span-8">
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
                            
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                <div className="p-4 rounded-xl bg-white/[0.03] border border-white/[0.05] space-y-2">
                                    <div className="flex justify-between items-start">
                                        <span className="text-[10px] font-mono text-aegis-cyan">IN 15 MIN</span>
                                        <AlertCircle className="w-3 h-3 text-aegis-cyan" />
                                    </div>
                                    <h4 className="text-xs font-medium">Sync Project Context</h4>
                                    <p className="text-[9px] text-white/30">Review Git status and pending tickets.</p>
                                </div>
                                <div className="p-4 rounded-xl bg-white/[0.03] border border-white/[0.05] space-y-2 opacity-50">
                                    <div className="flex justify-between items-start">
                                        <span className="text-[10px] font-mono text-white/40">IN 2 HOURS</span>
                                    </div>
                                    <h4 className="text-xs font-medium">Backup Ring 0 Identity</h4>
                                    <p className="text-[9px] text-white/30">Standard maintenance protocol.</p>
                                </div>
                            </div>
                        </div>
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
                                status="Todo" 
                                tickets={MOCK_TICKETS} 
                                icon={<Clock className="w-4 h-4" />}
                                color="bg-white/20 text-white"
                            />
                            <KanbanColumn 
                                title="Active Tasks" 
                                status="In Progress" 
                                tickets={MOCK_TICKETS} 
                                icon={<TrendingUp className="w-4 h-4" />}
                                color="bg-aegis-cyan/20 text-aegis-cyan"
                            />
                            <KanbanColumn 
                                title="Verified" 
                                status="Done" 
                                tickets={MOCK_TICKETS} 
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
                        <AgentTreeWidget tenantId={tenantId} sessionKey={sessionKey} />
                    </div>
                </div>
            </main>
        </div>
    );
};

export default Dashboard;

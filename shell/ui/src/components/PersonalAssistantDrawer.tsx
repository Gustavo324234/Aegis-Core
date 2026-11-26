import React, { useCallback, useEffect, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { useShallow } from 'zustand/react/shallow';
import { 
    X, 
    Wallet, 
    Brain, 
    Activity, 
    RefreshCw, 
    Calendar, 
    Tag, 
    Clock, 
    DollarSign,
    ListChecks,
    AlertCircle,
    Loader2,
    Check,
    Trash2,
    Pencil
} from 'lucide-react';
import { useAegisStore, AgentNodeSummary, AgentRole } from '../store/useAegisStore';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface ExpenseEntry {
    id?: number | string;
    amount: number;
    category: string;
    description: string;
    created_at?: string;
}

interface PersonalFact {
    id: number | string;
    category: string;
    fact: string;
    source: string;
    created_at?: string;
}

interface Reminder {
    id: number | string;
    description: string;
    status: 'pending' | 'completed' | string;
    remind_at?: string;
    due_date?: string;
    recurrence?: string;
    created_at?: string;
}

function formatRecurrenceLabel(rec?: string): string | null {
    if (!rec) return null;
    const map: Record<string, string> = {
        daily: 'Todos los días',
        weekdays: 'Lunes a viernes',
        weekly: 'Semanal',
        monthly: 'Mensual',
        'weekly:mon': 'Todos los lunes',
        'weekly:tue': 'Todos los martes',
        'weekly:wed': 'Todos los miércoles',
        'weekly:thu': 'Todos los jueves',
        'weekly:fri': 'Todos los viernes',
        'weekly:sat': 'Todos los sábados',
        'weekly:sun': 'Todos los domingos',
    };
    return map[rec] ?? rec;
}

interface PersonalSummaryData {
    ledger?: {
        total?: number;
        totals_by_category?: Record<string, number>;
        recent_entries?: ExpenseEntry[];
    };
    facts?: PersonalFact[];
    reminders?: Reminder[];
}

interface PersonalAssistantDrawerProps {
    isOpen: boolean;
    onClose: () => void;
}

function roleName(role: AgentRole): string {
    switch (role.type) {
        case 'ChatAgent': return 'Asistente Principal';
        case 'ProjectSupervisor': return `Proyecto ${role.project_id}`;
        case 'Supervisor': return role.name;
        case 'Specialist': return role.scope.length > 30 ? role.scope.slice(0, 30) + '…' : role.scope;
    }
}

export const PersonalAssistantDrawer: React.FC<PersonalAssistantDrawerProps> = ({ isOpen, onClose }) => {
    const { tenantId, sessionKey, agentTree } = useAegisStore(
        useShallow((s) => ({
            tenantId: s.tenantId,
            sessionKey: s.sessionKey,
            agentTree: s.agentTree,
        })),
    );
    const [activeTab, setActiveTab] = useState<'expenses' | 'memory' | 'background'>('expenses');
    const [loading, setLoading] = useState(false);
    const [summary, setSummary] = useState<PersonalSummaryData | null>(null);
    const [isConsolidating, setIsConsolidating] = useState(false);
    const [message, setMessage] = useState<string | null>(null);
    const [agyInstruction, setAgyInstruction] = useState('');
    const [agySending, setAgySending] = useState(false);
    const [agyFeedback, setAgyFeedback] = useState<string | null>(null);
    const [editingFactId, setEditingFactId] = useState<number | string | null>(null);
    const [editingFactText, setEditingFactText] = useState('');
    const [editingFactCategory, setEditingFactCategory] = useState('general');
    const [savingFact, setSavingFact] = useState(false);

    const handleSendToAntigravity = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!agyInstruction.trim() || agySending) return;
        setAgySending(true);
        setAgyFeedback(null);
        try {
            const res = await fetch('/api/antigravity/ide/outbox', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId || 'default',
                    'x-citadel-key': sessionKey || '',
                },
                body: JSON.stringify({
                    instruction: agyInstruction.trim(),
                    dispatch_sdk: true,
                }),
            });
            if (res.ok) {
                setAgyInstruction('');
                setAgyFeedback('Enviado a Google Antigravity (SDK + IDE). Te avisará por voz y chat al responder.');
            } else {
                setAgyFeedback('No se pudo enviar la instrucción a Antigravity.');
            }
        } catch {
            setAgyFeedback('Error de red al contactar Antigravity Bridge.');
        } finally {
            setAgySending(false);
        }
    };

    const fetchSummary = useCallback(async () => {
        if (!tenantId || !sessionKey) return;
        setLoading(true);
        try {
            const res = await fetch('/api/personal/summary', {
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
            });
            if (res.ok) {
                const data = await res.json() as PersonalSummaryData;
                setSummary(data);
            } else if (res.status === 404) {
                // Graceful fallback si el backend de personal summary aún no está montado
                setSummary({
                    ledger: { total: 0, totals_by_category: {}, recent_entries: [] },
                    facts: [],
                    reminders: [],
                });
            }
        } catch (e) {
            console.error('Error fetching personal summary:', e);
            setSummary({
                ledger: { total: 0, totals_by_category: {}, recent_entries: [] },
                facts: [],
                reminders: [],
            });
        } finally {
            setLoading(false);
        }
    }, [tenantId, sessionKey]);

    const handleConsolidateChronos = async () => {
        if (!tenantId || !sessionKey) return;
        setIsConsolidating(true);
        setMessage(null);
        try {
            const res = await fetch('/api/personal/chronos/consolidate', {
                method: 'POST',
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
            });
            if (res.ok) {
                setMessage('Consolidación de Chronos completada.');
                await fetchSummary();
            } else {
                setMessage('Chronos: No se pudo consolidar en este momento.');
            }
        } catch {
            setMessage('Error de comunicación con Chronos.');
        } finally {
            setIsConsolidating(false);
            setTimeout(() => setMessage(null), 4000);
        }
    };

    const handleSaveFactEdit = async (factId: number | string) => {
        if (!tenantId || !sessionKey || !editingFactText.trim() || savingFact) return;
        setSavingFact(true);
        try {
            const res = await fetch(`/api/personal/facts/${factId}`, {
                method: 'PUT',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({
                    category: editingFactCategory,
                    fact: editingFactText.trim(),
                }),
            });
            if (res.ok) {
                setEditingFactId(null);
                setMessage('Nota actualizada.');
                await fetchSummary();
            } else {
                setMessage('No se pudo actualizar la nota.');
            }
        } catch {
            setMessage('Error de red al actualizar la nota.');
        } finally {
            setSavingFact(false);
            setTimeout(() => setMessage(null), 3000);
        }
    };

    const handleDeleteFact = async (factId: number | string) => {
        if (!tenantId || !sessionKey) return;
        try {
            const res = await fetch(`/api/personal/facts/${factId}`, {
                method: 'DELETE',
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
            });
            if (res.ok) {
                if (editingFactId === factId) setEditingFactId(null);
                await fetchSummary();
            }
        } catch {
            setMessage('Error de red al eliminar la nota.');
            setTimeout(() => setMessage(null), 3000);
        }
    };

    useEffect(() => {
        if (isOpen) {
            void fetchSummary();
        }
    }, [isOpen, fetchSummary]);

    const totalAmount = summary?.ledger?.total ?? 0;
    const categoryTotals = summary?.ledger?.totals_by_category ?? {};
    const recentExpenses = summary?.ledger?.recent_entries ?? [];
    const facts = summary?.facts ?? [];
    const reminders = summary?.reminders ?? [];

    return (
        <AnimatePresence>
            {isOpen && (
                <>
                    {/* Backdrop */}
                    <motion.div
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        onClick={onClose}
                        className="fixed inset-0 bg-black/60 backdrop-blur-sm z-50"
                    />

                    {/* Drawer container */}
                    <motion.div
                        initial={{ x: '100%' }}
                        animate={{ x: 0 }}
                        exit={{ x: '100%' }}
                        transition={{ type: 'spring', damping: 25, stiffness: 220 }}
                        className="fixed right-0 top-0 bottom-0 w-full sm:w-[480px] bg-black/90 border-l border-white/10 backdrop-blur-2xl z-50 flex flex-col shadow-2xl text-white"
                    >
                        {/* Drawer Header */}
                        <div className="flex items-center justify-between px-6 py-4 border-b border-white/10 shrink-0 bg-white/5">
                            <div className="flex items-center gap-3">
                                <div className="p-2 rounded-xl bg-aegis-cyan/10 border border-aegis-cyan/20">
                                    <Brain className="w-5 h-5 text-aegis-cyan" />
                                </div>
                                <div>
                                    <h2 className="text-sm font-bold uppercase tracking-wider text-white">Mi Asistente Personal</h2>
                                    <p className="text-[10px] font-mono text-white/50">Memoria, Finanzas y Tareas</p>
                                </div>
                            </div>
                            <div className="flex items-center gap-2">
                                <button
                                    onClick={fetchSummary}
                                    disabled={loading}
                                    className="p-1.5 rounded-lg text-white/40 hover:text-white hover:bg-white/5 transition-colors disabled:opacity-30"
                                    title="Actualizar datos"
                                >
                                    <RefreshCw className={cn("w-4 h-4", loading && "animate-spin text-aegis-cyan")} />
                                </button>
                                <button
                                    onClick={onClose}
                                    className="p-1.5 rounded-lg text-white/40 hover:text-white hover:bg-white/5 transition-colors"
                                    title="Cerrar panel"
                                >
                                    <X className="w-5 h-5" />
                                </button>
                            </div>
                        </div>

                        {/* Tabs Switcher */}
                        <div className="flex border-b border-white/10 px-4 shrink-0 bg-black/40">
                            <button
                                onClick={() => setActiveTab('expenses')}
                                className={cn(
                                    "flex-1 py-3 text-xs font-mono tracking-wider uppercase border-b-2 flex items-center justify-center gap-2 transition-all",
                                    activeTab === 'expenses'
                                        ? "border-emerald-400 text-emerald-400 font-bold bg-emerald-500/5"
                                        : "border-transparent text-white/40 hover:text-white/80"
                                )}
                            >
                                <Wallet className="w-3.5 h-3.5" />
                                <span>Gastos</span>
                            </button>
                            <button
                                onClick={() => setActiveTab('memory')}
                                className={cn(
                                    "flex-1 py-3 text-xs font-mono tracking-wider uppercase border-b-2 flex items-center justify-center gap-2 transition-all",
                                    activeTab === 'memory'
                                        ? "border-aegis-cyan text-aegis-cyan font-bold bg-aegis-cyan/5"
                                        : "border-transparent text-white/40 hover:text-white/80"
                                )}
                            >
                                <Brain className="w-3.5 h-3.5" />
                                <span>Memoria</span>
                            </button>
                            <button
                                onClick={() => setActiveTab('background')}
                                className={cn(
                                    "flex-1 py-3 text-xs font-mono tracking-wider uppercase border-b-2 flex items-center justify-center gap-2 transition-all",
                                    activeTab === 'background'
                                        ? "border-aegis-purple text-aegis-purple font-bold bg-aegis-purple/5"
                                        : "border-transparent text-white/40 hover:text-white/80"
                                )}
                            >
                                <Activity className="w-3.5 h-3.5" />
                                <span>Segundo Plano</span>
                            </button>
                        </div>

                        {/* Notifications */}
                        {message && (
                            <div className="px-6 py-2 bg-aegis-cyan/10 border-b border-aegis-cyan/20 text-xs font-mono text-aegis-cyan flex items-center gap-2">
                                <AlertCircle className="w-4 h-4 shrink-0" />
                                <span>{message}</span>
                            </div>
                        )}

                        {/* Content Body */}
                        <div className="flex-1 overflow-y-auto p-6 space-y-6">
                            {/* TAB 1: GASTOS */}
                            {activeTab === 'expenses' && (
                                <div className="space-y-6">
                                    {/* Total Card */}
                                    <div className="glass p-5 rounded-2xl border border-white/10 bg-gradient-to-br from-emerald-500/10 via-black to-black space-y-2">
                                        <div className="flex items-center justify-between text-white/50 text-xs font-mono">
                                            <span>Total Registrado</span>
                                            <DollarSign className="w-4 h-4 text-emerald-400" />
                                        </div>
                                        <div className="text-3xl font-extrabold text-white font-mono tracking-tight">
                                            ${totalAmount.toLocaleString('es-AR', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
                                        </div>
                                        <p className="text-[10px] text-white/40 font-mono">
                                            Registrado por chat en lenguaje natural
                                        </p>
                                    </div>

                                    {/* Breakdown by Category */}
                                    <div className="space-y-3">
                                        <h3 className="text-xs font-mono uppercase tracking-wider text-white/60 flex items-center gap-2">
                                            <Tag className="w-3.5 h-3.5 text-emerald-400" />
                                            <span>Por Categoría</span>
                                        </h3>
                                        {Object.keys(categoryTotals).length === 0 ? (
                                            <div className="p-4 rounded-xl bg-white/5 border border-white/5 text-center text-xs text-white/40 font-mono">
                                                Sin gastos categorizados todavía.
                                            </div>
                                        ) : (
                                            <div className="space-y-2.5">
                                                {Object.entries(categoryTotals).map(([cat, amount]) => {
                                                    const pct = totalAmount > 0 ? Math.round((amount / totalAmount) * 100) : 0;
                                                    return (
                                                        <div key={cat} className="space-y-1">
                                                            <div className="flex justify-between text-xs font-mono">
                                                                <span className="capitalize text-white/80">{cat}</span>
                                                                <span className="text-white/60">${amount.toLocaleString()} ({pct}%)</span>
                                                            </div>
                                                            <div className="h-1.5 w-full bg-white/5 rounded-full overflow-hidden">
                                                                <div 
                                                                    className="h-full bg-emerald-400 rounded-full transition-all duration-500"
                                                                    style={{ width: `${pct}%` }}
                                                                />
                                                            </div>
                                                        </div>
                                                    );
                                                })}
                                            </div>
                                        )}
                                    </div>

                                    {/* Recent Entries */}
                                    <div className="space-y-3">
                                        <h3 className="text-xs font-mono uppercase tracking-wider text-white/60 flex items-center gap-2">
                                            <Clock className="w-3.5 h-3.5 text-emerald-400" />
                                            <span>Últimos Movimientos</span>
                                        </h3>
                                        {recentExpenses.length === 0 ? (
                                            <div className="p-4 rounded-xl bg-white/5 border border-white/5 text-center text-xs text-white/40 font-mono">
                                                No hay movimientos registrados. Decile a Aegis: "Gasté $5.000 en el súper".
                                            </div>
                                        ) : (
                                            <div className="space-y-2">
                                                {recentExpenses.map((exp, idx) => (
                                                    <div 
                                                        key={exp.id || idx}
                                                        className="p-3 rounded-xl bg-white/5 border border-white/5 flex items-center justify-between gap-3 text-xs"
                                                    >
                                                        <div className="min-w-0">
                                                            <div className="font-medium text-white/90 truncate">{exp.description}</div>
                                                            <div className="text-[10px] font-mono text-white/40 uppercase mt-0.5">
                                                                {exp.category} {exp.created_at && `• ${new Date(exp.created_at).toLocaleDateString()}`}
                                                            </div>
                                                        </div>
                                                        <div className="font-mono font-bold text-emerald-400 text-sm shrink-0">
                                                            -${exp.amount.toLocaleString()}
                                                        </div>
                                                    </div>
                                                ))}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            )}

                            {/* TAB 2: MEMORIA Y RECORDATORIOS */}
                            {activeTab === 'memory' && (
                                <div className="space-y-6">
                                    {/* Chronos Action Button */}
                                    <div className="flex items-center justify-between p-4 rounded-2xl bg-white/5 border border-white/10">
                                        <div>
                                            <h4 className="text-xs font-bold text-white uppercase tracking-wider">Consolidación Chronos</h4>
                                            <p className="text-[10px] font-mono text-white/40">Sintetiza hechos de tus conversaciones recientes</p>
                                        </div>
                                        <button
                                            onClick={handleConsolidateChronos}
                                            disabled={isConsolidating}
                                            className="px-3 py-1.5 rounded-xl bg-aegis-cyan/10 hover:bg-aegis-cyan/20 border border-aegis-cyan/30 text-aegis-cyan text-xs font-mono uppercase tracking-wider flex items-center gap-2 transition-all disabled:opacity-50"
                                        >
                                            {isConsolidating ? (
                                                <>
                                                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                                                    <span>Consolidando...</span>
                                                </>
                                            ) : (
                                                <>
                                                    <RefreshCw className="w-3.5 h-3.5" />
                                                    <span>Consolidar Ahora</span>
                                                </>
                                            )}
                                        </button>
                                    </div>

                                    {/* Learned Facts */}
                                    <div className="space-y-3">
                                        <h3 className="text-xs font-mono uppercase tracking-wider text-white/60 flex items-center gap-2">
                                            <Brain className="w-3.5 h-3.5 text-aegis-cyan" />
                                            <span>Hechos Aprendidos ({facts.length})</span>
                                        </h3>
                                        {facts.length === 0 ? (
                                            <div className="p-4 rounded-xl bg-white/5 border border-white/5 text-center text-xs text-white/40 font-mono">
                                                Aegis aún no ha registrado hechos sobre vos. Podés contarle cosas como "me gusta el café negro" o "mi cumpleaños es en mayo".
                                            </div>
                                        ) : (
                                            <div className="space-y-2">
                                                {facts.map((item, idx) => {
                                                    const isEditing = editingFactId === item.id;
                                                    return (
                                                        <div 
                                                            key={item.id || idx}
                                                            className="p-3.5 rounded-xl bg-white/5 border border-white/5 space-y-2"
                                                        >
                                                            {isEditing ? (
                                                                <div className="space-y-2">
                                                                    <textarea
                                                                        value={editingFactText}
                                                                        onChange={(e) => setEditingFactText(e.target.value)}
                                                                        rows={2}
                                                                        className="w-full bg-black/50 border border-aegis-cyan/40 rounded-lg px-2.5 py-1.5 text-xs text-white focus:outline-none focus:border-aegis-cyan"
                                                                    />
                                                                    <div className="flex items-center justify-between gap-2">
                                                                        <select
                                                                            value={editingFactCategory}
                                                                            onChange={(e) => setEditingFactCategory(e.target.value)}
                                                                            className="bg-black/50 border border-white/15 rounded-lg px-2 py-1 text-[10px] font-mono text-white/80 focus:outline-none"
                                                                        >
                                                                            <option value="preferencias">preferencias</option>
                                                                            <option value="rutina">rutina</option>
                                                                            <option value="salud">salud</option>
                                                                            <option value="trabajo">trabajo</option>
                                                                            <option value="familia">familia</option>
                                                                            <option value="general">general</option>
                                                                        </select>
                                                                        <div className="flex items-center gap-1.5">
                                                                            <button
                                                                                type="button"
                                                                                onClick={() => setEditingFactId(null)}
                                                                                className="px-2 py-1 rounded-lg bg-white/5 hover:bg-white/10 text-[10px] font-mono text-white/60"
                                                                            >
                                                                                Cancelar
                                                                            </button>
                                                                            <button
                                                                                type="button"
                                                                                disabled={savingFact || !editingFactText.trim()}
                                                                                onClick={() => handleSaveFactEdit(item.id)}
                                                                                className="px-2.5 py-1 rounded-lg bg-aegis-cyan/20 hover:bg-aegis-cyan/30 border border-aegis-cyan/40 text-[10px] font-mono text-aegis-cyan disabled:opacity-50"
                                                                            >
                                                                                {savingFact ? 'Guardando...' : 'Guardar'}
                                                                            </button>
                                                                        </div>
                                                                    </div>
                                                                </div>
                                                            ) : (
                                                                <>
                                                                    <div className="flex items-start justify-between gap-2">
                                                                        <div className="text-xs text-white/90 leading-relaxed flex-1">
                                                                            <span className="text-[10px] font-mono text-white/35 mr-1.5">#{item.id}</span>
                                                                            {item.fact}
                                                                        </div>
                                                                        <div className="flex items-center gap-1 shrink-0">
                                                                            <button
                                                                                type="button"
                                                                                onClick={() => {
                                                                                    setEditingFactId(item.id);
                                                                                    setEditingFactText(item.fact);
                                                                                    setEditingFactCategory(item.category || 'general');
                                                                                }}
                                                                                className="p-1 rounded-lg text-white/40 hover:text-aegis-cyan hover:bg-aegis-cyan/10 transition-colors"
                                                                                title="Editar nota"
                                                                            >
                                                                                <Pencil className="w-3.5 h-3.5" />
                                                                            </button>
                                                                            <button
                                                                                type="button"
                                                                                onClick={() => handleDeleteFact(item.id)}
                                                                                className="p-1 rounded-lg text-white/40 hover:text-red-400 hover:bg-red-500/10 transition-colors"
                                                                                title="Eliminar nota"
                                                                            >
                                                                                <Trash2 className="w-3.5 h-3.5" />
                                                                            </button>
                                                                        </div>
                                                                    </div>
                                                                    <div className="flex items-center gap-2 text-[9px] font-mono text-white/40 uppercase">
                                                                        <span className="px-1.5 py-0.5 rounded bg-white/5 border border-white/10">
                                                                            {item.category}
                                                                        </span>
                                                                        <span className="text-aegis-cyan/80">
                                                                            {item.source === 'chronos' ? 'Consolidado por Chronos' : item.source === 'user_ui' ? 'Editado por vos' : 'Aprendido en Chat'}
                                                                        </span>
                                                                    </div>
                                                                </>
                                                            )}
                                                        </div>
                                                    );
                                                })}
                                            </div>
                                        )}
                                    </div>

                                    {/* Reminders List */}
                                    <div className="space-y-3">
                                        <h3 className="text-xs font-mono uppercase tracking-wider text-white/60 flex items-center gap-2">
                                            <Calendar className="w-3.5 h-3.5 text-aegis-cyan" />
                                            <span>Recordatorios ({reminders.filter(r => r.status !== 'completed').length} pendientes)</span>
                                        </h3>
                                        {reminders.length === 0 ? (
                                            <div className="p-4 rounded-xl bg-white/5 border border-white/5 text-center text-xs text-white/40 font-mono">
                                                No tenés recordatorios pendientes.
                                            </div>
                                        ) : (
                                            <div className="space-y-2">
                                                {reminders.map((rem, idx) => {
                                                    const when = rem.remind_at || rem.due_date;
                                                    const recLabel = formatRecurrenceLabel(rem.recurrence);
                                                    const isDone = rem.status === 'completed';
                                                    return (
                                                        <div
                                                            key={rem.id ?? idx}
                                                            className={cn(
                                                                "p-3 rounded-xl bg-white/5 border border-white/5 flex items-start gap-3 text-xs",
                                                                isDone && "opacity-55"
                                                            )}
                                                        >
                                                            <ListChecks className={cn("w-4 h-4 mt-0.5 shrink-0", isDone ? "text-emerald-400" : "text-aegis-cyan")} />
                                                            <div className="min-w-0 flex-1">
                                                                <div className={cn("text-white/90", isDone && "line-through")}>
                                                                    <span className="text-[10px] font-mono text-white/35 mr-1.5">#{rem.id}</span>
                                                                    {rem.description}
                                                                </div>
                                                                <div className="flex flex-wrap items-center gap-2 mt-1 text-[10px] font-mono text-white/45">
                                                                    {when && <span>{when}</span>}
                                                                    {recLabel && (
                                                                        <span className="px-1.5 py-0.5 rounded bg-aegis-cyan/10 border border-aegis-cyan/25 text-aegis-cyan">
                                                                            {recLabel}
                                                                        </span>
                                                                    )}
                                                                </div>
                                                            </div>
                                                            <div className="flex items-center gap-1 shrink-0">
                                                                {!isDone && (
                                                                    <button
                                                                        onClick={async () => {
                                                                            if (!tenantId || !sessionKey) return;
                                                                            await fetch(`/api/personal/reminders/${rem.id}/complete`, {
                                                                                method: 'POST',
                                                                                headers: { 'x-citadel-tenant': tenantId, 'x-citadel-key': sessionKey },
                                                                            });
                                                                            await fetchSummary();
                                                                        }}
                                                                        className="p-1 rounded-lg text-white/40 hover:text-emerald-400 hover:bg-emerald-500/10 transition-colors"
                                                                        title="Marcar como completado"
                                                                    >
                                                                        <Check className="w-3.5 h-3.5" />
                                                                    </button>
                                                                )}
                                                                <button
                                                                    onClick={async () => {
                                                                        if (!tenantId || !sessionKey) return;
                                                                        await fetch(`/api/personal/reminders/${rem.id}`, {
                                                                            method: 'DELETE',
                                                                            headers: { 'x-citadel-tenant': tenantId, 'x-citadel-key': sessionKey },
                                                                        });
                                                                        await fetchSummary();
                                                                    }}
                                                                    className="p-1 rounded-lg text-white/40 hover:text-red-400 hover:bg-red-500/10 transition-colors"
                                                                    title="Eliminar recordatorio"
                                                                >
                                                                    <Trash2 className="w-3.5 h-3.5" />
                                                                </button>
                                                            </div>
                                                        </div>
                                                    );
                                                })}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            )}

                            {/* TAB 3: SEGUNDO PLANO */}
                            {activeTab === 'background' && (
                                <div className="space-y-6">
                                    {/* Puente directo con Google Antigravity */}
                                    <div className="p-4 rounded-2xl bg-gradient-to-br from-aegis-purple/15 to-aegis-cyan/10 border border-aegis-cyan/25 space-y-3">
                                        <div className="flex items-center justify-between">
                                            <span className="text-xs font-mono font-bold uppercase tracking-wider text-aegis-cyan flex items-center gap-1.5">
                                                ⚡ Puente Google Antigravity
                                            </span>
                                            <span className="px-2 py-0.5 rounded-full text-[9px] font-mono bg-aegis-cyan/15 text-aegis-cyan border border-aegis-cyan/30">
                                                SDK + IDE Hook
                                            </span>
                                        </div>
                                        <p className="text-[11px] text-white/65 leading-relaxed">
                                            Enviá una orden directa a Antigravity o hablá con tu asistente diciendo{' '}
                                            <span className="text-white font-mono">"decile a Antigravity que..."</span>. Cuando termine, te leerá la respuesta en voz alta.
                                        </p>
                                        <form onSubmit={handleSendToAntigravity} className="flex gap-2">
                                            <input
                                                type="text"
                                                value={agyInstruction}
                                                onChange={(e) => setAgyInstruction(e.target.value)}
                                                placeholder="Instrucción para Antigravity..."
                                                className="flex-1 bg-black/50 border border-white/15 rounded-xl px-3 py-2 text-xs text-white placeholder:text-white/30 focus:outline-none focus:border-aegis-cyan/60"
                                            />
                                            <button
                                                type="submit"
                                                disabled={agySending || !agyInstruction.trim()}
                                                className="px-3.5 py-2 rounded-xl bg-aegis-cyan/20 border border-aegis-cyan/40 text-xs font-mono font-bold text-aegis-cyan hover:bg-aegis-cyan/30 disabled:opacity-40 transition-all"
                                            >
                                                {agySending ? '...' : 'Enviar'}
                                            </button>
                                        </form>
                                        {agyFeedback && (
                                            <p className="text-[10px] font-mono text-emerald-300">
                                                {agyFeedback}
                                            </p>
                                        )}
                                    </div>

                                    <div className="space-y-3">
                                        <h3 className="text-xs font-mono uppercase tracking-wider text-white/60 flex items-center gap-2">
                                            <Activity className="w-3.5 h-3.5 text-aegis-purple" />
                                            <span>Agentes y Tareas Activas</span>
                                        </h3>
                                        {agentTree.length === 0 ? (
                                            <div className="p-6 rounded-2xl bg-white/5 border border-white/5 text-center space-y-2">
                                                <Activity className="w-8 h-8 text-white/20 mx-auto" />
                                                <div className="text-xs font-mono text-white/50">
                                                    No hay subagentes ni tareas concurrentes en ejecución.
                                                </div>
                                                <p className="text-[10px] text-white/30">
                                                    Cuando le pidas a Aegis investigar temas profundos o ejecutar proyectos complejos, sus workers se mostrarán aquí.
                                                </p>
                                            </div>
                                        ) : (
                                            <div className="space-y-3">
                                                {agentTree.map((node: AgentNodeSummary) => (
                                                    <div 
                                                        key={node.id}
                                                        className="p-4 rounded-xl bg-white/5 border border-white/10 space-y-2"
                                                    >
                                                        <div className="flex items-center justify-between text-xs">
                                                            <span className="font-bold text-white/90">
                                                                {roleName(node.role)}
                                                            </span>
                                                            <span className="px-2 py-0.5 rounded text-[10px] font-mono uppercase bg-aegis-purple/10 text-aegis-purple border border-aegis-purple/30">
                                                                {typeof node.state === 'string' ? node.state : 'Failed'}
                                                            </span>
                                                        </div>
                                                        {node.activity && (
                                                            <p className="text-xs text-white/60 italic font-mono">
                                                                "{node.activity}"
                                                            </p>
                                                        )}
                                                        {node.last_report && (
                                                            <div className="p-2.5 rounded-lg bg-black/50 border border-white/5 text-[11px] text-white/70">
                                                                {node.last_report}
                                                            </div>
                                                        )}
                                                    </div>
                                                ))}
                                            </div>
                                        )}
                                    </div>
                                </div>
                            )}
                        </div>
                    </motion.div>
                </>
            )}
        </AnimatePresence>
    );
};

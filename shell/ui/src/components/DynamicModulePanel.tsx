import React, { useState, useEffect, useCallback } from 'react';
import { motion } from 'framer-motion';
import {
    Puzzle,
    Play,
    Loader2,
    Cpu,
    Database,
    AlertTriangle,
    Plus,
    TrendingUp,
    CheckCircle2,
    Settings,
} from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';

// Dynamic mapping of Lucide icons based on manifest strings
const iconMap: Record<string, React.ComponentType<{ className?: string }>> = {
    Plus: Plus,
    TrendingUp: TrendingUp,
    Settings: Settings,
    Cpu: Cpu,
    Database: Database,
};

interface FieldSchema {
    name: string;
    label: string;
    type: 'text' | 'number';
    placeholder?: string;
    required?: boolean;
}

interface UiViewSchema {
    view_id: string;
    type: 'form';
    title: string;
    description?: string;
    icon?: string;
    tool_name: string;
    fields: FieldSchema[];
}

interface ExposedToolSchema {
    name: string;
    description: string;
    parameters: Record<string, unknown>;
}

interface ModuleData {
    module_id: string;
    display_name: string;
    version: string;
    active: boolean;
    exposed_tools: ExposedToolSchema[];
    ui_views: UiViewSchema[];
}

interface ExecuteResult {
    success: boolean;
    result?: unknown;
    error?: string;
}

export const DynamicModulePanel: React.FC = () => {
    const { tenantId, sessionKey } = useAegisStore();
    const [modules, setModules] = useState<ModuleData[]>([]);
    const [selectedModuleId, setSelectedModuleId] = useState<string | null>(null);
    const [activeViewId, setActiveViewId] = useState<string | null>(null);
    const [formValues, setFormValues] = useState<Record<string, string | number>>({});
    const [isLoading, setIsLoading] = useState<boolean>(false);
    const [executingTool, setExecutingTool] = useState<string | null>(null);
    const [execResult, setExecResult] = useState<ExecuteResult | null>(null);
    const [errorMsg, setErrorMsg] = useState<string | null>(null);
    const [fetchError, setFetchError] = useState<string | null>(null);
    const [refreshing, setRefreshing] = useState<boolean>(false);

    // Fetch discovered modules on mount
    const fetchModules = useCallback(async (silent = false) => {
        if (!tenantId || !sessionKey) return;
        if (!silent) setRefreshing(true);
        setFetchError(null);

        try {
            const res = await fetch('/api/router/modules', {
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
            });

            if (!res.ok) {
                throw new Error(`HTTP Error ${res.status}`);
            }

            const data = await res.json() as { modules: ModuleData[] };
            setModules(data.modules || []);

            // Auto-select first module if none selected
            if (data.modules && data.modules.length > 0 && !selectedModuleId) {
                const firstModule = data.modules[0];
                setSelectedModuleId(firstModule.module_id as string);
                if (firstModule.ui_views && firstModule.ui_views.length > 0) {
                    setActiveViewId(firstModule.ui_views[0].view_id);
                }
            }
        } catch (err: unknown) {
            console.error('Failed to fetch modules:', err);
            setFetchError((err as Error).message || 'Error de conexión');
        } finally {
            setRefreshing(false);
        }
    }, [tenantId, sessionKey, selectedModuleId]);

    useEffect(() => {
        fetchModules();
    }, [tenantId, sessionKey, fetchModules]);

    // Handle module activation toggle
    const toggleModule = async (moduleId: string, currentStatus: boolean) => {
        if (!tenantId || !sessionKey) return;
        setIsLoading(true);
        setErrorMsg(null);

        try {
            const res = await fetch(`/api/router/modules/${moduleId}/enable`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({ enabled: !currentStatus }),
            });

            if (!res.ok) {
                throw new Error(`Error toggling module: ${res.status}`);
            }

            const data = await res.json();
            if (data.success) {
                // Update local status
                setModules(prev =>
                    prev.map(m =>
                        m.module_id === moduleId ? { ...m, active: data.active } : m
                    )
                );
            }
        } catch (err: unknown) {
            console.error(err);
            setErrorMsg((err as Error).message || 'Error al cambiar estado del módulo');
        } finally {
            setIsLoading(false);
        }
    };

    const selectedModule = modules.find(m => m.module_id === selectedModuleId) || null;
    const activeView = selectedModule?.ui_views.find(v => v.view_id === activeViewId) || null;

    // Reset form values when view changes
    useEffect(() => {
        setFormValues({});
        setExecResult(null);
        setErrorMsg(null);
    }, [activeViewId, selectedModuleId]);

    const handleInputChange = (fieldName: string, value: string, type: 'text' | 'number') => {
        setFormValues(prev => ({
            ...prev,
            [fieldName]: type === 'number' ? (value === '' ? '' : Number(value)) : value,
        }));
    };

    // Execute the module tool via API
    const handleExecuteTool = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!tenantId || !sessionKey || !selectedModule || !activeView) return;

        // Form validation
        for (const field of activeView.fields) {
            if (field.required && (formValues[field.name] === undefined || formValues[field.name] === '')) {
                setErrorMsg(`El campo "${field.label}" es requerido.`);
                return;
            }
        }

        setExecutingTool(activeView.tool_name);
        setExecResult(null);
        setErrorMsg(null);

        try {
            // Remove empty fields
            const cleanArgs: Record<string, string | number> = {};
            Object.entries(formValues).forEach(([key, val]) => {
                if (val !== '' && val !== undefined) {
                    cleanArgs[key] = val;
                }
            });

            const res = await fetch(`/api/router/modules/${selectedModule.module_id}/execute`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({
                    tool_name: activeView.tool_name,
                    arguments: cleanArgs,
                }),
            });

            if (!res.ok) {
                const errData = await res.json().catch(() => ({}));
                throw new Error(errData.error || `HTTP ${res.status}`);
            }

            const data = await res.json();
            setExecResult(data);
        } catch (err: unknown) {
            console.error('Manual execution failed:', err);
            setErrorMsg((err as Error).message || 'Fallo en la ejecución del comando.');
        } finally {
            setExecutingTool(null);
        }
    };

    return (
        <div className="col-span-12 space-y-6">
            <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                    <div className="p-2 rounded-xl bg-aegis-cyan/10 text-aegis-cyan">
                        <Puzzle className="w-5 h-5" />
                    </div>
                    <div>
                        <h2 className="text-xl font-bold uppercase tracking-widest text-white">Microkernel Domain Modules</h2>
                        <p className="text-[10px] font-mono text-white/20 uppercase tracking-widest">— Dynamic Client Shell & SDUI Panels</p>
                    </div>
                </div>
                <button
                    onClick={() => fetchModules(false)}
                    disabled={refreshing}
                    className="px-3 py-1.5 border border-white/10 text-white/60 hover:text-white hover:border-white/30 text-[9px] font-mono uppercase tracking-widest rounded-lg transition-colors flex items-center gap-2"
                >
                    {refreshing ? <Loader2 className="w-3 h-3 animate-spin" /> : 'Refrescar'}
                </button>
            </div>

            {fetchError ? (
                <div className="glass p-6 rounded-2xl border border-red-500/20 flex flex-col items-center justify-center gap-3">
                    <AlertTriangle className="w-6 h-6 text-red-500/60" />
                    <p className="text-xs font-mono text-white/40 uppercase tracking-widest">Error Loading Modules Catalog</p>
                    <p className="text-[10px] font-mono text-red-400 bg-red-950/20 px-4 py-2 border border-red-500/10 rounded-xl max-w-md text-center leading-relaxed">
                        {fetchError}
                    </p>
                </div>
            ) : modules.length === 0 && !refreshing ? (
                <div className="glass p-8 rounded-2xl border border-white/5 flex flex-col items-center justify-center gap-3 text-center">
                    <Puzzle className="w-8 h-8 text-white/10 animate-pulse" />
                    <p className="text-xs font-mono text-white/30 uppercase">No Microkernel Modules Discovered</p>
                    <p className="text-[10px] font-mono text-white/20 max-w-[280px] leading-relaxed">
                        El escáner del microkernel no encontró ningún archivo <code className="text-aegis-cyan">module.json</code> válido.
                    </p>
                </div>
            ) : (
                <div className="grid grid-cols-12 gap-8">
                    {/* Left: Discovered Modules List */}
                    <div className="col-span-12 lg:col-span-4 flex flex-col gap-4">
                        <div className="glass p-4 rounded-2xl border border-white/10 flex-1 flex flex-col gap-3 min-h-[300px]">
                            <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mb-1">Módulos Escaneados</p>
                            <div className="space-y-3 flex-1 overflow-y-auto">
                                {modules.map(module => {
                                    const isSelected = module.module_id === selectedModuleId;
                                    return (
                                        <div
                                            key={module.module_id as string}
                                            onClick={() => {
                                                setSelectedModuleId(module.module_id as string);
                                                if (module.ui_views && module.ui_views.length > 0) {
                                                    setActiveViewId(module.ui_views[0].view_id);
                                                } else {
                                                    setActiveViewId(null);
                                                }
                                            }}
                                            className={`p-4 rounded-xl border transition-all cursor-pointer flex flex-col gap-3 group relative overflow-hidden ${
                                                isSelected
                                                    ? 'bg-white/[0.04] border-aegis-cyan/40 shadow-[0_0_15px_rgba(0,255,255,0.05)]'
                                                    : 'bg-white/[0.01] border-white/5 hover:border-white/10 hover:bg-white/[0.02]'
                                            }`}
                                        >
                                            {/* Selection glow */}
                                            {isSelected && (
                                                <div className="absolute -left-10 top-0 bottom-0 w-12 bg-aegis-cyan/10 blur-xl pointer-events-none" />
                                            )}

                                            <div className="flex items-start justify-between">
                                                <div>
                                                    <h3 className="text-xs font-bold text-white group-hover:text-aegis-cyan transition-colors">
                                                        {module.display_name}
                                                    </h3>
                                                    <p className="text-[9px] font-mono text-white/30 truncate max-w-[190px] mt-0.5">
                                                        {module.module_id}
                                                    </p>
                                                </div>
                                                <span className="text-[8px] font-mono bg-white/5 border border-white/10 px-2 py-0.5 rounded text-white/40">
                                                    v{module.version}
                                                </span>
                                            </div>

                                            <div className="flex items-center justify-between border-t border-white/5 pt-2.5">
                                                <span className="text-[9px] font-mono text-white/30">
                                                    {module.exposed_tools.length} Herramientas
                                                </span>
                                                <div className="flex items-center gap-2" onClick={e => e.stopPropagation()}>
                                                    <label
                                                        htmlFor={`toggle-${module.module_id}`}
                                                        className={`text-[9px] font-mono uppercase cursor-pointer ${module.active ? 'text-green-400' : 'text-white/20'}`}
                                                    >
                                                        {module.active ? 'Activo' : 'Inactivo'}
                                                    </label>
                                                    <button
                                                        id={`toggle-${module.module_id}`}
                                                        onClick={() => toggleModule(module.module_id as string, module.active)}
                                                        disabled={isLoading}
                                                        role="switch"
                                                        aria-checked={module.active}
                                                        aria-label={`Activar/desactivar m?dulo ${module.display_name}`}
                                                        className={`relative w-8 h-4.5 rounded-full transition-colors flex items-center px-0.5 ${
                                                            module.active ? 'bg-green-500/20 border border-green-500/30' : 'bg-white/5 border border-white/10'
                                                        }`}
                                                    >
                                                        <motion.div
                                                            animate={{ x: module.active ? 13 : 0 }}
                                                            transition={{ type: 'spring', stiffness: 500, damping: 30 }}
                                                            className={`w-3.5 h-3.5 rounded-full ${
                                                                module.active ? 'bg-green-400 shadow-[0_0_6px_rgba(74,222,128,0.5)]' : 'bg-white/20'
                                                            }`}
                                                        />
                                                    </button>
                                                </div>
                                            </div>
                                        </div>
                                    );
                                })}
                            </div>
                        </div>
                    </div>

                    {/* Right: Dynamic View Panel */}
                    <div className="col-span-12 lg:col-span-8">
                        <div className="glass p-6 rounded-2xl border border-white/10 min-h-[450px] flex flex-col gap-6">
                            {selectedModule ? (
                                <>
                                    {/* Module title & description */}
                                    <div className="flex flex-col gap-1 border-b border-white/5 pb-4">
                                        <h3 className="text-sm font-bold text-white uppercase tracking-wider flex items-center gap-2">
                                            <Cpu className="w-4 h-4 text-aegis-cyan" />
                                            {selectedModule.display_name}
                                        </h3>
                                        <p className="text-[10px] font-mono text-white/30 uppercase tracking-widest mt-0.5">
                                            VISTAS DE INTERFAZ DINÁMICAS (SERVER-DRIVEN UI)
                                        </p>
                                    </div>

                                    {!selectedModule.active ? (
                                        <div className="flex-1 flex flex-col items-center justify-center gap-4 text-center py-12">
                                            <div className="p-3 bg-yellow-500/10 border border-yellow-500/20 text-yellow-500 rounded-full animate-pulse">
                                                <AlertTriangle className="w-6 h-6" />
                                            </div>
                                            <div className="space-y-1">
                                                <p className="text-xs font-mono font-bold text-white/80 uppercase">Módulo Inactivo</p>
                                                <p className="text-[10px] font-mono text-white/30 max-w-[280px] leading-relaxed mx-auto">
                                                    Activá este módulo usando el switch de la izquierda o pedile al asistente que lo instale de forma segura para exponer sus formularios interactivos.
                                                </p>
                                            </div>
                                        </div>
                                    ) : selectedModule.ui_views.length === 0 ? (
                                        <div className="flex-1 flex flex-col items-center justify-center gap-3 text-center py-12">
                                            <Settings className="w-6 h-6 text-white/10" />
                                            <p className="text-xs font-mono text-white/30 uppercase">Sin vistas visuales definidas</p>
                                            <p className="text-[10px] font-mono text-white/20 max-w-[260px] leading-relaxed mx-auto">
                                                Este módulo está activo pero no expone un esquema de interfaz gráfica dinámica (<code className="text-aegis-purple">ui_views</code>).
                                            </p>
                                        </div>
                                    ) : (
                                        <>
                                            {/* Sub-tabs bar */}
                                            <div className="flex gap-2 border-b border-white/5 pb-0.5">
                                                {selectedModule.ui_views.map(view => {
                                                    const ViewIcon = view.icon ? (iconMap[view.icon] || Puzzle) : Puzzle;
                                                    const isTabActive = view.view_id === activeViewId;
                                                    return (
                                                        <button
                                                            key={view.view_id}
                                                            onClick={() => setActiveViewId(view.view_id)}
                                                            className={`px-4 py-2 border-b-2 text-[10px] font-mono uppercase tracking-widest transition-all flex items-center gap-2 ${
                                                                isTabActive
                                                                    ? 'border-aegis-cyan text-aegis-cyan font-bold bg-white/[0.01]'
                                                                    : 'border-transparent text-white/40 hover:text-white hover:bg-white/[0.005]'
                                                            }`}
                                                        >
                                                            <ViewIcon className="w-3.5 h-3.5" />
                                                            {view.title}
                                                        </button>
                                                    );
                                                })}
                                            </div>

                                            {/* Dynamic Form Area */}
                                            <div className="flex-1 grid grid-cols-1 md:grid-cols-2 gap-6 items-start">
                                                {activeView ? (
                                                    <form onSubmit={handleExecuteTool} className="space-y-4">
                                                        <div className="space-y-1">
                                                            <h4 className="text-xs font-bold text-white">{activeView.title}</h4>
                                                            {activeView.description && (
                                                                <p className="text-[10px] text-white/40 leading-relaxed">{activeView.description}</p>
                                                            )}
                                                        </div>

                                                        <div className="space-y-3.5 pt-2">
                                                            {activeView.fields.map(field => (
                                                                <div key={field.name} className="flex flex-col gap-1.5">
                                                                    <label htmlFor={`input-${field.name}`} className="text-[9px] font-mono uppercase tracking-wider text-white/45 flex items-center gap-1">
                                                                        {field.label}
                                                                        {field.required && <span className="text-red-500">*</span>}
                                                                    </label>
                                                                    <input
                                                                        id={`input-${field.name}`}
                                                                        type={field.type === 'number' ? 'number' : 'text'}
                                                                        required={field.required}
                                                                        placeholder={field.placeholder || ''}
                                                                        step={field.type === 'number' ? 'any' : undefined}
                                                                        value={formValues[field.name] === undefined ? '' : formValues[field.name]}
                                                                        onChange={e => handleInputChange(field.name, e.target.value, field.type)}
                                                                        className="w-full bg-white/[0.03] border border-white/10 focus:border-aegis-cyan/60 rounded-xl px-3.5 py-2 text-xs text-white placeholder-white/20 focus:outline-none transition-colors font-mono"
                                                                    />
                                                                </div>
                                                            ))}
                                                        </div>

                                                        {errorMsg && (
                                                            <p className="text-[9px] font-mono text-red-400 bg-red-950/20 border border-red-500/10 px-3 py-2 rounded-lg leading-relaxed uppercase tracking-wider">
                                                                {errorMsg}
                                                            </p>
                                                        )}

                                                        <button
                                                            type="submit"
                                                            disabled={!!executingTool}
                                                            className="w-full bg-aegis-cyan/10 border border-aegis-cyan/35 text-aegis-cyan hover:bg-aegis-cyan/25 hover:border-aegis-cyan/50 font-mono text-[10px] uppercase tracking-widest py-3 rounded-xl transition-all flex items-center justify-center gap-2.5 font-bold shadow-[0_0_12px_rgba(0,255,255,0.02)]"
                                                        >
                                                            {executingTool ? (
                                                                <>
                                                                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                                                                    EJECUTANDO...
                                                                </>
                                                            ) : (
                                                                <>
                                                                    <Play className="w-3 h-3 fill-aegis-cyan" />
                                                                    EJECUTAR HERRAMIENTA
                                                                </>
                                                            )}
                                                        </button>
                                                    </form>
                                                ) : (
                                                    <p className="text-xs font-mono text-white/20">Selecciona una vista</p>
                                                )}

                                                {/* Output Panel */}
                                                <div className="flex flex-col h-full min-h-[300px] border border-white/5 bg-white/[0.005] rounded-2xl p-4 gap-3 overflow-hidden">
                                                    <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest border-b border-white/5 pb-2">
                                                        Resultado del Microkernel
                                                    </p>
                                                    <div className="flex-1 overflow-auto flex flex-col font-mono text-[10px]">
                                                        {executingTool ? (
                                                            <div className="flex-1 flex flex-col items-center justify-center gap-2">
                                                                <Loader2 className="w-5 h-5 text-aegis-cyan animate-spin" />
                                                                <span className="text-[9px] uppercase tracking-wider text-white/20 animate-pulse">Llamando gRPC...</span>
                                                            </div>
                                                        ) : execResult ? (
                                                            <div className="space-y-3 flex-1 flex flex-col">
                                                                {execResult.success ? (
                                                                    <div className="flex items-center gap-2 text-green-400 bg-green-500/10 border border-green-500/20 px-3 py-1.5 rounded-lg shrink-0">
                                                                        <CheckCircle2 className="w-3.5 h-3.5 shrink-0" />
                                                                        <span className="uppercase text-[8px] font-bold tracking-wider">EJECUCIÓN EXITOSA</span>
                                                                    </div>
                                                                ) : (
                                                                    <div className="flex items-center gap-2 text-red-400 bg-red-500/10 border border-red-500/20 px-3 py-1.5 rounded-lg shrink-0">
                                                                        <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
                                                                        <span className="uppercase text-[8px] font-bold tracking-wider">FALLO DE EJECUCIÓN</span>
                                                                    </div>
                                                                )}
                                                                <pre className="flex-1 bg-black/40 border border-white/5 rounded-xl p-3.5 text-[9px] overflow-auto text-white/70 leading-relaxed select-all">
                                                                    {JSON.stringify(execResult.result || execResult.error, null, 2)}
                                                                </pre>
                                                            </div>
                                                        ) : (
                                                            <div className="flex-1 flex items-center justify-center text-center text-white/10 px-4 py-8">
                                                                Rellena el formulario e invoca la herramienta para ver los registros del ledger en tiempo real.
                                                            </div>
                                                        )}
                                                    </div>
                                                </div>
                                            </div>
                                        </>
                                    )}
                                </>
                            ) : (
                                <div className="flex-1 flex items-center justify-center">
                                    <p className="text-xs font-mono text-white/20 uppercase">Selecciona un módulo</p>
                                </div>
                            )}
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};

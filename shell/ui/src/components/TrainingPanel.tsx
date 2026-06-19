import React, { useState, useEffect, useRef } from 'react';
import { motion } from 'framer-motion';
import { 
    Cpu, 
    Cloud, 
    Play, 
    Square, 
    Activity, 
    Terminal as TerminalIcon, 
    ChevronRight,
    AlertTriangle
} from 'lucide-react';
import { useAegisStore, TrainingConfig } from '../store/useAegisStore';
import { useTranslation } from '../i18n';

interface ChartPoint {
    step: number;
    loss: number;
}

// ── SVG Real-time Loss Chart ──────────────────────────────────────────────────
const LossChart: React.FC<{ points: ChartPoint[] }> = ({ points }) => {
    if (points.length < 2) {
        return (
            <div className="h-56 border border-white/5 bg-white/[0.01] rounded-2xl flex flex-col items-center justify-center gap-3">
                <div className="relative">
                    <div className="w-2 h-2 rounded-full bg-aegis-cyan animate-ping absolute inset-0" />
                    <div className="w-2 h-2 rounded-full bg-aegis-cyan" />
                </div>
                <span className="text-[10px] font-mono text-white/30 uppercase tracking-widest">
                    Esperando métricas para trazar curva...
                </span>
            </div>
        );
    }

    const margin = { top: 20, right: 20, bottom: 30, left: 45 };
    const width = 600;
    const height = 224;

    const xMax = width - margin.left - margin.right;
    const yMax = height - margin.top - margin.bottom;

    const steps = points.map(p => p.step);
    const losses = points.map(p => p.loss);

    const minX = Math.min(...steps);
    const maxX = Math.max(...steps);
    const minY = 0;
    const maxY = Math.max(...losses) * 1.15; // 15% padding on top

    const scaleX = (x: number) => margin.left + ((x - minX) / (maxX - minX || 1)) * xMax;
    const scaleY = (y: number) => margin.top + yMax - ((y - minY) / (maxY - minY || 1)) * yMax;

    // Generar path SVG
    const pathD = points.reduce((acc, p, i) => {
        const x = scaleX(p.step);
        const y = scaleY(p.loss);
        return i === 0 ? `M ${x} ${y}` : `${acc} L ${x} ${y}`;
    }, '');

    // Generar path de gradiente inferior para efecto glow
    const areaD = `${pathD} L ${scaleX(points[points.length - 1].step)} ${margin.top + yMax} L ${scaleX(points[0].step)} ${margin.top + yMax} Z`;

    const yTicks = 4;
    const xTicks = 5;

    return (
        <div className="relative w-full h-56 bg-white/[0.01] border border-white/5 rounded-2xl p-2">
            <svg viewBox={`0 0 ${width} ${height}`} className="w-full h-full text-white/10 font-mono text-[9px]">
                <defs>
                    <linearGradient id="lossAreaGradient" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stopColor="#00ffff" stopOpacity="0.15" />
                        <stop offset="100%" stopColor="#00ffff" stopOpacity="0.0" />
                    </linearGradient>
                </defs>

                {/* Grid Y */}
                {Array.from({ length: yTicks }).map((_, i) => {
                    const val = minY + (i / (yTicks - 1)) * (maxY - minY);
                    const y = scaleY(val);
                    return (
                        <g key={i}>
                            <line x1={margin.left} y1={y} x2={width - margin.right} y2={y} stroke="currentColor" strokeDasharray="4,4" />
                            <text x={margin.left - 8} y={y + 3} textAnchor="end" fill="rgba(255,255,255,0.3)" className="font-mono">
                                {val.toFixed(3)}
                            </text>
                        </g>
                    );
                })}

                {/* Grid X */}
                {Array.from({ length: xTicks }).map((_, i) => {
                    const val = minX + (i / (xTicks - 1)) * (maxX - minX);
                    const x = scaleX(val);
                    return (
                        <g key={i}>
                            <line x1={x} y1={margin.top} x2={x} y2={height - margin.bottom} stroke="currentColor" strokeDasharray="4,4" />
                            <text x={x} y={height - margin.bottom + 14} textAnchor="middle" fill="rgba(255,255,255,0.3)" className="font-mono">
                                Step {Math.round(val)}
                            </text>
                        </g>
                    );
                })}

                {/* Gradient Fill under path */}
                <path d={areaD} fill="url(#lossAreaGradient)" />

                {/* Path line */}
                <path 
                    d={pathD} 
                    fill="none" 
                    stroke="#00ffff" 
                    strokeWidth="2" 
                    strokeLinecap="round" 
                    strokeLinejoin="round" 
                    className="drop-shadow-[0_0_6px_rgba(0,255,255,0.4)]"
                />

                {/* Active Dots */}
                {points.map((p, i) => (
                    <g key={i} className="group">
                        <circle 
                            cx={scaleX(p.step)} 
                            cy={scaleY(p.loss)} 
                            r="3" 
                            fill="#00ffff" 
                            stroke="#000"
                            strokeWidth="1"
                        />
                        <title>{`Paso ${p.step}: Loss ${p.loss.toFixed(4)}`}</title>
                    </g>
                ))}
            </svg>
        </div>
    );
};

// ── Main TrainingPanel Component ──────────────────────────────────────────────
export const TrainingPanel: React.FC<{ tenantId: string; sessionKey: string }> = () => {
    const { t } = useTranslation();
    const { 
        trainingProgress, 
        trainingLogs, 
        trainingSocket, 
        startTraining, 
        cancelTraining, 
        connectTrainingStream,
        fetchTrainingStatus,
        system_metrics
    } = useAegisStore();

    const vramPercent = system_metrics.vram_total_mb > 0 
        ? (system_metrics.vram_allocated_mb / system_metrics.vram_total_mb) * 100 
        : 0;

    // Local form states
    const [mode, setMode] = useState<'local' | 'cloud'>('local');
    const [modelId, setModelId] = useState('qwen3.6-7b-instruct');
    const [isCustomModel, setIsCustomModel] = useState(false);
    const [epochs, setEpochs] = useState(3);
    const [learningRate, setLearningRate] = useState(2e-5);
    const [batchSize, setBatchSize] = useState(4);
    const [datasetPath, setDatasetPath] = useState('tools/fine-tuning/dataset.jsonl');
    const [cloudApiKey, setCloudApiKey] = useState('');
    const [isSubmitting, setIsSubmitting] = useState(false);

    // Curve metric accumulation
    const [points, setPoints] = useState<ChartPoint[]>([]);

    const consoleEndRef = useRef<HTMLDivElement | null>(null);

    // Sync training status on load
    useEffect(() => {
        fetchTrainingStatus();
    }, [fetchTrainingStatus]);

    // WebSocket auto-reconnect if running
    useEffect(() => {
        if (trainingProgress) {
            const statusStr = typeof trainingProgress.status === 'string' 
                ? trainingProgress.status 
                : Object.keys(trainingProgress.status)[0]; // Handles Failed(String) shapes
            
            if (['Preparing', 'Training', 'Exporting'].includes(statusStr) && !trainingSocket) {
                connectTrainingStream();
            }
        }
    }, [trainingProgress, trainingSocket, connectTrainingStream]);

    // Track metrics history as they arrive
    useEffect(() => {
        if (trainingProgress) {
            const statusStr = typeof trainingProgress.status === 'string' 
                ? trainingProgress.status 
                : Object.keys(trainingProgress.status)[0];

            if (statusStr === 'Training' && trainingProgress.step > 0 && trainingProgress.loss > 0) {
                setPoints(prev => {
                    const exists = prev.some(p => p.step === trainingProgress.step);
                    if (exists) return prev;
                    return [...prev, { step: trainingProgress.step, loss: trainingProgress.loss }].sort((a, b) => a.step - b.step);
                });
            } else if (statusStr === 'Preparing') {
                // Reiniciar puntos al iniciar un nuevo entrenamiento
                setPoints([]);
            }
        }
    }, [trainingProgress]);

    // Auto-scroll logs terminal
    useEffect(() => {
        if (consoleEndRef.current) {
            consoleEndRef.current.scrollIntoView({ behavior: 'smooth' });
        }
    }, [trainingLogs]);

    const isRunning = (() => {
        if (!trainingProgress) return false;
        const statusStr = typeof trainingProgress.status === 'string' 
            ? trainingProgress.status 
            : Object.keys(trainingProgress.status)[0];
        return ['Preparing', 'Training', 'Exporting'].includes(statusStr);
    })();

    const activeStatus = (() => {
        if (!trainingProgress) return 'Idle';
        const status = trainingProgress.status;
        if (typeof status === 'string') return status;
        if (typeof status === 'object' && status !== null) {
            if ('Failed' in status) return `Failed: ${status.Failed}`;
            return Object.keys(status)[0] || 'Unknown';
        }
        return 'Idle';
    })();

    const handleStart = async (e: React.FormEvent) => {
        e.preventDefault();
        setIsSubmitting(true);
        setPoints([]); // Reset metrics
        
        const config: TrainingConfig = {
            mode,
            model_id: modelId,
            dataset_path: datasetPath,
            epochs,
            learning_rate: learningRate,
            batch_size: batchSize,
            cloud_api_key: mode === 'cloud' && cloudApiKey ? cloudApiKey : null
        };

        await startTraining(config);
        setIsSubmitting(false);
    };

    const handleCancel = async () => {
        if (confirm('¿Estás seguro de que deseas cancelar la ejecución del entrenamiento? Se enviará una señal de detención al subproceso.')) {
            await cancelTraining();
        }
    };

    const formatETA = (seconds: number) => {
        if (seconds === 0) return '--:--';
        const mins = Math.floor(seconds / 60);
        const secs = seconds % 60;
        return `${mins}:${secs.toString().padStart(2, '0')} min`;
    };

    return (
        <div className="grid grid-cols-12 gap-6 max-w-5xl mx-auto pb-12">
            
            {/* ── SECCIÓN IZQUIERDA: CONFIGURACIÓN ──────────────────────────────── */}
            <div className="col-span-12 lg:col-span-5 flex flex-col gap-6">
                <div className="glass p-6 rounded-2xl border border-white/10 flex flex-col gap-4">
                    <div>
                        <h3 className="text-sm font-mono font-bold tracking-widest text-white uppercase flex items-center gap-2">
                            <Cpu className="w-4 h-4 text-aegis-cyan" />
                            {t('training_setup') || 'Ajustes del Motor'}
                        </h3>
                        <p className="text-[9px] font-mono text-white/30 uppercase mt-0.5">
                            Parámetros y optimizaciones de fine-tuning
                        </p>
                    </div>

                    <form onSubmit={handleStart} className="space-y-4 text-xs font-mono">
                        
                        {/* Selector de Modo */}
                        <div className="space-y-1">
                            <label className="text-[10px] text-white/40 uppercase tracking-widest">Entorno</label>
                            <div className="grid grid-cols-2 gap-2">
                                <button
                                    type="button"
                                    disabled={isRunning}
                                    onClick={() => setMode('local')}
                                    className={`py-2 px-3 rounded-lg border transition-all flex items-center justify-center gap-2 uppercase tracking-wider ${
                                        mode === 'local' 
                                            ? 'bg-aegis-cyan/20 border-aegis-cyan/40 text-aegis-cyan' 
                                            : 'bg-white/2 border-white/5 text-white/40 hover:text-white hover:bg-white/5'
                                    }`}
                                >
                                    <Cpu className="w-3.5 h-3.5" /> Local (GPU)
                                </button>
                                <button
                                    type="button"
                                    disabled={isRunning}
                                    onClick={() => setMode('cloud')}
                                    className={`py-2 px-3 rounded-lg border transition-all flex items-center justify-center gap-2 uppercase tracking-wider ${
                                        mode === 'cloud' 
                                            ? 'bg-aegis-purple/20 border-aegis-purple/40 text-aegis-purple' 
                                            : 'bg-white/2 border-white/5 text-white/40 hover:text-white hover:bg-white/5'
                                    }`}
                                >
                                    <Cloud className="w-3.5 h-3.5" /> Cloud
                                </button>
                            </div>
                        </div>

                        {/* Modelo Base */}
                        <div className="space-y-1">
                            <label className="text-[10px] text-white/40 uppercase tracking-widest">Modelo Base</label>
                            {isCustomModel ? (
                                <div className="flex gap-2">
                                    <input
                                        type="text"
                                        disabled={isRunning}
                                        value={modelId}
                                        onChange={(e) => setModelId(e.target.value)}
                                        className="w-full bg-black/40 border border-white/10 rounded-lg py-2 px-3 text-white placeholder-white/20 focus:border-aegis-cyan/40 focus:outline-none"
                                        placeholder="ej. Qwen/Qwen2.5-7B-Instruct"
                                        required
                                    />
                                    <button
                                        type="button"
                                        disabled={isRunning}
                                        onClick={() => { setIsCustomModel(false); setModelId('qwen3.6-7b-instruct'); }}
                                        className="text-[9px] uppercase tracking-wider text-aegis-cyan underline hover:text-white"
                                    >
                                        Lista
                                    </button>
                                </div>
                            ) : (
                                <div className="flex gap-2">
                                    <select
                                        disabled={isRunning}
                                        value={modelId}
                                        onChange={(e) => setModelId(e.target.value)}
                                        className="w-full bg-black/40 border border-white/10 rounded-lg py-2 px-3 text-white focus:border-aegis-cyan/40 focus:outline-none"
                                    >
                                        <option value="qwen3.6-7b-instruct">Qwen 3.6 (Aegis Base)</option>
                                        <option value="qwen2.5-1.5b-instruct">Qwen 2.5 (1.5B - Bajo Consumo)</option>
                                        <option value="phi-4-mini-instruct">Phi-4-mini (Microsoft)</option>
                                        <option value="llama3.1-8b-instruct">Llama 3.1 (Meta)</option>
                                    </select>
                                    <button
                                        type="button"
                                        disabled={isRunning}
                                        onClick={() => { setIsCustomModel(true); setModelId(''); }}
                                        className="text-[9px] uppercase tracking-wider text-aegis-cyan underline hover:text-white"
                                    >
                                        Custom
                                    </button>
                                </div>
                            )}
                        </div>

                        {/* Hiperparámetros Grid */}
                        <div className="grid grid-cols-2 gap-3">
                            <div className="space-y-1">
                                <label className="text-[10px] text-white/40 uppercase tracking-widest">Épocas</label>
                                <input
                                    type="number"
                                    disabled={isRunning}
                                    min={1}
                                    max={100}
                                    value={epochs}
                                    onChange={(e) => setEpochs(parseInt(e.target.value, 10))}
                                    className="w-full bg-black/40 border border-white/10 rounded-lg py-2 px-3 text-white focus:border-aegis-cyan/40 focus:outline-none"
                                    required
                                />
                            </div>
                            <div className="space-y-1">
                                <label className="text-[10px] text-white/40 uppercase tracking-widest">Batch Size</label>
                                <input
                                    type="number"
                                    disabled={isRunning}
                                    min={1}
                                    max={64}
                                    value={batchSize}
                                    onChange={(e) => setBatchSize(parseInt(e.target.value, 10))}
                                    className="w-full bg-black/40 border border-white/10 rounded-lg py-2 px-3 text-white focus:border-aegis-cyan/40 focus:outline-none"
                                    required
                                />
                            </div>
                        </div>

                        {/* Learning Rate */}
                        <div className="space-y-1">
                            <label className="text-[10px] text-white/40 uppercase tracking-widest">Learning Rate</label>
                            <input
                                type="number"
                                disabled={isRunning}
                                step="0.000001"
                                min="0.000001"
                                max="0.01"
                                value={learningRate}
                                onChange={(e) => setLearningRate(parseFloat(e.target.value))}
                                className="w-full bg-black/40 border border-white/10 rounded-lg py-2 px-3 text-white focus:border-aegis-cyan/40 focus:outline-none"
                                required
                            />
                        </div>

                        {/* Ruta del dataset */}
                        <div className="space-y-1">
                            <label className="text-[10px] text-white/40 uppercase tracking-widest">Archivo Dataset (JSONL)</label>
                            <input
                                type="text"
                                disabled={isRunning}
                                value={datasetPath}
                                onChange={(e) => setDatasetPath(e.target.value)}
                                className="w-full bg-black/40 border border-white/10 rounded-lg py-2 px-3 text-white focus:border-aegis-cyan/40 focus:outline-none"
                                required
                            />
                        </div>

                        {/* API Key (Cloud Only) */}
                        {mode === 'cloud' && (
                            <motion.div 
                                initial={{ opacity: 0, y: -5 }} 
                                animate={{ opacity: 1, y: 0 }}
                                className="space-y-1"
                            >
                                <label className="text-[10px] text-aegis-purple uppercase tracking-widest">API Key de la Nube</label>
                                <input
                                    type="password"
                                    disabled={isRunning}
                                    value={cloudApiKey}
                                    onChange={(e) => setCloudApiKey(e.target.value)}
                                    placeholder="Ingresa clave del proveedor de nube"
                                    className="w-full bg-black/40 border border-white/10 rounded-lg py-2 px-3 text-white focus:border-aegis-purple/40 focus:outline-none"
                                />
                            </motion.div>
                        )}

                        {/* Botones de acción */}
                        <div className="pt-2">
                            {isRunning ? (
                                <button
                                    type="button"
                                    onClick={handleCancel}
                                    className="w-full py-2.5 px-4 rounded-xl bg-red-500/10 border border-red-500/30 text-red-400 font-bold hover:bg-red-500/20 transition-all flex items-center justify-center gap-2 uppercase tracking-wider"
                                >
                                    <Square className="w-4 h-4 fill-red-400" /> Cancelar Entrenamiento
                                </button>
                            ) : (
                                <button
                                    type="submit"
                                    disabled={isSubmitting}
                                    className="w-full py-2.5 px-4 rounded-xl bg-aegis-cyan/20 border border-aegis-cyan/40 text-aegis-cyan font-bold hover:bg-aegis-cyan/30 disabled:opacity-50 transition-all flex items-center justify-center gap-2 uppercase tracking-wider"
                                >
                                    {isSubmitting ? (
                                        <div className="w-4 h-4 rounded-full border-2 border-aegis-cyan border-t-transparent animate-spin" />
                                    ) : (
                                        <Play className="w-4 h-4 fill-aegis-cyan" />
                                    )}
                                    Iniciar Aprendizaje
                                </button>
                            )}
                        </div>
                    </form>
                </div>
            </div>

            {/* ── SECCIÓN DERECHA: MONITOREO Y LOGS ──────────────────────────────── */}
            <div className="col-span-12 lg:col-span-7 flex flex-col gap-6">
                
                {/* Stats & Status */}
                <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                    <div className="glass p-4 rounded-xl border border-white/5 flex flex-col justify-between">
                        <span className="text-[9px] font-mono text-white/30 uppercase tracking-wider">Estado</span>
                        <span className={`text-xs font-mono font-bold uppercase mt-1 ${
                            isRunning 
                                ? 'text-aegis-cyan animate-pulse' 
                                : activeStatus === 'Completed' 
                                ? 'text-green-400' 
                                : activeStatus.startsWith('Failed') 
                                ? 'text-red-400' 
                                : 'text-white/40'
                        }`}>
                            {activeStatus.startsWith('Failed') ? 'Failed' : activeStatus}
                        </span>
                    </div>
                    <div className="glass p-4 rounded-xl border border-white/5 flex flex-col justify-between">
                        <span className="text-[9px] font-mono text-white/30 uppercase tracking-wider">Época</span>
                        <span className="text-xs font-mono font-bold text-white mt-1">
                            {trainingProgress ? trainingProgress.epoch.toFixed(2) : '0.00'}
                        </span>
                    </div>
                    <div className="glass p-4 rounded-xl border border-white/5 flex flex-col justify-between">
                        <span className="text-[9px] font-mono text-white/30 uppercase tracking-wider">Pérdida (Loss)</span>
                        <span className="text-xs font-mono font-bold text-aegis-cyan mt-1">
                            {trainingProgress && trainingProgress.loss > 0 ? trainingProgress.loss.toFixed(4) : '0.0000'}
                        </span>
                    </div>
                    <div className="glass p-4 rounded-xl border border-white/5 flex flex-col justify-between">
                        <span className="text-[9px] font-mono text-white/30 uppercase tracking-wider">Tiempo Restante (ETA)</span>
                        <span className="text-xs font-mono font-bold text-white mt-1">
                            {trainingProgress ? formatETA(trainingProgress.eta_seconds) : '--:--'}
                        </span>
                    </div>
                </div>

                {/* VRAM Telemetry Bar */}
                <div className="glass p-4 rounded-xl border border-white/5 flex flex-col gap-2">
                    <div className="flex justify-between items-center text-[10px] font-mono">
                        <span className="text-white/40 uppercase tracking-widest flex items-center gap-1.5">
                            <Cpu className="w-3.5 h-3.5 text-aegis-cyan" /> Utilización de VRAM
                        </span>
                        <span className="text-white/70">
                            {system_metrics.vram_allocated_mb} MB / {system_metrics.vram_total_mb || 8192} MB ({vramPercent.toFixed(0)}%)
                        </span>
                    </div>
                    <div className="w-full h-2 bg-white/5 rounded-full overflow-hidden">
                        <motion.div 
                            initial={{ width: 0 }}
                            animate={{ width: `${vramPercent}%` }}
                            className={`h-full ${vramPercent > 90 ? 'bg-red-500' : vramPercent > 75 ? 'bg-yellow-500' : 'bg-aegis-cyan'}`}
                        />
                    </div>
                    {vramPercent > 85 && (
                        <p className="text-[9px] font-mono text-yellow-500/80 flex items-center gap-1.5 uppercase">
                            <AlertTriangle className="w-3.5 h-3.5" /> Peligro de CUDA Out Of Memory. Considera reducir el Batch Size o la longitud máxima.
                        </p>
                    )}
                </div>

                {/* Loss Chart */}
                <div className="glass p-6 rounded-2xl border border-white/10 flex flex-col gap-4">
                    <div className="flex items-center justify-between">
                        <div>
                            <h3 className="text-sm font-mono font-bold tracking-widest text-white uppercase flex items-center gap-2">
                                <Activity className="w-4 h-4 text-aegis-cyan" />
                                Curva de Pérdida
                            </h3>
                            <p className="text-[9px] font-mono text-white/30 uppercase mt-0.5">
                                Evolución temporal de la convergencia del modelo
                            </p>
                        </div>
                        {points.length > 0 && (
                            <span className="text-[10px] font-mono text-aegis-cyan/60 uppercase">
                                {points.length} puntos capturados
                            </span>
                        )}
                    </div>
                    <LossChart points={points} />
                </div>

                {/* Live Console Logs */}
                <div className="glass p-6 rounded-2xl border border-white/10 flex-1 flex flex-col gap-4 min-h-[300px] max-h-[350px]">
                    <div className="flex items-center gap-2">
                        <TerminalIcon className="w-4 h-4 text-aegis-cyan" />
                        <h3 className="text-sm font-mono font-bold tracking-widest text-white uppercase">
                            Logs en Tiempo Real
                        </h3>
                    </div>

                    <div className="flex-1 bg-black/60 rounded-xl p-4 border border-white/5 font-mono text-[10px] text-white/70 overflow-y-auto scrollbar-hide flex flex-col gap-1.5 leading-relaxed select-text">
                        {trainingLogs.length === 0 ? (
                            <span className="text-white/20 italic uppercase tracking-wider">
                                [Aegis] No hay sesiones activas de entrenamiento...
                            </span>
                        ) : (
                            trainingLogs.map((log, idx) => (
                                <div key={idx} className="flex gap-2 items-start">
                                    <ChevronRight className="w-3.5 h-3.5 text-white/20 shrink-0 mt-0.5" />
                                    <span>{log}</span>
                                </div>
                            ))
                        )}
                        <div ref={consoleEndRef} />
                    </div>
                </div>

            </div>

        </div>
    );
};

export default TrainingPanel;

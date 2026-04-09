import React, { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Cpu, Database, Activity, LayoutPanelLeft, ChevronRight, ChevronLeft, Zap } from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';
import { useTranslation } from '../i18n';
import TheOrb from './TheOrb';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

const TelemetrySidebar: React.FC = () => {
    const { t } = useTranslation();
    const { system_metrics, status, lastRoutingInfo } = useAegisStore();
    const [isOpen, setIsOpen] = useState(true);

    const vramPercentage = (system_metrics.vram_allocated_mb / system_metrics.vram_total_mb) * 100 || 0;

    return (
        <motion.div
            initial={false}
            animate={{ width: isOpen ? 280 : 64 }}
            className="relative h-screen border-l border-white/5 bg-black/40 backdrop-blur-xl flex flex-col transition-all duration-300 ease-in-out z-30"
        >
            {/* Toggle Button */}
            <button
                onClick={() => setIsOpen(!isOpen)}
                className="absolute -left-3 top-20 w-6 h-6 bg-aegis-steel border border-white/10 rounded-full flex items-center justify-center text-white/60 hover:text-aegis-cyan hover:border-aegis-cyan/40 transition-all"
            >
                {isOpen ? <ChevronRight size={14} /> : <ChevronLeft size={14} />}
            </button>

            {/* Top Section: The Orb */}
            <div className="flex flex-col items-center py-8 gap-4 overflow-hidden">
                <div className={cn("transition-transform duration-500", !isOpen && "scale-50")}>
                    <TheOrb status={status} />
                </div>
                <AnimatePresence>
                    {isOpen && (
                        <motion.div
                            initial={{ opacity: 0, y: -10 }}
                            animate={{ opacity: 1, y: 0 }}
                            exit={{ opacity: 0, y: -10 }}
                            className="text-center"
                        >
                            <h2 className="text-xs font-mono font-bold tracking-[0.3em] text-white/80 uppercase">Aegis Core</h2>
                            <p className="text-[9px] font-mono text-aegis-cyan animate-pulse uppercase mt-1">
                                {t('link_status')}: {t('established')}
                            </p>
                        </motion.div>
                    )}
                </AnimatePresence>
            </div>

            {/* Metrics Section */}
            <div className="flex-1 px-4 py-4 flex flex-col gap-6 overflow-hidden">
                <MetricItem
                    isOpen={isOpen}
                    icon={<Cpu size={18} />}
                    label={t('neural_load')}
                    value={`${system_metrics.cpu_load.toFixed(1)}%`}
                    percentage={system_metrics.cpu_load}
                    color="cyan"
                />

                <MetricItem
                    isOpen={isOpen}
                    icon={<Database size={18} />}
                    label={t('vram_allocation')}
                    value={`${(system_metrics.vram_allocated_mb / 1024).toFixed(1)}GB`}
                    percentage={vramPercentage}
                    color={vramPercentage > 90 ? "red" : "purple"}
                />

                <MetricItem
                    isOpen={isOpen}
                    icon={<Activity size={18} />}
                    label={t('memory_swarm')}
                    value={`${system_metrics.active_workers} ${t('nodes')}`}
                    percentage={100} // Static for now
                    color="steel"
                    hideBar
                />

                {lastRoutingInfo && isOpen && (
                    <div className="border-t border-white/5 pt-4">
                        <p className="text-[9px] font-mono text-white/20 uppercase tracking-[0.2em] mb-3">{t('last_process')}</p>
                        <div className="space-y-2">
                            <div className="flex justify-between items-center">
                                <span className="text-[9px] font-mono text-white/30">{t('model')}</span>
                                <span className="text-[9px] font-mono text-aegis-cyan truncate max-w-[120px]" title={lastRoutingInfo.model_id}>
                                    {lastRoutingInfo.model_id.split('/').pop() ?? lastRoutingInfo.model_id}
                                </span>
                            </div>
                            <div className="flex justify-between items-center">
                                <span className="text-[9px] font-mono text-white/30">Provider</span>
                                <span className="text-[9px] font-mono text-white/60">{lastRoutingInfo.provider}</span>
                            </div>
                            <div className="flex justify-between items-center">
                                <span className="text-[9px] font-mono text-white/30">{t('status')}</span>
                                <span className="text-[9px] font-mono text-white/60">{lastRoutingInfo.task_type}</span>
                            </div>
                            {lastRoutingInfo.latency_ms > 0 && (
                                <div className="flex justify-between items-center">
                                    <span className="text-[9px] font-mono text-white/30">{t('latency')}</span>
                                    <span className="text-[9px] font-mono text-white/60">{(lastRoutingInfo.latency_ms / 1000).toFixed(1)}s</span>
                                </div>
                            )}
                        </div>
                    </div>
                )}

                <div className="mt-auto pb-8">
                    <AnimatePresence>
                        {isOpen ? (
                            <motion.div
                                initial={{ opacity: 0 }}
                                animate={{ opacity: 1 }}
                                exit={{ opacity: 0 }}
                                className="bg-white/5 border border-white/10 rounded-lg p-3"
                            >
                                <div className="flex items-center gap-2 mb-2">
                                    <Zap size={14} className="text-aegis-cyan" />
                                    <span className="text-[10px] font-mono font-bold text-white/40 uppercase">{t('active_engine')}</span>
                                </div>
                                <div className="text-[11px] font-mono text-aegis-cyan bg-black/40 p-2 rounded border border-aegis-cyan/10">
                                    ANK-V1::PROTOCOL-CITADEL
                                </div>
                            </motion.div>
                        ) : (
                            <div className="flex flex-col items-center gap-4 text-white/20">
                                <LayoutPanelLeft size={20} />
                            </div>
                        )}
                    </AnimatePresence>
                </div>
            </div>
        </motion.div>
    );
};

interface MetricItemProps {
    isOpen: boolean;
    icon: React.ReactNode;
    label: string;
    value: string;
    percentage: number;
    color: 'cyan' | 'purple' | 'red' | 'steel';
    hideBar?: boolean;
}

const MetricItem: React.FC<MetricItemProps> = ({ isOpen, icon, label, value, percentage, color, hideBar }) => {
    const colorMap = {
        cyan: 'bg-aegis-cyan shadow-[0_0_8px_rgba(0,242,254,0.4)]',
        purple: 'bg-aegis-purple shadow-[0_0_8px_rgba(191,0,255,0.4)]',
        red: 'bg-red-500 shadow-[0_0_8px_rgba(239,68,68,0.4)]',
        steel: 'bg-aegis-steel'
    };

    const textMap = {
        cyan: 'text-aegis-cyan',
        purple: 'text-aegis-purple',
        red: 'text-red-500',
        steel: 'text-white/40'
    };

    return (
        <div className="flex flex-col gap-2">
            <div className={cn("flex items-center gap-3", !isOpen && "justify-center")}>
                <div className={cn("transition-colors", isOpen ? "text-white/40" : textMap[color])}>
                    {icon}
                </div>
                {isOpen && (
                    <div className="flex justify-between w-full items-baseline">
                        <span className="text-[10px] uppercase font-mono text-white/40 tracking-tighter">{label}</span>
                        <span className={cn("text-xs font-mono font-bold", textMap[color])}>{value}</span>
                    </div>
                )}
            </div>

            {isOpen && !hideBar && (
                <div className="h-1 w-full bg-white/5 rounded-full overflow-hidden">
                    <motion.div
                        initial={{ width: 0 }}
                        animate={{ width: `${Math.min(percentage, 100)}%` }}
                        transition={{ duration: 1, ease: "easeOut" }}
                        className={cn("h-full rounded-full", colorMap[color])}
                    />
                </div>
            )}
        </div>
    );
};

export default TelemetrySidebar;

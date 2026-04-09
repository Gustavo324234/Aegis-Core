import React from 'react';
import { motion } from 'framer-motion';
import { Cpu, Database, Activity, Zap } from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';
import { useTranslation } from '../i18n';
import TheOrb from './TheOrb';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

const TelemetryDashboard: React.FC = () => {
    const { t } = useTranslation();
    const { system_metrics, status } = useAegisStore();
    const vramPercentage = (system_metrics.vram_allocated_mb / system_metrics.vram_total_mb) * 100 || 0;

    return (
        <div className="w-full bg-black/40 backdrop-blur-2xl border-b border-white/5 px-8 py-2 flex items-center justify-between z-40 relative overflow-hidden">
            {/* Subtle background glow */}
            <div className="absolute top-0 left-1/4 w-1/2 h-px bg-gradient-to-r from-transparent via-aegis-cyan/20 to-transparent" />
            
            <div className="flex items-center gap-6">
                {/* The Mini Orb */}
                <div className="flex items-center gap-3">
                    <div className="scale-[0.4] origin-center -my-6 -mx-4">
                        <OpenOrb status={status} />
                    </div>
                    <div className="flex flex-col">
                        <span className="text-[10px] font-mono font-bold tracking-[0.2em] text-white uppercase">Aegis Core</span>
                        <div className="flex items-center gap-1.5">
                            <div className="w-1 h-1 rounded-full bg-aegis-cyan animate-pulse" />
                            <span className="text-[8px] font-mono text-aegis-cyan/60 uppercase">{t('status_active')}</span>
                        </div>
                    </div>
                </div>

                <div className="h-8 w-px bg-white/5 mx-2" />

                {/* Metrics */}
                <div className="flex items-center gap-10">
                    <HorizontalMetric
                        icon={<Cpu size={14} />}
                        label={t('neural_load').split(' ')[0]}
                        value={`${system_metrics.cpu_load.toFixed(1)}%`}
                        percentage={system_metrics.cpu_load}
                        color="cyan"
                    />
                    <HorizontalMetric
                        icon={<Database size={14} />}
                        label="VRAM"
                        value={`${(system_metrics.vram_allocated_mb / 1024).toFixed(1)}GB`}
                        percentage={vramPercentage}
                        color={vramPercentage > 90 ? "red" : "purple"}
                    />
                    <HorizontalMetric
                        icon={<Activity size={14} />}
                        label={t('memory_swarm').split(' ')[0]}
                        value={`${system_metrics.active_workers} ${t('nodes')}`}
                        percentage={100}
                        color="steel"
                        hideBar
                    />
                </div>
            </div>

            <div className="flex items-center gap-4">
                <div className="hidden xl:flex flex-col items-end gap-0.5">
                    <span className="text-[8px] font-mono text-white/20 uppercase tracking-widest">Active Thread</span>
                    <span className="text-[9px] font-mono text-aegis-purple uppercase tracking-widest">Sync::Siren-02</span>
                </div>
                <div className="bg-white/5 border border-white/10 rounded-lg px-3 py-1.5 flex items-center gap-2 group hover:border-aegis-cyan/30 transition-all">
                    <Zap size={12} className="text-aegis-cyan group-hover:animate-pulse" />
                    <span className="text-[9px] font-mono text-white/40 uppercase tracking-[0.2em]">Citadel Protocol</span>
                </div>
            </div>
        </div>
    );
};

// Helper component to fix the naming discrepancy in the legacy code if any
const OpenOrb: React.FC<{ status: any }> = ({ status }) => <TheOrb status={status} />;

interface HorizontalMetricProps {
    icon: React.ReactNode;
    label: string;
    value: string;
    percentage: number;
    color: 'cyan' | 'purple' | 'red' | 'steel';
    hideBar?: boolean;
}

const HorizontalMetric: React.FC<HorizontalMetricProps> = ({ icon, label, value, percentage, color, hideBar }) => {
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
        <div className="flex items-center gap-3 min-w-[120px]">
            <div className={cn("transition-colors", textMap[color])}>
                {icon}
            </div>
            <div className="flex flex-col gap-1 w-full relative">
                <div className="flex justify-between items-baseline gap-4">
                    <span className="text-[8px] uppercase font-mono text-white/30 tracking-widest">{label}</span>
                    <span className={cn("text-[10px] font-mono font-bold", textMap[color])}>{value}</span>
                </div>
                {!hideBar && (
                    <div className="h-0.5 w-full bg-white/5 rounded-full overflow-hidden absolute -bottom-1">
                        <motion.div
                            initial={{ width: 0 }}
                            animate={{ width: `${Math.min(percentage, 100)}%` }}
                            transition={{ duration: 1.5, ease: "circOut" }}
                            className={cn("h-full rounded-full", colorMap[color])}
                        />
                    </div>
                )}
            </div>
        </div>
    );
};

export default TelemetryDashboard;

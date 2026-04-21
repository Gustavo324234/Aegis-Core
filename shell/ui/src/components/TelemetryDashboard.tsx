import React from 'react';
import { motion } from 'framer-motion';
import { Cpu, Database, Activity, Zap } from 'lucide-react';
import { useAegisStore, type SystemStatus } from '../store/useAegisStore';
import { useTranslation } from '../i18n';
import TheOrb from './TheOrb';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

// CORE-126: Panel de telemetría rediseñado — texto legible, más altura, métricas visibles
const TelemetryDashboard: React.FC = () => {
    const { t } = useTranslation();
    const { system_metrics, status, lastRoutingInfo } = useAegisStore();
    const vramPercentage = (system_metrics.vram_allocated_mb / system_metrics.vram_total_mb) * 100 || 0;

    return (
        <div className="w-full bg-black/60 backdrop-blur-2xl border-b border-white/10 px-6 py-3 flex items-center justify-between z-40 relative overflow-hidden shrink-0">
            {/* Subtle background glow */}
            <div className="absolute top-0 left-1/4 w-1/2 h-px bg-gradient-to-r from-transparent via-aegis-cyan/30 to-transparent" />

            <div className="flex items-center gap-6">
                {/* Orb + título */}
                <div className="flex items-center gap-3">
                    <div className="scale-[0.45] origin-center -my-5 -mx-3">
                        <OpenOrb status={status} />
                    </div>
                    <div className="flex flex-col gap-0.5">
                        <span className="text-xs font-mono font-bold tracking-widest text-white uppercase">Aegis Core</span>
                        <div className="flex items-center gap-1.5">
                            <div className="w-1.5 h-1.5 rounded-full bg-aegis-cyan animate-pulse" />
                            <span className="text-[10px] font-mono text-aegis-cyan/70 uppercase tracking-wider">{t('status_active')}</span>
                        </div>
                    </div>
                </div>

                <div className="h-10 w-px bg-white/10 mx-1" />

                {/* Métricas */}
                <div className="flex items-center gap-8">
                    <HorizontalMetric
                        icon={<Cpu size={15} />}
                        label="CARGA"
                        value={`${system_metrics.cpu_load.toFixed(1)}%`}
                        percentage={system_metrics.cpu_load}
                        color="cyan"
                    />
                    <HorizontalMetric
                        icon={<Database size={15} />}
                        label="VRAM"
                        value={`${(system_metrics.vram_allocated_mb / 1024).toFixed(1)}GB`}
                        percentage={vramPercentage}
                        color={vramPercentage > 90 ? "red" : "purple"}
                    />
                    <HorizontalMetric
                        icon={<Activity size={15} />}
                        label="ENJAMBRE"
                        value={`${system_metrics.active_workers} Nodos`}
                        percentage={100}
                        color="steel"
                        hideBar
                    />
                </div>
            </div>

            {/* Derecha: modelo activo + protocolo */}
            <div className="flex items-center gap-4">
                {lastRoutingInfo && (
                    <div className="hidden lg:flex flex-col items-end gap-0.5">
                        <span className="text-[9px] font-mono text-white/30 uppercase tracking-widest">Modelo Activo</span>
                        <span className="text-[11px] font-mono text-aegis-purple font-bold uppercase tracking-wider">
                            {lastRoutingInfo.model_id.split('/').pop()}
                        </span>
                    </div>
                )}
                {lastRoutingInfo && <div className="h-8 w-px bg-white/10" />}
                <div className="bg-white/5 border border-white/10 rounded-lg px-3 py-2 flex items-center gap-2 group hover:border-aegis-cyan/30 transition-all">
                    <Zap size={13} className="text-aegis-cyan group-hover:animate-pulse" />
                    <span className="text-[10px] font-mono text-white/50 uppercase tracking-widest">Citadel Protocol</span>
                </div>
            </div>
        </div>
    );
};

const OpenOrb: React.FC<{ status: SystemStatus }> = ({ status }) => <TheOrb status={status} />;

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
        steel: 'text-white/50'
    };

    return (
        <div className="flex items-center gap-2.5 min-w-[130px]">
            <div className={cn("transition-colors shrink-0", textMap[color])}>
                {icon}
            </div>
            <div className="flex flex-col gap-1 w-full relative">
                <div className="flex justify-between items-baseline gap-3">
                    <span className="text-[9px] uppercase font-mono text-white/40 tracking-widest">{label}</span>
                    <span className={cn("text-[11px] font-mono font-bold", textMap[color])}>{value}</span>
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

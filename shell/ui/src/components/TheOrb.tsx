import React from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { SystemStatus } from '../store/useAegisStore';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface TheOrbProps {
    status: SystemStatus;
}

const TheOrb: React.FC<TheOrbProps> = ({ status }) => {
    // Configuración de variantes para Framer Motion
    const orbVariants = {
        idle: {
            scale: 1,
            rotate: 0,
            filter: 'drop-shadow(0 0 15px rgba(0, 242, 254, 0.4))',
            transition: { duration: 2, repeat: Infinity, ease: "easeInOut" }
        },
        thinking: {
            scale: 1.1,
            rotate: 360,
            filter: 'drop-shadow(0 0 25px rgba(191, 0, 255, 0.7))',
            transition: {
                rotate: { duration: 4, repeat: Infinity, ease: "linear" },
                scale: { duration: 0.5, repeat: Infinity, repeatType: "mirror" as const }
            }
        },
        executing_syscall: {
            scale: [1, 1.05, 0.95, 1.02, 1],
            x: [0, -2, 2, -1, 0],
            filter: 'drop-shadow(0 0 20px rgba(255, 165, 0, 0.6))',
            transition: { duration: 0.2, repeat: Infinity }
        },
        error: {
            scale: [1, 1.2, 1],
            filter: 'drop-shadow(0 0 35px rgba(255, 0, 0, 0.8))',
            transition: { duration: 0.4, repeat: Infinity }
        },
        connecting: {
            scale: [0.8, 1, 0.8],
            opacity: [0.3, 0.8, 0.3],
            transition: { duration: 1.5, repeat: Infinity }
        },
        disconnected: {
            scale: 0.9,
            opacity: 0.5,
            filter: 'grayscale(1) drop-shadow(0 0 5px rgba(255, 255, 255, 0.1))',
        }
    };

    const getOrbColor = () => {
        switch (status) {
            case 'idle': return 'from-aegis-cyan via-aegis-cyan/40 to-black';
            case 'thinking': return 'from-aegis-purple via-aegis-purple/60 to-black';
            case 'executing_syscall': return 'from-orange-500 via-orange-400/50 to-black';
            case 'error': return 'from-red-600 via-red-900 to-black';
            case 'connecting': return 'from-yellow-400 via-yellow-600/30 to-black';
            default: return 'from-gray-600 to-black';
        }
    };

    const getBorderColor = () => {
        switch (status) {
            case 'idle': return 'border-aegis-cyan/30';
            case 'thinking': return 'border-aegis-purple/50';
            case 'executing_syscall': return 'border-orange-500/70';
            case 'error': return 'border-red-600';
            default: return 'border-white/10';
        }
    };

    return (
        <div className="relative w-32 h-32 flex items-center justify-center p-4">
            {/* Halo Exterior (Atmosphere) */}
            <AnimatePresence mode="wait">
                <motion.div
                    key={status + "-halo"}
                    initial={{ opacity: 0, scale: 0.8 }}
                    animate={{ opacity: 1, scale: 1.2 }}
                    exit={{ opacity: 0, scale: 1.5 }}
                    className={cn(
                        "absolute inset-0 rounded-full blur-2xl opacity-20",
                        status === 'idle' && 'bg-aegis-cyan',
                        status === 'thinking' && 'bg-aegis-purple',
                        status === 'executing_syscall' && 'bg-orange-500',
                        status === 'error' && 'bg-red-600'
                    )}
                />
            </AnimatePresence>

            {/* Núcleo del Orbe */}
            <motion.div
                variants={orbVariants}
                animate={status}
                className={cn(
                    "relative w-20 h-20 rounded-full border-2 shadow-inner overflow-hidden flex items-center justify-center",
                    getBorderColor(),
                    "bg-gradient-to-br",
                    getOrbColor()
                )}
            >
                {/* Efectos de Brillo Interno */}
                <div className="absolute inset-0 bg-[radial-gradient(circle_at_30%_30%,rgba(255,255,255,0.4),transparent)]" />

                {/* Anillo de Rotación (Visible en Thinking/Syscall) */}
                {(status === 'thinking' || status === 'executing_syscall') && (
                    <motion.div
                        animate={{ rotate: 360 }}
                        transition={{ duration: 2, repeat: Infinity, ease: "linear" }}
                        className="absolute inset-1 rounded-full border border-dashed border-white/20"
                    />
                )}

                {/* El "Ojo" Central */}
                <div className={cn(
                    "w-4 h-4 rounded-full blur-[1px] shadow-lg",
                    status === 'error' ? 'bg-red-400' : 'bg-white/80'
                )} />
            </motion.div>

            {/* Escaneo Visual (Scanline) */}
            {status !== 'disconnected' && (
                <motion.div
                    animate={{ y: [-40, 40, -40] }}
                    transition={{ duration: 3, repeat: Infinity, ease: "linear" }}
                    className="absolute w-28 h-[1px] bg-white/10 blur-[2px] z-10 pointer-events-none"
                />
            )}
        </div>
    );
};

export default TheOrb;

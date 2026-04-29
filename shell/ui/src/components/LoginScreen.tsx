import React, { useState } from 'react';
import { motion } from 'framer-motion';
import { Lock, User, Terminal, ChevronRight } from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';
import { AegisLogo } from './AegisLogo';
import { useTranslation } from '../i18n';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

const LoginScreen: React.FC<{ onNeedsPasswordReset?: () => void; prefillTenantId?: string | null }> = ({ onNeedsPasswordReset, prefillTenantId }) => {
    const { t } = useTranslation();
    const { authenticate } = useAegisStore();
    const [tenantId, setTenantId] = useState(prefillTenantId || '');
    const [passphrase, setPassphrase] = useState('');
    const [isInitializing, setIsInitializing] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [, setSyncVersion] = useState<string>('Unknown');

    React.useEffect(() => {
        fetch('/api/system/sync_version')
            .then(res => res.json())
            .then(data => setSyncVersion(data.version))
            .catch(() => setSyncVersion('Error'));
    }, []);

    const handleLogin = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!tenantId || !passphrase) return;

        setIsInitializing(true);
        setError(null);

        const result = await authenticate(tenantId, passphrase);

        if (result === 'password_must_change') {
            // Signal App.tsx to show ForcePasswordChangeScreen
            onNeedsPasswordReset?.();
            setIsInitializing(false);
            return;
        }

        if (result === 'failed') {
            setError(t('auth_failed_invalid_credentials'));
            setIsInitializing(false);
        }
        // Si tiene éxito, el Store cambia isAuthenticated y App.tsx desmonta LoginScreen
    };

    return (
        <div className="min-h-screen bg-black flex items-center justify-center p-4 overflow-hidden relative">
            {/* Background Effect */}
            <div className="absolute inset-0 overflow-hidden pointer-events-none">
                <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[800px] bg-aegis-cyan/5 rounded-full blur-[120px]" />
                <div className="absolute top-1/4 left-1/4 w-[400px] h-[400px] bg-aegis-purple/5 rounded-full blur-[100px]" />
            </div>

            <motion.div
                initial={{ opacity: 0, scale: 0.9, y: 20 }}
                animate={{ opacity: 1, scale: 1, y: 0 }}
                transition={{ duration: 0.8, ease: "easeOut" }}
                className="w-full max-w-md z-10"
            >
                <div className="glass p-8 rounded-2xl border border-white/10 shadow-2xl relative overflow-hidden">
                    {/* Scanline Effect */}
                    <div className="absolute inset-0 bg-gradient-to-b from-transparent via-white/5 to-transparent h-24 w-full -translate-y-full animate-[scan_4s_linear_infinity] pointer-events-none" />

                    <div className="flex flex-col items-center mb-8">
                        <div className="relative mb-6">
                            <motion.div
                                animate={{ rotate: 360 }}
                                transition={{ duration: 20, repeat: Infinity, ease: "linear" }}
                                className="absolute -inset-4 border border-dashed border-aegis-cyan/20 rounded-full"
                            />
                            <div className="p-4 rounded-full bg-aegis-cyan/10 border border-aegis-cyan/30 relative">
                                <AegisLogo variant="icon" className="w-10 h-10 text-aegis-cyan" />
                            </div>
                        </div>

                        <h1 className="text-2xl font-bold tracking-[0.2em] text-white uppercase mb-1">
                            AEGIS <span className="text-aegis-cyan">NEURAL KERNEL</span>
                        </h1>
                        <div className="flex items-center gap-2">
                            <div className="h-[1px] w-4 bg-aegis-purple/40" />
                            <p className="text-[10px] font-mono text-aegis-purple uppercase tracking-widest">
                                {t('citadel_protocol_restricted')}
                            </p>
                            <div className="h-[1px] w-4 bg-aegis-purple/40" />
                        </div>
                    </div>

                    <form onSubmit={handleLogin} className="space-y-6">
                        <div className="space-y-2">
                            <label className="text-[10px] font-mono text-white/40 uppercase ml-1 tracking-widest">{t('tenant_id_label')}</label>
                            <div className="relative">
                                <User className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-white/20" />
                                <input
                                    type="text"
                                    value={tenantId}
                                    onChange={(e) => setTenantId(e.target.value)}
                                    placeholder="USER_ID_HEX"
                                    className="w-full bg-black/40 border border-white/10 rounded-lg py-3 pl-10 pr-4 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all placeholder:text-white/10"
                                    required
                                />
                            </div>
                        </div>

                        <div className="space-y-2">
                            <label className="text-[10px] font-mono text-white/40 uppercase ml-1 tracking-widest">{t('citadel_access_phrase_label')}</label>
                            <div className="relative">
                                <Lock className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-white/20" />
                                <input
                                    type="password"
                                    value={passphrase}
                                    onChange={(e) => setPassphrase(e.target.value)}
                                    placeholder="••••••••••••"
                                    className="w-full bg-black/40 border border-white/10 rounded-lg py-3 pl-10 pr-4 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all placeholder:text-white/10"
                                    required
                                />
                            </div>
                        </div>

                        {error && (
                            <motion.div
                                initial={{ opacity: 0, x: -10 }}
                                animate={{ opacity: 1, x: 0 }}
                                className="bg-red-500/10 border border-red-500/30 p-3 rounded-lg flex items-center gap-3"
                            >
                                <Terminal className="w-4 h-4 text-red-500" />
                                <span className="text-[10px] font-mono text-red-400 leading-tight">
                                    {error}
                                </span>
                            </motion.div>
                        )}

                        <button
                            type="submit"
                            disabled={isInitializing}
                            className={cn(
                                "w-full group relative overflow-hidden rounded-lg py-4 transition-all duration-500",
                                isInitializing
                                    ? "bg-aegis-purple/20 cursor-wait"
                                    : "bg-aegis-cyan/10 hover:bg-aegis-cyan/20 border border-aegis-cyan/30"
                            )}
                        >
                            <div className="relative z-10 flex items-center justify-center gap-3">
                                {isInitializing ? (
                                    <>
                                        <Terminal className="w-4 h-4 animate-pulse text-aegis-cyan" />
                                        <span className="text-xs font-mono font-bold tracking-widest uppercase text-aegis-cyan">{t('derivating_keys')}</span>
                                    </>
                                ) : (
                                    <>
                                        <span className="text-xs font-mono font-bold tracking-[0.3em] uppercase">{t('init_neural_link')}</span>
                                        <ChevronRight className="w-4 h-4 group-hover:translate-x-1 transition-transform" />
                                    </>
                                )}
                            </div>

                            {/* Button Progress Animation */}
                            {isInitializing && (
                                <motion.div
                                    initial={{ x: '-100%' }}
                                    animate={{ x: '0%' }}
                                    transition={{ duration: 2 }}
                                    className="absolute inset-0 bg-aegis-cyan/10 z-0"
                                />
                            )}
                        </button>
                    </form>

                    <div className="mt-8 pt-6 border-t border-white/5 flex justify-between items-center text-[9px] font-mono text-white/20 uppercase tracking-tighter">
                        <div className="flex flex-col">
                            <span>Citadel Encrypted</span>
                            <span className="text-aegis-cyan/40 underline decoration-aegis-cyan/20">CITADEL-V2-PATCHED</span>
                        </div>
                        <span>v1.1.3-SEC</span>
                    </div>
                </div>

                <p className="text-center mt-6 text-[8px] font-mono text-white/10 uppercase tracking-[0.4em]">
                    {t('warning_ring0_monitoring')}
                </p>
            </motion.div>
        </div>
    );
};

export default LoginScreen;

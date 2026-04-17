import React, { useState } from 'react';
import { motion } from 'framer-motion';
import { Key, Terminal, ChevronRight } from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';
import { useTranslation } from '../i18n';

const ForcePasswordChangeScreen: React.FC<{ onPasswordChanged: () => void }> = ({ onPasswordChanged }) => {
    const { t } = useTranslation();
    const { sessionKey, changeOwnPassword } = useAegisStore();
    const [newPassphrase, setNewPassphrase] = useState('');
    const [confirmPassphrase, setConfirmPassphrase] = useState('');
    const [isUpdating, setIsUpdating] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const handlePasswordChange = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!newPassphrase) return;
        if (newPassphrase === sessionKey) {
            setError(t('error_same_password'));
            return;
        }
        if (newPassphrase !== confirmPassphrase) {
            setError(t('passwords_not_match'));
            return;
        }

        setIsUpdating(true);
        setError(null);

        try {
            // changeOwnPassword usa /api/auth/change_password — no requiere privilegios de admin
            const success = await changeOwnPassword(newPassphrase);
            if (success) {
                onPasswordChanged();
            } else {
                setError(t('error_updating_password'));
            }
        } catch (err) {
            setError(err instanceof Error ? err.message : t('unknown_error'));
        } finally {
            setIsUpdating(false);
        }
    };

    return (
        <div className="min-h-screen bg-black flex items-center justify-center p-4 overflow-hidden relative">
            <div className="absolute inset-0 overflow-hidden pointer-events-none">
                <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[800px] bg-yellow-500/5 rounded-full blur-[120px]" />
            </div>

            <motion.div
                initial={{ opacity: 0, scale: 0.9, y: 20 }}
                animate={{ opacity: 1, scale: 1, y: 0 }}
                transition={{ duration: 0.8, ease: "easeOut" }}
                className="w-full max-w-md z-10"
            >
                <div className="glass p-8 rounded-2xl border border-yellow-500/20 shadow-2xl relative overflow-hidden">
                    <div className="absolute inset-0 bg-gradient-to-b from-transparent via-yellow-500/5 to-transparent h-24 w-full -translate-y-full animate-[scan_4s_linear_infinity] pointer-events-none" />

                    <div className="flex flex-col items-center mb-8">
                        <div className="relative mb-6">
                            <motion.div
                                animate={{ rotate: 360 }}
                                transition={{ duration: 15, repeat: Infinity, ease: "linear" }}
                                className="absolute -inset-4 border border-dashed border-yellow-500/30 rounded-full"
                            />
                            <div className="p-4 rounded-full bg-yellow-500/10 border border-yellow-500/30 relative">
                                <Key className="w-10 h-10 text-yellow-500" />
                            </div>
                        </div>

                        <h1 className="text-xl font-bold tracking-[0.2em] text-white uppercase mb-1 text-center">
                            {t('action_required').split(' ')[0]} <span className="text-yellow-500">{t('action_required').split(' ')[1] || ''}</span>
                        </h1>
                        <p className="text-[10px] font-mono text-yellow-500/60 uppercase tracking-widest text-center mt-2">
                            {t('temp_password_detected')}
                        </p>
                    </div>

                    <form onSubmit={handlePasswordChange} className="space-y-6">
                        <div className="space-y-2">
                            <label className="text-[10px] font-mono text-white/40 uppercase ml-1 tracking-widest">{t('new_citadel_phrase')}</label>
                            <input
                                type="password"
                                value={newPassphrase}
                                onChange={(e) => setNewPassphrase(e.target.value)}
                                placeholder="••••••••••••"
                                className="w-full bg-black/40 border border-white/10 rounded-lg py-3 px-4 text-sm font-mono focus:border-yellow-500/50 focus:ring-0 transition-all placeholder:text-white/10"
                                required
                            />
                        </div>

                        <div className="space-y-2">
                            <label className="text-[10px] font-mono text-white/40 uppercase ml-1 tracking-widest">{t('confirm_password_label')}</label>
                            <input
                                type="password"
                                value={confirmPassphrase}
                                onChange={(e) => setConfirmPassphrase(e.target.value)}
                                placeholder="••••••••••••"
                                className="w-full bg-black/40 border border-white/10 rounded-lg py-3 px-4 text-sm font-mono focus:border-yellow-500/50 focus:ring-0 transition-all placeholder:text-white/10"
                                required
                            />
                        </div>

                        {error && (
                            <motion.div
                                initial={{ opacity: 0, x: -10 }}
                                animate={{ opacity: 1, x: 0 }}
                                className="bg-red-500/10 border border-red-500/30 p-3 rounded-lg flex items-center gap-3"
                            >
                                <Terminal className="w-4 h-4 text-red-500" />
                                <span className="text-[10px] font-mono text-red-400 leading-tight">{error}</span>
                            </motion.div>
                        )}

                        <button
                            type="submit"
                            disabled={isUpdating}
                            className={`w-full group relative overflow-hidden rounded-lg py-4 transition-all duration-500 ${isUpdating
                                ? "bg-yellow-500/20 cursor-wait"
                                : "bg-yellow-500/10 hover:bg-yellow-500/20 border border-yellow-500/30"
                                }`}
                        >
                            <div className="relative z-10 flex items-center justify-center gap-3">
                                {isUpdating ? (
                                    <>
                                        <Terminal className="w-4 h-4 animate-pulse text-yellow-500" />
                                        <span className="text-xs font-mono font-bold tracking-widest uppercase text-yellow-500">{t('securing')}</span>
                                    </>
                                ) : (
                                    <>
                                        <span className="text-xs font-mono font-bold tracking-[0.3em] uppercase text-yellow-100">{t('apply_encryption')}</span>
                                        <ChevronRight className="w-4 h-4 text-yellow-100 group-hover:translate-x-1 transition-transform" />
                                    </>
                                )}
                            </div>
                        </button>
                    </form>

                    <div className="mt-8 pt-6 border-t border-white/5 flex justify-between items-center text-[9px] font-mono text-yellow-500/40 uppercase tracking-tighter">
                        <span>Ring 0 Policy Enforced</span>
                        <span>Zero-Knowledge Mode</span>
                    </div>
                </div>
            </motion.div>
        </div>
    );
};

export default ForcePasswordChangeScreen;

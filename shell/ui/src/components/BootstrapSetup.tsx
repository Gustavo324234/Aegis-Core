import React, { useState } from 'react';
import { motion } from 'framer-motion';
import { Shield, Loader2, CheckCircle2, AlertTriangle, ArrowRight, User, Lock, Key } from 'lucide-react';
import { useTranslation } from '../i18n';
import { useAegisStore } from '../store/useAegisStore';

interface BootstrapSetupProps {
    token: string;
    onComplete: () => void;
}

const BootstrapSetup: React.FC<BootstrapSetupProps> = ({ token, onComplete }) => {
    const { t } = useTranslation();
    const { logout } = useAegisStore();
    const [username, setUsername] = useState('');
    const [password, setPassword] = useState('');
    const [confirm, setConfirm] = useState('');
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [success, setSuccess] = useState(false);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (password !== confirm) {
            setError(t('passwords_not_match'));
            return;
        }

        setIsSubmitting(true);
        setError(null);

        try {
            const res = await fetch('/api/admin/setup-token', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    username,
                    password,
                    setup_token: token
                })
            });

            if (res.ok) {
                logout();
                setSuccess(true);
                setTimeout(() => {
                    onComplete();
                }, 3000);
            } else {
                const data = await res.json();
                if (res.status === 401) {
                    setError(t('token_expired_error'));
                } else {
                    setError(data.detail || t('bootstrap_error'));
                }
            }
        } catch (err) {
            setError(t('bff_communication_error'));
        } finally {
            setIsSubmitting(false);
        }
    };

    if (success) {
        return (
            <div className="flex flex-col items-center justify-center min-h-screen bg-black p-6">
                <motion.div 
                    initial={{ scale: 0, opacity: 0 }}
                    animate={{ scale: 1, opacity: 1 }}
                    className="glass p-12 rounded-3xl border border-green-500/30 flex flex-col items-center text-center max-w-md"
                >
                    <div className="p-4 rounded-full bg-green-500/10 mb-6">
                        <CheckCircle2 className="w-16 h-16 text-green-400" />
                    </div>
                    <h1 className="text-2xl font-bold tracking-[0.2em] uppercase text-white mb-2">{t('master_admin_created')}</h1>
                    <p className="text-[11px] font-mono text-white/40 uppercase tracking-widest leading-relaxed">
                        {t('neural_link_established')}
                    </p>
                </motion.div>
            </div>
        );
    }

    return (
        <div className="flex flex-col items-center justify-center min-h-screen bg-black p-6 relative overflow-hidden">
            {/* Background Effects */}
            <div className="absolute top-0 left-0 w-full h-full pointer-events-none">
                <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[800px] bg-aegis-cyan/5 rounded-full blur-[120px]" />
                <div className="absolute top-0 left-0 w-full h-full bg-[radial-gradient(circle_at_50%_50%,rgba(255,255,255,0.03)_0%,transparent_100%)]" />
            </div>

            <motion.div 
                initial={{ y: 20, opacity: 0 }}
                animate={{ y: 0, opacity: 1 }}
                className="w-full max-w-md relative z-10"
            >
                <div className="flex flex-col items-center mb-10">
                    <div className="p-4 rounded-3xl bg-white/5 border border-white/10 mb-6 shadow-2xl relative group">
                        <div className="absolute inset-0 bg-aegis-cyan/20 blur-xl opacity-0 group-hover:opacity-100 transition-opacity duration-700" />
                        <Shield className="w-12 h-12 text-aegis-cyan relative z-10" />
                    </div>
                    <h1 className="text-2xl font-bold tracking-[0.3em] uppercase text-white">AEGIS OS Bootstrap</h1>
                    <div className="mt-2 flex items-center gap-2">
                        <Key className="w-3 h-3 text-white/20" />
                        <p className="text-[10px] font-mono text-white/30 uppercase tracking-widest">{t('initial_access_procedure')}</p>
                    </div>
                </div>

                <form onSubmit={handleSubmit} className="glass p-8 rounded-3xl border border-white/10 shadow-2xl backdrop-blur-xl relative">
                    <div className="space-y-6">
                        <div className="space-y-2">
                            <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest ml-1">Username</label>
                            <div className="relative">
                                <User className="absolute left-4 top-1/2 -translate-y-1/2 w-4 h-4 text-white/20" />
                                <input 
                                    type="text" 
                                    required
                                    value={username}
                                    onChange={(e) => setUsername(e.target.value)}
                                    placeholder="admin"
                                    className="w-full bg-black/40 border border-white/5 rounded-xl py-3.5 px-12 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all placeholder:text-white/10"
                                />
                            </div>
                        </div>

                        <div className="space-y-2">
                            <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest ml-1">Password</label>
                            <div className="relative">
                                <Lock className="absolute left-4 top-1/2 -translate-y-1/2 w-4 h-4 text-white/20" />
                                <input 
                                    type="password" 
                                    required
                                    value={password}
                                    onChange={(e) => setPassword(e.target.value)}
                                    placeholder="••••••••"
                                    className="w-full bg-black/40 border border-white/5 rounded-xl py-3.5 px-12 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all placeholder:text-white/10"
                                />
                            </div>
                        </div>

                        <div className="space-y-2">
                            <label className="text-[10px] font-mono text-white/40 uppercase tracking-widest ml-1">{t('confirm_password_label')}</label>
                            <div className="relative">
                                <Lock className="absolute left-4 top-1/2 -translate-y-1/2 w-4 h-4 text-white/20" />
                                <input 
                                    type="password" 
                                    required
                                    value={confirm}
                                    onChange={(e) => setConfirm(e.target.value)}
                                    placeholder="••••••••"
                                    className="w-full bg-black/40 border border-white/5 rounded-xl py-3.5 px-12 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all placeholder:text-white/10"
                                />
                            </div>
                        </div>

                        {error && (
                            <motion.div 
                                initial={{ opacity: 0, x: -10 }}
                                animate={{ opacity: 1, x: 0 }}
                                className="bg-red-500/10 border border-red-500/30 p-4 rounded-xl flex items-start gap-3"
                            >
                                <AlertTriangle className="w-4 h-4 text-red-500 shrink-0 mt-0.5" />
                                <p className="text-[10px] font-mono text-red-400 uppercase leading-relaxed">{error}</p>
                            </motion.div>
                        )}

                        <button 
                            type="submit"
                            disabled={isSubmitting}
                            className={`w-full group relative overflow-hidden rounded-xl py-4 transition-all duration-500 ${isSubmitting ? "bg-aegis-cyan/10 cursor-wait" : "bg-aegis-cyan text-black hover:shadow-[0_0_30px_rgba(0,186,211,0.4)] active:scale-95"}`}
                        >
                            <div className="relative z-10 flex items-center justify-center gap-3">
                                {isSubmitting ? <Loader2 className="w-5 h-5 text-aegis-cyan animate-spin" /> : <ArrowRight className="w-5 h-5" />}
                                <span className="text-[11px] font-bold tracking-[0.3em] uppercase">
                                    {isSubmitting ? t('establishing_link') : t('create_master_admin')}
                                </span>
                            </div>
                        </button>
                    </div>
                </form>

                <div className="mt-8 text-center">
                    <p className="text-[9px] font-mono text-white/10 uppercase tracking-[0.4em]">Secure Auth Bridge v2.0 // One-Time Setup Protocol</p>
                </div>
            </motion.div>
        </div>
    );
};

export default BootstrapSetup;

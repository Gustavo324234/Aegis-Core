import React, { useState } from 'react';
import { Key, Terminal } from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';
import { motion } from 'framer-motion';
import { useTranslation } from '../i18n';

const UserPasswordChange: React.FC<{ onComplete?: () => void }> = ({ onComplete }) => {
    const { tenantId, sessionKey } = useAegisStore();
    const [newPassphrase, setNewPassphrase] = useState('');
    const [confirmPassphrase, setConfirmPassphrase] = useState('');
    const [isUpdating, setIsUpdating] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [success, setSuccess] = useState(false);
    const { t } = useTranslation();

    const handlePasswordChange = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!newPassphrase) return;
        if (newPassphrase !== confirmPassphrase) {
            setError(t('passwords_not_match'));
            return;
        }

        setIsUpdating(true);
        setError(null);
        setSuccess(false);

        try {
            const response = await fetch('/api/admin/reset_password', {
                method: 'POST',
                headers: { 
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId!,
                    'x-citadel-key': sessionKey!
                },
                body: JSON.stringify({
                    tenant_id: tenantId,
                    new_passphrase: newPassphrase
                })
            });

            if (response.ok) {
                const data = await response.json();
                if (data.success) {
                    setSuccess(true);
                    setNewPassphrase('');
                    setConfirmPassphrase('');
                    if (onComplete) {
                        setTimeout(onComplete, 2000);
                    }
                } else {
                    setError(data.message || t('error_updating_password'));
                }
            } else {
                setError(t('ring0_connection_failed'));
            }
        } catch (err) {
            setError(err instanceof Error ? err.message : t('unknown_error'));
        } finally {
            setIsUpdating(false);
        }
    };

    return (
        <div className="glass p-6 rounded-2xl border border-white/10 mt-6 relative overflow-hidden">
            <div className="flex items-center gap-3 mb-6">
                <Key className="w-5 h-5 text-yellow-500" />
                <h3 className="text-sm font-mono font-bold tracking-widest uppercase text-white">{t('change_password')}</h3>
            </div>
            
            {success ? (
                <div className="bg-green-500/10 border border-green-500/30 p-4 rounded-lg flex flex-col items-center justify-center gap-2 py-8">
                    <div className="w-10 h-10 bg-green-500/20 rounded-full flex items-center justify-center text-green-400 font-bold mb-2">
                        ✓
                    </div>
                    <span className="text-xs font-mono text-green-400 uppercase tracking-widest text-center">{t('password_changed_success')}</span>
                </div>
            ) : (
                <form onSubmit={handlePasswordChange} className="space-y-4">
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                        <div>
                            <label className="block text-[10px] font-mono text-white/40 uppercase tracking-widest mb-1">{t('new_password')}</label>
                            <input
                                type="password"
                                value={newPassphrase}
                                onChange={(e) => setNewPassphrase(e.target.value)}
                                className="w-full bg-black/40 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono text-white focus:border-yellow-500/50 outline-none placeholder:text-white/10"
                                required
                            />
                        </div>
                        <div>
                            <label className="block text-[10px] font-mono text-white/40 uppercase tracking-widest mb-1">{t('confirm_password')}</label>
                            <input
                                type="password"
                                value={confirmPassphrase}
                                onChange={(e) => setConfirmPassphrase(e.target.value)}
                                className="w-full bg-black/40 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono text-white focus:border-yellow-500/50 outline-none placeholder:text-white/10"
                                required
                            />
                        </div>
                    </div>
                    
                    {error && (
                        <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="bg-red-500/10 border border-red-500/30 p-3 rounded-lg flex items-center gap-3">
                            <Terminal className="w-4 h-4 text-red-500" />
                            <span className="text-[10px] font-mono text-red-400 leading-tight">{error}</span>
                        </motion.div>
                    )}

                    <button
                        type="submit"
                        disabled={isUpdating}
                        className={`w-full group relative overflow-hidden rounded-lg py-3 transition-colors ${isUpdating ? "bg-yellow-500/10 cursor-wait" : "bg-yellow-500/10 hover:bg-yellow-500/20 border border-yellow-500/30"}`}
                    >
                        <div className="relative z-10 flex items-center justify-center gap-2">
                            <span className="text-xs font-mono font-bold tracking-widest uppercase text-yellow-500">
                                {isUpdating ? t('applying') : t('apply_change')}
                            </span>
                        </div>
                    </button>
                </form>
            )}
        </div>
    );
};

export default UserPasswordChange;

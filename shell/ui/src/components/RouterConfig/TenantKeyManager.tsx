import React, { useState, useEffect, useCallback } from 'react';
import { Plus, Trash2, RefreshCw, Key, Info } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { useTranslation } from '../../i18n';

interface KeyInfo {
    key_id: string;
    provider: string;
    api_key: string;
    api_url?: string;
    label?: string;
    is_active: boolean;
    rate_limited_until?: string;
}

const PROVIDERS = ['anthropic', 'openai', 'groq', 'deepseek', 'mistral', 'google', 'openrouter', 'qwen', 'ollama'];

const TenantKeyManager: React.FC<{ tenantId: string; sessionKey: string }> = ({ tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const [keys, setKeys] = useState<KeyInfo[]>([]);
    const [isLoading, setIsLoading] = useState(false);
    const [showModal, setShowModal] = useState(false);
    const [provider, setProvider] = useState('anthropic');
    const [apiKey, setApiKey] = useState('');
    const [label, setLabel] = useState('');
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const fetchKeys = useCallback(async () => {
        setIsLoading(true);
        try {
            const res = await fetch(
                `/api/router/keys/tenant?tenant_id=${encodeURIComponent(tenantId)}`,
                { headers: { 'x-citadel-key': sessionKey } }
            );
            if (res.ok) {
                const data = await res.json();
                setKeys(data.keys || []);
            }
        } catch (err) {
            console.error('Failed to fetch tenant keys:', err);
        } finally {
            setIsLoading(false);
        }
    }, [tenantId, sessionKey]);

    useEffect(() => { fetchKeys(); }, [fetchKeys]);

    const handleAdd = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!apiKey.trim()) {
            setError(t('api_key_required_error'));
            return;
        }
        setIsSubmitting(true);
        setError(null);
        try {
            const res = await fetch('/api/router/keys/tenant', {
                method: 'POST',
                headers: { 
                    'Content-Type': 'application/json',
                    'x-citadel-key': sessionKey
                },
                body: JSON.stringify({
                    tenant_id: tenantId,
                    provider,
                    api_key: apiKey,
                    label: label || null,
                }),
            });
            if (res.ok) {
                setShowModal(false);
                setApiKey('');
                setLabel('');
                await fetchKeys();
            } else {
                const d = await res.json();
                setError(d.detail || t('error_updating_password'));
            }
        } catch (err) {
            setError(err instanceof Error ? err.message : t('unknown_error'));
        } finally {
            setIsSubmitting(false);
        }
    };

    const handleDelete = async (keyId: string) => {
        try {
            await fetch(
                `/api/router/keys/tenant/${encodeURIComponent(keyId)}?tenant_id=${encodeURIComponent(tenantId)}`,
                { 
                    method: 'DELETE',
                    headers: { 'x-citadel-key': sessionKey }
                }
            );
            await fetchKeys();
        } catch (err) {
            console.error('Failed to delete key:', err);
        }
    };

    const getRateLimitText = (until: string | undefined): string => {
        if (!until) return '';
        const ms = new Date(until).getTime() - Date.now();
        if (ms <= 0) return '';
        const minutes = Math.ceil(ms / 60000);
        return t('rate_limited_until', { minutes: minutes.toString() });
    };

    return (
        <div className="glass p-6 rounded-2xl border border-white/10">
            <div className="flex items-start gap-3 p-3 mb-6 bg-white/5 border border-white/10 rounded-lg">
                <Info className="w-4 h-4 text-aegis-cyan mt-0.5 shrink-0" />
                <p className="text-[10px] font-mono text-white/40 leading-relaxed">
                    {t('global_keys_usage_info')}
                </p>
            </div>

            <div className="flex items-center justify-between mb-6">
                <div className="flex items-center gap-3">
                    <Key className="w-5 h-5 text-aegis-cyan" />
                    <h3 className="text-sm font-mono font-bold tracking-widest uppercase text-white">{t('my_keys')}</h3>
                </div>
                <div className="flex gap-2">
                    <button onClick={fetchKeys} className="p-2 border border-white/10 rounded-lg hover:bg-white/5 transition-colors">
                        <RefreshCw className="w-4 h-4 text-white/40" />
                    </button>
                    <button
                        onClick={() => setShowModal(true)}
                        className="flex items-center gap-2 px-3 py-2 bg-aegis-cyan/10 border border-aegis-cyan/30 rounded-lg hover:bg-aegis-cyan/20 transition-colors text-xs font-mono text-aegis-cyan"
                    >
                        <Plus className="w-4 h-4" /> {t('add_key')}
                    </button>
                </div>
            </div>

            {isLoading ? (
                <div className="text-center py-8 text-white/30 text-xs font-mono">{t('syncing')}</div>
            ) : keys.length === 0 ? (
                <div className="text-center py-8 text-white/30 text-xs font-mono">{t('personal_keys_notice')}</div>
            ) : (
                <div className="overflow-x-auto">
                    <table className="w-full text-xs font-mono">
                        <thead>
                            <tr className="text-white/30 uppercase tracking-widest border-b border-white/5">
                                <th className="text-left py-2 pr-4">{t('model').split(' ')[0]}</th>
                                <th className="text-left py-2 pr-4">{t('provider_selection').split(' ')[0]}</th>
                                <th className="text-left py-2 pr-4">{t('status')}</th>
                                <th className="text-right py-2">{t('actions')}</th>
                            </tr>
                        </thead>
                        <tbody>
                            {keys.map((k) => {
                                const rateLimitText = getRateLimitText(k.rate_limited_until);
                                const isAvailable = k.is_active && !rateLimitText;
                                return (
                                    <tr key={k.key_id} className="border-b border-white/5">
                                        <td className="py-3 pr-4 text-white/70">{k.label || '—'}</td>
                                        <td className="py-3 pr-4 text-aegis-cyan">{k.provider}</td>
                                        <td className="py-3 pr-4">
                                            {rateLimitText ? (
                                                <span className="text-yellow-400">{rateLimitText}</span>
                                            ) : isAvailable ? (
                                                <span className="text-green-400">{t('available')}</span>
                                            ) : (
                                                <span className="text-red-400">{t('inactive')}</span>
                                            )}
                                        </td>
                                        <td className="py-3 text-right">
                                            <button
                                                onClick={() => handleDelete(k.key_id)}
                                                className="p-1.5 border border-red-500/20 rounded hover:bg-red-500/10 transition-colors"
                                            >
                                                <Trash2 className="w-3.5 h-3.5 text-red-400" />
                                            </button>
                                        </td>
                                    </tr>
                                );
                            })}
                        </tbody>
                    </table>
                </div>
            )}

            <AnimatePresence>
                {showModal && (
                    <motion.div
                        initial={{ opacity: 0, height: 0, marginTop: 0 }}
                        animate={{ opacity: 1, height: 'auto', marginTop: 24 }}
                        exit={{ opacity: 0, height: 0, marginTop: 0 }}
                        className="overflow-hidden"
                    >
                        <div className="w-full bg-white/5 border border-white/10 rounded-2xl p-6 shadow-2xl">
                            <h4 className="text-sm font-mono font-bold tracking-widest uppercase text-white mb-6">{t('add_my_api_key')}</h4>
                            <form onSubmit={handleAdd} className="space-y-4">
                                <div>
                                    <label className="block text-xs font-mono text-white/40 uppercase tracking-widest mb-1">Provider</label>
                                    <select
                                        value={provider}
                                        onChange={(e) => setProvider(e.target.value)}
                                        className="w-full bg-black/40 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono text-white focus:border-aegis-cyan/50 outline-none"
                                    >
                                        {PROVIDERS.map((p) => (
                                            <option key={p} value={p}>{p}</option>
                                        ))}
                                    </select>
                                </div>
                                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                    <div>
                                        <label className="block text-xs font-mono text-white/40 uppercase tracking-widest mb-1">API Key *</label>
                                        <input
                                            type="password"
                                            value={apiKey}
                                            onChange={(e) => setApiKey(e.target.value)}
                                            placeholder="sk-..."
                                            className="w-full bg-black/40 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono text-white focus:border-aegis-cyan/50 outline-none"
                                            required
                                        />
                                    </div>
                                    <div>
                                        <label className="block text-xs font-mono text-white/40 uppercase tracking-widest mb-1">{t('model').split(' ')[0]} ({t('optional')})</label>
                                        <input
                                            type="text"
                                            value={label}
                                            onChange={(e) => setLabel(e.target.value)}
                                            placeholder="Mi key personal"
                                            className="w-full bg-black/40 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono text-white focus:border-aegis-cyan/50 outline-none"
                                        />
                                    </div>
                                </div>
                                {error && <p className="text-red-400 text-xs font-mono">{error}</p>}
                                <div className="flex gap-3 pt-2">
                                    <button
                                        type="button"
                                        onClick={() => setShowModal(false)}
                                        className="flex-1 px-4 py-2 border border-white/10 rounded-lg text-xs font-mono text-white/40 hover:bg-white/5 transition-colors uppercase tracking-widest"
                                    >
                                        {t('cancel')}
                                    </button>
                                    <button
                                        type="submit"
                                        disabled={isSubmitting}
                                        className="flex-1 px-4 py-2 bg-aegis-cyan/20 border border-aegis-cyan/30 rounded-lg text-xs font-mono text-aegis-cyan hover:bg-aegis-cyan/30 transition-colors disabled:opacity-50 uppercase tracking-widest font-bold"
                                    >
                                        {isSubmitting ? t('saving') : t('save')}
                                    </button>
                                </div>
                            </form>
                        </div>
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
};

export default TenantKeyManager;

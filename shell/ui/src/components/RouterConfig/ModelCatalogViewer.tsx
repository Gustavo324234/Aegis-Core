import React, { useState, useEffect, useCallback } from 'react';
import { RefreshCw, Search } from 'lucide-react';
import { useTranslation } from '../../i18n';

interface ModelTaskScores {
    chat: number;
    coding: number;
    planning: number;
    analysis: number;
}

interface ModelInfo {
    model_id: string;
    provider: string;
    display_name: string;
    context_window: number;
    cost_input_per_mtok: number;
    cost_output_per_mtok: number;
    is_local: boolean;
    task_scores: ModelTaskScores;
}

function ScoreDots({ score }: { score: number }) {
    return (
        <span className="font-mono text-[10px] tracking-tight">
            {Array.from({ length: 5 }, (_, i) => (
                <span key={i} className={i < score ? 'text-aegis-cyan' : 'text-white/20'}>
                    {i < score ? '\u25CF' : '\u25CB'}
                </span>
            ))}
        </span>
    );
}

const ModelCatalogViewer: React.FC<{
    tenantId: string;
    sessionKey: string;
    isAdmin: boolean;
}> = ({ tenantId, sessionKey, isAdmin }) => {
    const { t } = useTranslation();
    const [models, setModels] = useState<ModelInfo[]>([]);
    const [syncedAt, setSyncedAt] = useState<string | null>(null);
    const [isLoading, setIsLoading] = useState(false);
    const [search, setSearch] = useState('');
    const [providerFilter, setProviderFilter] = useState('');
    const [hasKeys, setHasKeys] = useState(false);
    const [keyProviders, setKeyProviders] = useState<Set<string>>(new Set());

    const fetchModels = useCallback(async () => {
        setIsLoading(true);
        try {
            const [globalRes, tenantRes] = await Promise.all([
                fetch(`/api/router/keys/global?tenant_id=${encodeURIComponent(tenantId)}`, { headers: { 'x-citadel-key': sessionKey } }),
                fetch(`/api/router/keys/tenant?tenant_id=${encodeURIComponent(tenantId)}`, { headers: { 'x-citadel-key': sessionKey } })
            ]);
            
            const providersWithKeys = new Set<string>();
            if (globalRes.ok) {
                const globalData = await globalRes.json();
                (globalData.keys || []).forEach((k: { provider: string }) => providersWithKeys.add(k.provider));
            }
            if (tenantRes.ok) {
                const tenantData = await tenantRes.json();
                (tenantData.keys || []).forEach((k: { provider: string }) => providersWithKeys.add(k.provider));
            }
            setKeyProviders(providersWithKeys);
            setHasKeys(providersWithKeys.size > 0);

            const res = await fetch(
                `/api/router/models?tenant_id=${encodeURIComponent(tenantId)}`,
                { headers: { 'x-citadel-key': sessionKey } }
            );
            if (res.ok) {
                const data = await res.json();
                setModels(data.models || []);
                setSyncedAt(data.synced_at || null);
            }
        } catch (err) {
            console.error('Failed to fetch catalog data:', err);
        } finally {
            setIsLoading(false);
        }
    }, [tenantId, sessionKey]);

    const handleSync = async () => {
        setIsLoading(true);
        try {
            const res = await fetch(
                `/api/router/sync?tenant_id=${encodeURIComponent(tenantId)}`,
                { 
                    method: 'POST',
                    headers: { 'x-citadel-key': sessionKey }
                }
            );
            if (res.ok) {
                await fetchModels();
            }
        } catch (err) {
            console.error('Manual sync failed:', err);
        } finally {
            setIsLoading(false);
        }
    };

    useEffect(() => { fetchModels(); }, [fetchModels]);

    const providers = Array.from(new Set(models.map((m) => m.provider))).sort();

    const filtered = models.filter((m) => {
        const matchSearch = !search || m.display_name.toLowerCase().includes(search.toLowerCase()) || m.model_id.toLowerCase().includes(search.toLowerCase());
        const matchProvider = !providerFilter || m.provider === providerFilter;
        return matchSearch && matchProvider;
    });

    return (
        <div className="glass p-6 rounded-2xl border border-white/10">
            <div className="flex items-center justify-between mb-6">
                <div className="flex items-center gap-3">
                    <h3 className="text-sm font-mono font-bold tracking-widest uppercase text-white">{t('model_catalog')}</h3>
                    {syncedAt ? (
                        <span className="text-[9px] font-mono text-white/30 bg-white/5 px-2 py-1 rounded-full border border-white/10">
                            {t('synced_at', { time: new Date(syncedAt).toLocaleTimeString() })}
                        </span>
                    ) : models.length > 0 ? (
                        <span className="text-[9px] font-mono text-aegis-cyan/40 bg-aegis-cyan/5 px-2 py-1 rounded-full border border-aegis-cyan/10">
                            {t('local_models_only')}
                        </span>
                    ) : null}
                </div>
                {isAdmin && (
                    <button 
                        onClick={handleSync} 
                        disabled={isLoading}
                        className="flex items-center gap-2 px-3 py-1.5 border border-white/10 rounded-lg hover:bg-white/5 transition-colors text-[10px] font-mono text-white/60 disabled:opacity-50"
                    >
                        <RefreshCw className={`w-3 h-3 ${isLoading ? 'animate-spin' : ''}`} />
                        {t('sync_now')}
                    </button>
                )}
            </div>

            <div className="flex gap-3 mb-4">
                <div className="relative flex-1">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-white/30" />
                    <input
                        type="text"
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        placeholder={t('search_model')}
                        className="w-full bg-black/40 border border-white/10 rounded-lg pl-9 pr-3 py-2 text-xs font-mono text-white placeholder-white/20 focus:border-aegis-cyan/50 outline-none"
                    />
                </div>
                {providers.length > 0 && (
                    <select
                        value={providerFilter}
                        onChange={(e) => setProviderFilter(e.target.value)}
                        className="bg-black/40 border border-white/10 rounded-lg px-3 py-2 text-xs font-mono text-white focus:border-aegis-cyan/50 outline-none"
                    >
                        <option value="">{t('all_providers')}</option>
                        {providers.map((p) => (
                            <option key={p} value={p}>{p}</option>
                        ))}
                    </select>
                )}
            </div>

            {isLoading && models.length === 0 ? (
                <div className="text-center py-12">
                     <RefreshCw className="w-6 h-6 text-aegis-cyan animate-spin mx-auto mb-3 opacity-50" />
                     <div className="text-white/30 text-xs font-mono">{t('consulting_kernel')}</div>
                </div>
            ) : filtered.length === 0 ? (
                <div className="text-center py-12 flex flex-col items-center">
                    <div className="text-white/30 text-xs font-mono mb-6">
                        {models.length === 0
                            ? t('catalog_pending_sync')
                            : t('no_models_match')}
                    </div>
                    {hasKeys && models.length === 0 && (
                        <button 
                            onClick={handleSync}
                            className="px-6 py-2 bg-aegis-cyan/10 border border-aegis-cyan/30 rounded-full text-aegis-cyan text-[10px] font-mono uppercase tracking-widest hover:bg-aegis-cyan/20 transition-all shadow-[0_0_15px_rgba(0,192,255,0.1)]"
                        >
                            {t('sync_now')}
                        </button>
                    )}
                </div>
            ) : (
                <div className="overflow-x-auto overflow-y-auto max-h-96">
                    <table className="w-full text-xs font-mono">
                        <thead>
                            <tr className="text-white/30 uppercase tracking-widest border-b border-white/5">
                                <th className="text-left py-2 pr-4">{t('model')}</th>
                                <th className="text-left py-2 pr-4">Provider</th>
                                <th className="text-center py-2 pr-4">Chat</th>
                                <th className="text-center py-2 pr-4">Code</th>
                                <th className="text-center py-2 pr-4">Plan</th>
                                <th className="text-center py-2 pr-4">{t('analysis_desc')}</th>
                                <th className="text-right py-2 pr-4">{t('technical_info').split(' ')[1]}</th>
                                <th className="text-right py-2 pr-4">{t('technical_info').split(' ')[0]}</th>
                                <th className="text-center py-2">{t('status')}</th>
                                <th className="text-center py-2 pl-2">Key</th>
                            </tr>
                        </thead>
                        <tbody>
                            {filtered.map((m) => (
                                <tr key={m.model_id} className="border-b border-white/5 hover:bg-white/2">
                                    <td className="py-3 pr-4 text-white/80">{m.display_name}</td>
                                    <td className="py-3 pr-4 text-white/40">{m.provider}</td>
                                    <td className="py-3 pr-4 text-center"><ScoreDots score={m.task_scores.chat} /></td>
                                    <td className="py-3 pr-4 text-center"><ScoreDots score={m.task_scores.coding} /></td>
                                    <td className="py-3 pr-4 text-center"><ScoreDots score={m.task_scores.planning} /></td>
                                    <td className="py-3 pr-4 text-center"><ScoreDots score={m.task_scores.analysis} /></td>
                                    <td className="py-3 pr-4 text-right text-white/40">
                                        {m.is_local ? 'Free' : `$${(m.cost_input_per_mtok + m.cost_output_per_mtok).toFixed(2)}`}
                                    </td>
                                    <td className="py-3 pr-4 text-right text-white/40">
                                        {m.context_window >= 1000000 ? `${(m.context_window / 1000000).toFixed(1)}M` : `${Math.round(m.context_window / 1000)}K`}
                                    </td>
                                    <td className="py-3 text-center">
                                        <span className={`px-2 py-0.5 rounded-full text-[9px] border ${
                                            m.is_local
                                                ? 'bg-green-500/20 text-green-400 border-green-500/30'
                                                : 'bg-white/5 text-white/30 border-white/10'
                                        }`}>
                                            {m.is_local ? 'Local' : 'Cloud'}
                                        </span>
                                    </td>
                                    <td className="py-3 pl-2 text-center">
                                        {keyProviders.has(m.provider)
                                            ? <span className="text-[9px] font-mono text-green-400">✓</span>
                                            : <span className="text-[9px] font-mono text-white/20">—</span>
                                        }
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            )}
        </div>
    );
};

export default ModelCatalogViewer;

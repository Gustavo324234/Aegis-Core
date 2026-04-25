import React, { useEffect } from 'react';
import { GitPullRequest, RefreshCw, Merge, Zap, ZapOff } from 'lucide-react';
import { usePrStore, ManagedPr, PrStatus } from '../../store/prStore';
import { useAegisStore } from '../../store/useAegisStore';

const statusColors: Record<PrStatus, string> = {
    open: 'text-white/50 bg-white/5',
    ci_running: 'text-yellow-400 bg-yellow-500/10',
    ci_passed: 'text-green-400 bg-green-500/10',
    ci_failed: 'text-red-400 bg-red-500/10',
    auto_fix_in_progress: 'text-aegis-purple bg-aegis-purple/10',
    merge_ready: 'text-aegis-cyan bg-aegis-cyan/10',
    merged: 'text-white/30 bg-white/5',
    closed: 'text-white/20 bg-white/5',
};

const PrRow: React.FC<{ pr: ManagedPr; tenantId: string; sessionKey: string }> = ({
    pr, tenantId, sessionKey,
}) => {
    const { patchPr, mergeNow } = usePrStore();

    const toggleMergeMode = () =>
        patchPr(tenantId, sessionKey, pr.pr_number, {
            merge_mode: pr.merge_mode === 'automatic' ? 'manual' : 'automatic',
        });

    const toggleAutoFix = () =>
        patchPr(tenantId, sessionKey, pr.pr_number, { auto_fix_ci: !pr.auto_fix_ci });

    return (
        <div className="flex flex-col gap-2 p-3 rounded-xl bg-white/[0.03] border border-white/[0.05] hover:border-white/10 transition-colors">
            <div className="flex items-start justify-between gap-2">
                <div className="min-w-0">
                    <a
                        href={pr.url}
                        target="_blank"
                        rel="noreferrer"
                        className="text-[11px] font-medium text-white/80 hover:text-aegis-cyan transition-colors truncate block"
                    >
                        #{pr.pr_number} {pr.title}
                    </a>
                    <p className="text-[9px] font-mono text-white/30 mt-0.5">
                        {pr.branch} → {pr.base_branch}
                    </p>
                </div>
                <span className={`shrink-0 text-[9px] font-mono px-2 py-0.5 rounded-full uppercase tracking-tighter ${statusColors[pr.status]}`}>
                    {pr.status.replace(/_/g, ' ')}
                </span>
            </div>

            <div className="flex items-center gap-2 flex-wrap">
                <button
                    onClick={toggleMergeMode}
                    className={`flex items-center gap-1 text-[9px] font-mono px-2 py-1 rounded-lg transition-colors ${
                        pr.merge_mode === 'automatic'
                            ? 'bg-aegis-cyan/10 text-aegis-cyan'
                            : 'bg-white/5 text-white/30 hover:bg-white/10'
                    }`}
                    title="Toggle merge mode"
                >
                    <Merge className="w-3 h-3" />
                    {pr.merge_mode}
                </button>

                <button
                    onClick={toggleAutoFix}
                    className={`flex items-center gap-1 text-[9px] font-mono px-2 py-1 rounded-lg transition-colors ${
                        pr.auto_fix_ci
                            ? 'bg-aegis-purple/10 text-aegis-purple'
                            : 'bg-white/5 text-white/30 hover:bg-white/10'
                    }`}
                    title="Toggle auto-fix CI"
                >
                    {pr.auto_fix_ci ? <Zap className="w-3 h-3" /> : <ZapOff className="w-3 h-3" />}
                    auto-fix
                    {pr.auto_fix_attempts > 0 && (
                        <span className="ml-0.5 opacity-60">({pr.auto_fix_attempts}/3)</span>
                    )}
                </button>

                {pr.status === 'merge_ready' && pr.merge_mode === 'manual' && (
                    <button
                        onClick={() => mergeNow(tenantId, sessionKey, pr.pr_number)}
                        className="flex items-center gap-1 text-[9px] font-mono px-2 py-1 rounded-lg bg-green-500/10 text-green-400 hover:bg-green-500/20 transition-colors ml-auto"
                    >
                        <Merge className="w-3 h-3" />
                        Merge Now
                    </button>
                )}
            </div>
        </div>
    );
};

const PRManagerPanel: React.FC = () => {
    const { tenantId, sessionKey } = useAegisStore();
    const { prs, isLoading, fetchPrs } = usePrStore();

    useEffect(() => {
        if (tenantId && sessionKey) fetchPrs(tenantId, sessionKey);
    }, [tenantId, sessionKey, fetchPrs]);

    const activePrs = prs.filter((p) => !['merged', 'closed'].includes(p.status));

    return (
        <div className="glass rounded-2xl border border-white/10 flex flex-col overflow-hidden" style={{ minHeight: '280px' }}>
            <div className="flex items-center justify-between px-4 py-3 border-b border-white/5">
                <div className="flex items-center gap-2">
                    <GitPullRequest className="w-4 h-4 text-aegis-cyan" />
                    <span className="text-[10px] font-mono uppercase tracking-widest text-white/60">
                        PR Manager
                    </span>
                    {activePrs.length > 0 && (
                        <span className="text-[10px] font-mono bg-aegis-cyan/10 text-aegis-cyan px-2 py-0.5 rounded-full">
                            {activePrs.length} active
                        </span>
                    )}
                </div>
                <button
                    onClick={() => tenantId && sessionKey && fetchPrs(tenantId, sessionKey)}
                    disabled={isLoading}
                    className="text-white/20 hover:text-white/60 transition-colors disabled:opacity-30"
                >
                    <RefreshCw className={`w-3.5 h-3.5 ${isLoading ? 'animate-spin' : ''}`} />
                </button>
            </div>

            <div className="flex-1 overflow-y-auto p-3 space-y-2 scrollbar-hide">
                {prs.length === 0 && !isLoading && (
                    <p className="text-[10px] font-mono text-white/20 text-center py-4">
                        No managed PRs. Open a PR from Chat.
                    </p>
                )}
                {prs.map((pr) => (
                    <PrRow
                        key={pr.pr_number}
                        pr={pr}
                        tenantId={tenantId ?? ''}
                        sessionKey={sessionKey ?? ''}
                    />
                ))}
            </div>
        </div>
    );
};

export default PRManagerPanel;

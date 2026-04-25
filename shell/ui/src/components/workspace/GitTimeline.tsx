import React, { useState, useCallback } from 'react';
import { GitBranch, GitCommit, RefreshCw } from 'lucide-react';
import { useAegisStore } from '../../store/useAegisStore';

interface CommitInfo {
    sha: string;
    message: string;
    author_name: string;
    date: string;
}

interface BranchInfo {
    name: string;
    short_sha: string;
    is_remote: boolean;
}

const GitTimeline: React.FC = () => {
    const { tenantId, sessionKey } = useAegisStore();
    const [branches, setBranches] = useState<BranchInfo[]>([]);
    const [commits, setCommits] = useState<CommitInfo[]>([]);
    const [currentBranch, setCurrentBranch] = useState('');
    const [loading, setLoading] = useState(false);
    const [loaded, setLoaded] = useState(false);

    const load = useCallback(async () => {
        if (!tenantId || !sessionKey) return;
        setLoading(true);
        try {
            const [branchRes, commitRes] = await Promise.all([
                fetch('/api/git/branches', {
                    headers: { 'x-citadel-tenant': tenantId, 'x-citadel-key': sessionKey },
                }),
                fetch('/api/git/commits', {
                    headers: { 'x-citadel-tenant': tenantId, 'x-citadel-key': sessionKey },
                }),
            ]);
            if (branchRes.ok) {
                const d = await branchRes.json() as { branches: BranchInfo[] };
                setBranches(d.branches ?? []);
            }
            if (commitRes.ok) {
                const d = await commitRes.json() as { branch: string; commits: CommitInfo[] };
                setCurrentBranch(d.branch ?? '');
                setCommits(d.commits ?? []);
            }
            setLoaded(true);
        } finally {
            setLoading(false);
        }
    }, [tenantId, sessionKey]);

    return (
        <div className="glass rounded-2xl border border-white/10 flex flex-col overflow-hidden" style={{ height: '380px' }}>
            <div className="flex items-center justify-between px-4 py-3 border-b border-white/5">
                <div className="flex items-center gap-2">
                    <GitBranch className="w-4 h-4 text-aegis-cyan" />
                    <span className="text-[10px] font-mono uppercase tracking-widest text-white/60">
                        Git Timeline
                    </span>
                    {currentBranch && (
                        <span className="text-[10px] font-mono bg-aegis-cyan/10 text-aegis-cyan px-2 py-0.5 rounded-full">
                            {currentBranch}
                        </span>
                    )}
                </div>
                <button
                    onClick={load}
                    disabled={loading}
                    className="text-white/20 hover:text-white/60 transition-colors disabled:opacity-30"
                >
                    <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
                </button>
            </div>

            {!loaded ? (
                <div className="flex-1 flex items-center justify-center">
                    <button
                        onClick={load}
                        className="text-[10px] font-mono text-aegis-cyan/60 hover:text-aegis-cyan transition-colors"
                    >
                        Load Git data
                    </button>
                </div>
            ) : (
                <div className="flex flex-1 overflow-hidden">
                    {/* Branches */}
                    <div className="w-44 border-r border-white/5 overflow-y-auto py-2 px-2 space-y-1 scrollbar-hide shrink-0">
                        <p className="text-[9px] font-mono uppercase text-white/20 tracking-widest px-1 mb-2">Branches</p>
                        {branches.filter((b) => !b.is_remote).map((b) => (
                            <div key={b.name} className="flex items-center gap-1.5 py-1 px-2 rounded hover:bg-white/5 transition-colors">
                                <GitBranch className="w-3 h-3 text-white/20 shrink-0" />
                                <span className={`text-[10px] font-mono truncate ${b.name === currentBranch ? 'text-aegis-cyan' : 'text-white/50'}`}>
                                    {b.name}
                                </span>
                            </div>
                        ))}
                    </div>

                    {/* Commits */}
                    <div className="flex-1 overflow-y-auto py-2 px-3 space-y-2 scrollbar-hide">
                        <p className="text-[9px] font-mono uppercase text-white/20 tracking-widest mb-2">Commits</p>
                        {commits.map((c) => (
                            <div key={c.sha} className="flex gap-2 items-start py-1.5 border-b border-white/[0.03]">
                                <GitCommit className="w-3.5 h-3.5 text-white/20 mt-0.5 shrink-0" />
                                <div className="min-w-0">
                                    <p className="text-[11px] text-white/70 leading-tight truncate">{c.message}</p>
                                    <p className="text-[9px] font-mono text-white/25 mt-0.5">
                                        {c.sha.slice(0, 7)} · {c.author_name} · {new Date(c.date).toLocaleDateString()}
                                    </p>
                                </div>
                            </div>
                        ))}
                        {commits.length === 0 && (
                            <p className="text-[10px] font-mono text-white/20">No commits found.</p>
                        )}
                    </div>
                </div>
            )}
        </div>
    );
};

export default GitTimeline;

import React, { useState, useEffect, useCallback } from 'react';
import { Settings, Save, Eye, EyeOff, RefreshCw } from 'lucide-react';
import { useAegisStore } from '../../store/useAegisStore';

interface WorkspaceDto {
    github_token_status: string | null;
    project_root: string | null;
    github_repo: string | null;
    terminal_allowlist: string[] | null;
    pr_merge_mode: string | null;
    pr_auto_fix_ci: boolean | null;
}

const Field: React.FC<{
    label: string;
    value: string;
    onChange: (v: string) => void;
    placeholder?: string;
    type?: string;
    hint?: string;
}> = ({ label, value, onChange, placeholder, type = 'text', hint }) => (
    <div className="flex flex-col gap-1">
        <label className="text-[9px] font-mono uppercase tracking-widest text-white/30">{label}</label>
        {hint && <p className="text-[9px] text-white/20">{hint}</p>}
        <input
            type={type}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            className="bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-[11px] font-mono text-white placeholder-white/20 focus:outline-none focus:border-aegis-cyan/50 transition-colors"
        />
    </div>
);

const WorkspaceSettings: React.FC = () => {
    const { tenantId, sessionKey } = useAegisStore();
    const [config, setConfig] = useState<WorkspaceDto | null>(null);
    const [loading, setLoading] = useState(false);
    const [saving, setSaving] = useState(false);
    const [saved, setSaved] = useState(false);
    const [showToken, setShowToken] = useState(false);

    const [token, setToken] = useState('');
    const [repo, setRepo] = useState('');
    const [root, setRoot] = useState('');
    const [allowlist, setAllowlist] = useState('');
    const [mergeMode, setMergeMode] = useState('manual');
    const [autoFix, setAutoFix] = useState(true);

    const authHeaders = useCallback(() => ({
        'Content-Type': 'application/json',
        'x-citadel-tenant': tenantId ?? '',
        'x-citadel-key': sessionKey ?? '',
    }), [tenantId, sessionKey]);

    const load = useCallback(async () => {
        if (!tenantId || !sessionKey) return;
        setLoading(true);
        try {
            const res = await fetch('/api/workspace/config', { headers: authHeaders() });
            if (res.ok) {
                const data = await res.json() as WorkspaceDto;
                setConfig(data);
                setRepo(data.github_repo ?? '');
                setRoot(data.project_root ?? '');
                setAllowlist((data.terminal_allowlist ?? ['cargo', 'npm', 'git', 'python']).join(', '));
                setMergeMode(data.pr_merge_mode ?? 'manual');
                setAutoFix(data.pr_auto_fix_ci ?? true);
            }
        } finally {
            setLoading(false);
        }
    }, [tenantId, sessionKey, authHeaders]);

    useEffect(() => {
        load();
    }, [load]);

    const setKey = async (key: string, value: string) => {
        await fetch('/api/workspace/config', {
            method: 'POST',
            headers: authHeaders(),
            body: JSON.stringify({ key, value }),
        });
    };

    const save = async () => {
        if (!tenantId || !sessionKey) return;
        setSaving(true);
        try {
            const ops: Promise<void>[] = [];
            if (token) ops.push(setKey('github_token', token));
            if (repo) ops.push(setKey('github_repo', repo));
            if (root) ops.push(setKey('project_root', root));
            ops.push(setKey('terminal_allowlist', JSON.stringify(
                allowlist.split(',').map((s) => s.trim()).filter(Boolean),
            )));
            ops.push(setKey('pr_merge_mode', mergeMode));
            ops.push(setKey('pr_auto_fix_ci', String(autoFix)));
            await Promise.all(ops);
            setSaved(true);
            setToken('');
            await load();
            setTimeout(() => setSaved(false), 2000);
        } finally {
            setSaving(false);
        }
    };

    return (
        <div className="glass rounded-2xl border border-white/10 flex flex-col overflow-hidden">
            <div className="flex items-center justify-between px-4 py-3 border-b border-white/5">
                <div className="flex items-center gap-2">
                    <Settings className="w-4 h-4 text-aegis-cyan" />
                    <span className="text-[10px] font-mono uppercase tracking-widest text-white/60">
                        Workspace Config
                    </span>
                </div>
                <button
                    onClick={load}
                    disabled={loading}
                    className="text-white/20 hover:text-white/60 transition-colors disabled:opacity-30"
                >
                    <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
                </button>
            </div>

            <div className="p-4 grid grid-cols-1 md:grid-cols-2 gap-4">
                {/* GitHub Token */}
                <div className="flex flex-col gap-1">
                    <label className="text-[9px] font-mono uppercase tracking-widest text-white/30">
                        GitHub Token
                    </label>
                    {config?.github_token_status && (
                        <p className="text-[9px] text-green-400 font-mono">● configured</p>
                    )}
                    <div className="relative">
                        <input
                            type={showToken ? 'text' : 'password'}
                            value={token}
                            onChange={(e) => setToken(e.target.value)}
                            placeholder={config?.github_token_status ? 'Leave blank to keep current' : 'ghp_...'}
                            className="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 pr-8 text-[11px] font-mono text-white placeholder-white/20 focus:outline-none focus:border-aegis-cyan/50"
                        />
                        <button
                            type="button"
                            onClick={() => setShowToken((v) => !v)}
                            className="absolute right-2 top-1/2 -translate-y-1/2 text-white/20 hover:text-white/50"
                        >
                            {showToken ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                        </button>
                    </div>
                </div>

                <Field
                    label="GitHub Repo (owner/name)"
                    value={repo}
                    onChange={setRepo}
                    placeholder="owner/repo-name"
                />

                <Field
                    label="Project Root (absolute path)"
                    value={root}
                    onChange={setRoot}
                    placeholder="/home/user/my-project"
                />

                <Field
                    label="Terminal Allowlist (comma-separated)"
                    value={allowlist}
                    onChange={setAllowlist}
                    placeholder="cargo, npm, git, python"
                />

                {/* Merge Mode */}
                <div className="flex flex-col gap-1">
                    <label className="text-[9px] font-mono uppercase tracking-widest text-white/30">PR Merge Mode</label>
                    <div className="flex gap-2">
                        {(['manual', 'automatic'] as const).map((mode) => (
                            <button
                                key={mode}
                                onClick={() => setMergeMode(mode)}
                                className={`flex-1 py-2 rounded-lg text-[10px] font-mono transition-colors ${
                                    mergeMode === mode
                                        ? 'bg-aegis-cyan/10 text-aegis-cyan border border-aegis-cyan/30'
                                        : 'bg-white/5 text-white/30 border border-white/10 hover:bg-white/10'
                                }`}
                            >
                                {mode}
                            </button>
                        ))}
                    </div>
                </div>

                {/* Auto Fix CI */}
                <div className="flex flex-col gap-1">
                    <label className="text-[9px] font-mono uppercase tracking-widest text-white/30">Auto-Fix CI</label>
                    <button
                        onClick={() => setAutoFix((v) => !v)}
                        className={`py-2 rounded-lg text-[10px] font-mono transition-colors ${
                            autoFix
                                ? 'bg-aegis-purple/10 text-aegis-purple border border-aegis-purple/30'
                                : 'bg-white/5 text-white/30 border border-white/10 hover:bg-white/10'
                        }`}
                    >
                        {autoFix ? 'Enabled' : 'Disabled'}
                    </button>
                </div>
            </div>

            <div className="px-4 pb-4">
                <button
                    onClick={save}
                    disabled={saving}
                    className={`w-full flex items-center justify-center gap-2 py-2.5 rounded-xl text-[10px] font-bold uppercase tracking-[0.2em] transition-all ${
                        saved
                            ? 'bg-green-500/10 text-green-400 border border-green-500/20'
                            : 'bg-aegis-cyan/10 text-aegis-cyan hover:bg-aegis-cyan/20 border border-aegis-cyan/20'
                    } disabled:opacity-50`}
                >
                    <Save className="w-3.5 h-3.5" />
                    {saved ? 'Saved!' : saving ? 'Saving…' : 'Save Config'}
                </button>
            </div>
        </div>
    );
};

export default WorkspaceSettings;

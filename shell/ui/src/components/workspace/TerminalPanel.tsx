import React, { useEffect, useRef, useState } from 'react';
import { Terminal, Trash2, Play } from 'lucide-react';
import { useTerminalStore, LineKind } from '../../store/terminalStore';
import { useAegisStore } from '../../store/useAegisStore';

const lineColor: Record<LineKind, string> = {
    Stdout: 'text-white/80',
    Stderr: 'text-red-400',
    System: 'text-aegis-cyan/70',
};

const TerminalPanel: React.FC = () => {
    const { lines, isRunning, clear } = useTerminalStore();
    const { tenantId, sessionKey } = useAegisStore();
    const [cmd, setCmd] = useState('');
    const [args, setArgs] = useState('');
    const bottomRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [lines]);

    const runCommand = async () => {
        if (!tenantId || !sessionKey || !cmd.trim()) return;
        useTerminalStore.getState().setRunning(true);
        useTerminalStore.getState().addSystemLine(`$ ${cmd} ${args}`);
        try {
            const argsArr = args.trim() ? args.trim().split(/\s+/) : [];
            const res = await fetch('/api/workspace/terminal', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({ command: cmd, args: argsArr }),
            });
            if (!res.ok) {
                useTerminalStore.getState().addSystemLine('Error: command rejected by server');
            }
        } catch (e) {
            useTerminalStore.getState().addSystemLine(`Error: ${String(e)}`);
        } finally {
            useTerminalStore.getState().setRunning(false);
        }
    };

    return (
        <div className="glass rounded-2xl border border-white/10 flex flex-col overflow-hidden" style={{ height: '360px' }}>
            <div className="flex items-center justify-between px-4 py-3 border-b border-white/5">
                <div className="flex items-center gap-2">
                    <Terminal className="w-4 h-4 text-aegis-cyan" />
                    <span className="text-[10px] font-mono uppercase tracking-widest text-white/60">
                        Dev Terminal
                    </span>
                    {isRunning && (
                        <span className="w-2 h-2 rounded-full bg-aegis-cyan animate-pulse" />
                    )}
                </div>
                <button
                    onClick={clear}
                    className="text-white/20 hover:text-white/60 transition-colors"
                    title="Clear"
                >
                    <Trash2 className="w-3.5 h-3.5" />
                </button>
            </div>

            <div className="flex-1 overflow-y-auto p-3 font-mono text-[11px] space-y-0.5 scrollbar-hide bg-black/30">
                {lines.length === 0 && (
                    <p className="text-white/20">No output yet. Run a command below.</p>
                )}
                {lines.map((line, i) => (
                    <div key={i} className={`${lineColor[line.kind]} leading-relaxed whitespace-pre-wrap break-all`}>
                        {line.content}
                    </div>
                ))}
                <div ref={bottomRef} />
            </div>

            <div className="flex gap-2 p-3 border-t border-white/5">
                <input
                    type="text"
                    value={cmd}
                    onChange={(e) => setCmd(e.target.value)}
                    placeholder="command"
                    className="w-28 bg-white/5 border border-white/10 rounded-lg px-3 py-1.5 text-[11px] font-mono text-white placeholder-white/20 focus:outline-none focus:border-aegis-cyan/50"
                />
                <input
                    type="text"
                    value={args}
                    onChange={(e) => setArgs(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && !isRunning && runCommand()}
                    placeholder="args…"
                    className="flex-1 bg-white/5 border border-white/10 rounded-lg px-3 py-1.5 text-[11px] font-mono text-white placeholder-white/20 focus:outline-none focus:border-aegis-cyan/50"
                />
                <button
                    onClick={runCommand}
                    disabled={isRunning || !cmd.trim()}
                    className="px-3 py-1.5 rounded-lg bg-aegis-cyan/10 hover:bg-aegis-cyan/20 text-aegis-cyan disabled:opacity-30 transition-colors"
                >
                    <Play className="w-3.5 h-3.5" />
                </button>
            </div>
        </div>
    );
};

export default TerminalPanel;

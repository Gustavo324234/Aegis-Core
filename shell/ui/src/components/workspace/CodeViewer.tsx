import React, { useState, useCallback } from 'react';
import { FolderOpen, File, ChevronRight, ChevronDown, Code } from 'lucide-react';
import { useAegisStore } from '../../store/useAegisStore';

interface FsEntry {
    name: string;
    path: string;
    kind: 'File' | 'Dir';
    children?: FsEntry[];
    size?: number;
}

const FileNode: React.FC<{
    entry: FsEntry;
    depth: number;
    selected: string | null;
    onSelect: (path: string) => void;
}> = ({ entry, depth, selected, onSelect }) => {
    const [expanded, setExpanded] = useState(depth < 1);

    if (entry.kind === 'Dir') {
        return (
            <div>
                <button
                    onClick={() => setExpanded((v) => !v)}
                    className="flex items-center gap-1.5 w-full text-left py-0.5 px-2 rounded hover:bg-white/5 transition-colors text-white/60 hover:text-white/90"
                    style={{ paddingLeft: `${8 + depth * 12}px` }}
                >
                    {expanded ? (
                        <ChevronDown className="w-3 h-3 shrink-0 text-white/30" />
                    ) : (
                        <ChevronRight className="w-3 h-3 shrink-0 text-white/30" />
                    )}
                    <FolderOpen className="w-3.5 h-3.5 shrink-0 text-yellow-500/70" />
                    <span className="text-[11px] font-mono truncate">{entry.name}</span>
                </button>
                {expanded && entry.children?.map((child) => (
                    <FileNode
                        key={child.path}
                        entry={child}
                        depth={depth + 1}
                        selected={selected}
                        onSelect={onSelect}
                    />
                ))}
            </div>
        );
    }

    return (
        <button
            onClick={() => onSelect(entry.path)}
            className={`flex items-center gap-1.5 w-full text-left py-0.5 px-2 rounded transition-colors ${
                selected === entry.path
                    ? 'bg-aegis-cyan/10 text-aegis-cyan'
                    : 'text-white/50 hover:text-white/80 hover:bg-white/5'
            }`}
            style={{ paddingLeft: `${8 + depth * 12}px` }}
        >
            <File className="w-3 h-3 shrink-0 opacity-50" />
            <span className="text-[11px] font-mono truncate">{entry.name}</span>
        </button>
    );
};

const CodeViewer: React.FC = () => {
    const { tenantId, sessionKey } = useAegisStore();
    const [tree, setTree] = useState<FsEntry[]>([]);
    const [selectedPath, setSelectedPath] = useState<string | null>(null);
    const [content, setContent] = useState<string>('');
    const [loading, setLoading] = useState(false);
    const [treeLoaded, setTreeLoaded] = useState(false);

    const loadTree = useCallback(async () => {
        if (!tenantId || !sessionKey || treeLoaded) return;
        setLoading(true);
        try {
            const res = await fetch('/api/fs/tree', {
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
            });
            if (res.ok) {
                const data = await res.json() as { tree: FsEntry[] };
                setTree(data.tree ?? []);
                setTreeLoaded(true);
            }
        } finally {
            setLoading(false);
        }
    }, [tenantId, sessionKey, treeLoaded]);

    const loadFile = async (path: string) => {
        if (!tenantId || !sessionKey) return;
        setSelectedPath(path);
        setLoading(true);
        try {
            const res = await fetch(`/api/fs/file?path=${encodeURIComponent(path)}`, {
                headers: {
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
            });
            if (res.ok) {
                const data = await res.json() as { content: string };
                setContent(data.content ?? '');
            } else if (res.status === 413) {
                setContent('// File too large to display (>500KB)');
            } else {
                setContent('// Failed to load file');
            }
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="glass rounded-2xl border border-white/10 flex flex-col overflow-hidden" style={{ height: '420px' }}>
            <div className="flex items-center gap-2 px-4 py-3 border-b border-white/5">
                <Code className="w-4 h-4 text-aegis-cyan" />
                <span className="text-[10px] font-mono uppercase tracking-widest text-white/60">Code Viewer</span>
                {loading && <span className="w-2 h-2 rounded-full bg-aegis-cyan animate-pulse ml-1" />}
                {!treeLoaded && (
                    <button
                        onClick={loadTree}
                        className="ml-auto text-[10px] font-mono text-aegis-cyan/60 hover:text-aegis-cyan transition-colors"
                    >
                        Load tree
                    </button>
                )}
            </div>

            <div className="flex flex-1 overflow-hidden">
                <div className="w-44 border-r border-white/5 overflow-y-auto py-1 scrollbar-hide shrink-0">
                    {tree.length === 0 && (
                        <p className="text-[10px] font-mono text-white/20 px-3 pt-2">
                            {treeLoaded ? 'Empty' : 'Click "Load tree"'}
                        </p>
                    )}
                    {tree.map((entry) => (
                        <FileNode
                            key={entry.path}
                            entry={entry}
                            depth={0}
                            selected={selectedPath}
                            onSelect={loadFile}
                        />
                    ))}
                </div>

                <pre className="flex-1 overflow-auto p-3 text-[11px] font-mono text-white/70 bg-black/20 scrollbar-hide leading-relaxed whitespace-pre">
                    {selectedPath ? content : (
                        <span className="text-white/20">Select a file to view its contents.</span>
                    )}
                </pre>
            </div>
        </div>
    );
};

export default CodeViewer;

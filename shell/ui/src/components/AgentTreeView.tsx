import React, { useState } from 'react';
import { ChevronRight, ChevronDown, CheckCircle2, Circle, Loader2, AlertCircle, X } from 'lucide-react';
import { useAegisStore, AgentNodeSummary, AgentRole } from '../store/useAegisStore';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

function roleName(role: AgentRole): string {
    switch (role.type) {
        case 'ChatAgent': return 'Chat Agent';
        case 'ProjectSupervisor': return role.project_id;
        case 'Supervisor': return role.name;
        case 'Specialist': return role.scope.length > 40 ? role.scope.slice(0, 40) + '…' : role.scope;
    }
}

function roleLabel(role: AgentRole): string {
    switch (role.type) {
        case 'ChatAgent': return 'Chat';
        case 'ProjectSupervisor': return 'Project Supervisor';
        case 'Supervisor': return 'Supervisor';
        case 'Specialist': return 'Specialist';
    }
}

function stateColor(state: AgentNodeSummary['state']): string {
    if (state === 'Running' || state === 'WaitingReport' || state === 'WaitingQuery') return 'text-aegis-cyan';
    if (state === 'Complete') return 'text-green-400';
    if (typeof state === 'object' && 'Failed' in state) return 'text-red-400';
    return 'text-white/20';
}

function StateIcon({ state }: { state: AgentNodeSummary['state'] }) {
    if (state === 'Running' || state === 'WaitingReport' || state === 'WaitingQuery') {
        return <Loader2 className="w-3.5 h-3.5 animate-spin shrink-0 text-aegis-cyan" />;
    }
    if (state === 'Complete') return <CheckCircle2 className="w-3.5 h-3.5 shrink-0 text-green-400" />;
    if (typeof state === 'object' && 'Failed' in state) return <AlertCircle className="w-3.5 h-3.5 shrink-0 text-red-400" />;
    return <Circle className="w-3.5 h-3.5 shrink-0 text-white/20" />;
}

const NodeDetailDrawer: React.FC<{
    node: AgentNodeSummary;
    onClose: () => void;
}> = ({ node, onClose }) => (
    <div className="fixed inset-0 z-50 flex justify-end">
        <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />
        <div className="relative w-full max-w-sm h-full bg-black border-l border-white/10 flex flex-col overflow-hidden">
            <div className="flex items-center justify-between px-4 py-3 border-b border-white/10 shrink-0">
                <div>
                    <p className="text-[11px] font-mono text-white/80">{roleName(node.role)}</p>
                    <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mt-0.5">{roleLabel(node.role)}</p>
                </div>
                <button onClick={onClose} className="p-1.5 rounded hover:bg-white/10 text-white/40 transition-colors">
                    <X className="w-4 h-4" />
                </button>
            </div>
            <div className="flex-1 overflow-y-auto p-4 space-y-4">
                <div>
                    <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mb-1">State</p>
                    <div className={cn("flex items-center gap-2 text-[11px] font-mono", stateColor(node.state))}>
                        <StateIcon state={node.state} />
                        {typeof node.state === 'string' ? node.state : `Failed: ${node.state.Failed.reason}`}
                    </div>
                </div>
                {node.model && (
                    <div>
                        <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mb-1">Model</p>
                        <p className="text-[11px] font-mono text-aegis-purple/80">{node.model}</p>
                    </div>
                )}
                {node.activity && (
                    <div>
                        <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mb-1">Current Activity</p>
                        <p className="text-[11px] font-mono text-aegis-cyan/60 italic">{node.activity}</p>
                    </div>
                )}
                {node.last_report && (
                    <div>
                        <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mb-2">Last Report</p>
                        <div className="bg-white/5 rounded-lg p-3 border border-white/10">
                            <p className="text-[11px] font-mono text-white/60 whitespace-pre-wrap">{node.last_report}</p>
                        </div>
                    </div>
                )}
            </div>
        </div>
    </div>
);

const TreeRow: React.FC<{
    node: AgentNodeSummary;
    allNodes: AgentNodeSummary[];
    depth: number;
    onSelect: (node: AgentNodeSummary) => void;
}> = ({ node, allNodes, depth, onSelect }) => {
    const [expanded, setExpanded] = useState(true);
    const children = allNodes.filter(n => node.children.includes(n.id));
    const hasChildren = children.length > 0;

    return (
        <div>
            <div
                className="flex items-center gap-2 py-1.5 rounded-lg px-2 group transition-colors cursor-pointer hover:bg-white/5"
                style={{ paddingLeft: `${depth * 16 + 8}px` }}
                onClick={(e) => {
                    if (hasChildren) {
                        e.stopPropagation();
                        setExpanded(ex => !ex);
                    }
                    onSelect(node);
                }}
            >
                <span className="w-3.5 h-3.5 shrink-0 text-white/20">
                    {hasChildren
                        ? expanded ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />
                        : null
                    }
                </span>

                <StateIcon state={node.state} />

                <div className="flex-1 min-w-0">
                    <span className={cn("text-[11px] font-mono truncate block", stateColor(node.state))}>
                        {roleName(node.role)}
                    </span>
                    {node.activity && (
                        <span className="text-[9px] font-mono text-aegis-cyan/40 italic truncate block">
                            {node.activity}
                        </span>
                    )}
                </div>

                <div className="flex items-center gap-2 shrink-0">
                    <span className="text-[8px] font-mono text-white/20">{roleLabel(node.role)}</span>
                    {node.model && (
                        <span className="text-[8px] font-mono text-aegis-purple/40">
                            {node.model.split('/').pop()}
                        </span>
                    )}
                </div>
            </div>

            {hasChildren && expanded && children.map(child => (
                <TreeRow
                    key={child.id}
                    node={child}
                    allNodes={allNodes}
                    depth={depth + 1}
                    onSelect={onSelect}
                />
            ))}
        </div>
    );
};

interface AgentTreeViewProps {
    projectId: string;
}

export const AgentTreeView: React.FC<AgentTreeViewProps> = ({ projectId }) => {
    const { agentTree, activeProjects } = useAegisStore();
    const [selectedNode, setSelectedNode] = useState<AgentNodeSummary | null>(null);

    const project = activeProjects.find(p => p.project_id === projectId);
    const root = agentTree.find(n =>
        n.role.type === 'ProjectSupervisor' &&
        (n.role as { type: 'ProjectSupervisor'; project_id: string }).project_id === projectId
    );

    if (!root) {
        return (
            <div className="flex flex-col items-center justify-center py-8 gap-2">
                <Circle className="w-6 h-6 text-white/10" />
                <p className="text-[10px] font-mono text-white/20">No active agents for this project</p>
            </div>
        );
    }

    return (
        <div className="flex flex-col gap-2">
            {project && (
                <div className="px-2 mb-2">
                    <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest">Project</p>
                    <p className="text-[12px] font-mono text-white/70 mt-0.5">{project.name}</p>
                </div>
            )}

            <div className="overflow-y-auto max-h-[400px] scrollbar-hide">
                <TreeRow
                    node={root}
                    allNodes={agentTree}
                    depth={0}
                    onSelect={setSelectedNode}
                />
            </div>

            {selectedNode && (
                <NodeDetailDrawer
                    node={selectedNode}
                    onClose={() => setSelectedNode(null)}
                />
            )}
        </div>
    );
};

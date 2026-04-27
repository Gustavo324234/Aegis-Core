import React, { useState } from 'react';
import { ChevronRight, ChevronDown, Cpu, CheckCircle2, Circle, Loader2, AlertCircle } from 'lucide-react';
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
        case 'Specialist': return role.scope.length > 30 ? role.scope.slice(0, 30) + '…' : role.scope;
    }
}

function roleLabel(role: AgentRole): string {
    switch (role.type) {
        case 'ChatAgent': return 'Chat';
        case 'ProjectSupervisor': return 'Project';
        case 'Supervisor': return 'Supervisor';
        case 'Specialist': return 'Specialist';
    }
}

function StateIndicator({ state }: { state: AgentNodeSummary['state'] }) {
    if (state === 'Running' || state === 'WaitingReport' || state === 'WaitingQuery') {
        return <Loader2 className="w-3 h-3 text-aegis-cyan animate-spin shrink-0" />;
    }
    if (state === 'Complete') {
        return <CheckCircle2 className="w-3 h-3 text-green-400 shrink-0" />;
    }
    if (typeof state === 'object' && 'Failed' in state) {
        return <AlertCircle className="w-3 h-3 text-red-400 shrink-0" />;
    }
    return <Circle className="w-3 h-3 text-white/20 shrink-0" />;
}

const AgentNodeRow: React.FC<{
    node: AgentNodeSummary;
    allNodes: AgentNodeSummary[];
    depth: number;
}> = ({ node, allNodes, depth }) => {
    const [expanded, setExpanded] = useState(true);
    const children = allNodes.filter(n => node.children.includes(n.id));
    const hasChildren = children.length > 0;
    const isActive = node.state === 'Running' || node.state === 'WaitingReport' || node.state === 'WaitingQuery';

    return (
        <div>
            <div
                className={cn(
                    "flex items-center gap-1.5 py-0.5 rounded px-1 group transition-colors",
                    isActive && "bg-aegis-cyan/5",
                    hasChildren && "cursor-pointer hover:bg-white/5"
                )}
                style={{ paddingLeft: `${depth * 12 + 4}px` }}
                onClick={() => hasChildren && setExpanded(e => !e)}
            >
                <span className="w-3 h-3 shrink-0 text-white/20">
                    {hasChildren
                        ? expanded
                            ? <ChevronDown className="w-3 h-3" />
                            : <ChevronRight className="w-3 h-3" />
                        : null
                    }
                </span>

                <StateIndicator state={node.state} />

                <span className={cn(
                    "text-[10px] font-mono truncate max-w-[140px]",
                    isActive ? "text-white/80" : "text-white/40"
                )}>
                    {roleName(node.role)}
                </span>

                <span className="text-[9px] font-mono text-white/20 shrink-0">
                    {roleLabel(node.role)}
                </span>

                {node.model && (
                    <span className="text-[8px] font-mono text-aegis-purple/50 shrink-0 ml-auto">
                        {node.model.split('/').pop()}
                    </span>
                )}
            </div>

            {node.activity && isActive && (
                <div
                    className="text-[9px] font-mono text-aegis-cyan/40 italic truncate py-0.5"
                    style={{ paddingLeft: `${depth * 12 + 24}px` }}
                >
                    {node.activity}
                </div>
            )}

            {hasChildren && expanded && (
                <div>
                    {children.map(child => (
                        <AgentNodeRow
                            key={child.id}
                            node={child}
                            allNodes={allNodes}
                            depth={depth + 1}
                        />
                    ))}
                </div>
            )}
        </div>
    );
};

export const AgentActivityPanel: React.FC = () => {
    const { agentTree } = useAegisStore();
    const [panelExpanded, setPanelExpanded] = useState(false);

    const workingAgents = agentTree.filter(n =>
        n.role.type !== 'ChatAgent' &&
        (n.state === 'Running' || n.state === 'WaitingReport' || n.state === 'WaitingQuery')
    );

    if (workingAgents.length === 0) return null;

    const roots = agentTree.filter(n =>
        n.role.type === 'ProjectSupervisor' &&
        n.parent_id === null
    );

    return (
        <div className="max-w-4xl mx-auto mb-2">
            <div className="bg-black/60 border border-aegis-cyan/10 rounded-xl overflow-hidden backdrop-blur-sm">

                <button
                    onClick={() => setPanelExpanded(e => !e)}
                    className="w-full flex items-center gap-2 px-3 py-2 hover:bg-white/5 transition-colors"
                >
                    <Cpu className="w-3 h-3 text-aegis-cyan/60 shrink-0" />
                    <span className="text-[9px] font-mono text-aegis-cyan/60 uppercase tracking-widest">
                        {workingAgents.length} agent{workingAgents.length !== 1 ? 's' : ''} active
                    </span>
                    <span className="ml-auto text-white/20">
                        {panelExpanded
                            ? <ChevronDown className="w-3 h-3" />
                            : <ChevronRight className="w-3 h-3" />
                        }
                    </span>
                </button>

                {panelExpanded && (
                    <div className="border-t border-white/5 px-1 py-1 max-h-48 overflow-y-auto scrollbar-hide">
                        {roots.length > 0
                            ? roots.map(root => (
                                <AgentNodeRow
                                    key={root.id}
                                    node={root}
                                    allNodes={agentTree}
                                    depth={0}
                                />
                            ))
                            : <p className="text-[9px] font-mono text-white/20 px-3 py-2">No active projects</p>
                        }
                    </div>
                )}
            </div>
        </div>
    );
};

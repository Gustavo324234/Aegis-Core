import React, { useCallback, useEffect, useState } from 'react';
import { FolderOpen, FolderArchive, Circle, Loader2, Lock, Unlock } from 'lucide-react';
import { useAegisStore, ProjectSummary } from '../store/useAegisStore';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
    return twMerge(clsx(inputs));
}

interface ProjectListProps {
    onSelectProject: (projectId: string) => void;
    selectedProjectId: string | null;
}

export const ProjectList: React.FC<ProjectListProps> = ({ onSelectProject, selectedProjectId }) => {
    const { activeProjects, agentTree, tenantId, sessionKey } = useAegisStore();
    // Project IDs the user has put in autonomous mode (skip permission prompts).
    const [autonomous, setAutonomous] = useState<Set<string>>(new Set());

    const authHeaders = useCallback((): Record<string, string> | null => {
        if (!tenantId || !sessionKey) return null;
        return { 'x-citadel-tenant': tenantId, 'x-citadel-key': sessionKey };
    }, [tenantId, sessionKey]);

    // Load the current autonomous-project list once we have credentials.
    useEffect(() => {
        const headers = authHeaders();
        if (!headers) return;
        let cancelled = false;
        (async () => {
            try {
                const res = await fetch('/api/agents/projects/autonomous', { headers });
                if (!res.ok) return;
                const data = (await res.json()) as { autonomous_projects?: string[] };
                if (!cancelled) setAutonomous(new Set(data.autonomous_projects ?? []));
            } catch {
                /* non-fatal: leave the toggles in their default (off) state */
            }
        })();
        return () => {
            cancelled = true;
        };
    }, [authHeaders]);

    const toggleAutonomous = useCallback(
        async (projectId: string) => {
            const headers = authHeaders();
            if (!headers) return;
            const enabled = !autonomous.has(projectId);
            // Optimistic update — revert if the request fails.
            setAutonomous(prev => {
                const next = new Set(prev);
                if (enabled) next.add(projectId);
                else next.delete(projectId);
                return next;
            });
            try {
                const res = await fetch(
                    `/api/agents/projects/${encodeURIComponent(projectId)}/autonomous`,
                    {
                        method: 'POST',
                        headers: { ...headers, 'Content-Type': 'application/json' },
                        body: JSON.stringify({ enabled }),
                    },
                );
                if (!res.ok) throw new Error(`HTTP ${res.status}`);
            } catch {
                setAutonomous(prev => {
                    const next = new Set(prev);
                    if (enabled) next.delete(projectId);
                    else next.add(projectId);
                    return next;
                });
            }
        },
        [authHeaders, autonomous],
    );

    if (activeProjects.length === 0) {
        return (
            <div className="flex flex-col items-center justify-center py-8 gap-2">
                <FolderOpen className="w-6 h-6 text-white/10" />
                <p className="text-[10px] font-mono text-white/20 uppercase tracking-widest">No active projects</p>
            </div>
        );
    }

    const getProjectAgentCount = (project: ProjectSummary) => {
        if (!project.root_agent_id) return 0;
        const countDescendants = (nodeId: string): number => {
            const node = agentTree.find(n => n.id === nodeId);
            if (!node) return 0;
            return 1 + node.children.reduce((acc, childId) => acc + countDescendants(childId), 0);
        };
        return countDescendants(project.root_agent_id);
    };

    const getProjectStatus = (project: ProjectSummary): 'active' | 'idle' | 'archived' => {
        if (project.status === 'archived') return 'archived';
        const agentCount = getProjectAgentCount(project);
        if (agentCount > 0) {
            const root = agentTree.find(n => n.id === project.root_agent_id);
            if (root && (root.state === 'Running' || root.state === 'WaitingReport')) return 'active';
        }
        return 'idle';
    };

    return (
        <div className="flex flex-col gap-1">
            {activeProjects.map(project => {
                const status = getProjectStatus(project);
                const agentCount = getProjectAgentCount(project);
                const isSelected = project.project_id === selectedProjectId;
                const isAutonomous = autonomous.has(project.project_id);

                return (
                    <div key={project.project_id} className="flex items-center gap-1">
                        <button
                            onClick={() => onSelectProject(project.project_id)}
                            className={cn(
                                "flex-1 min-w-0 flex items-center gap-3 px-3 py-2.5 rounded-lg transition-all text-left",
                                isSelected
                                    ? "bg-aegis-cyan/10 border border-aegis-cyan/20"
                                    : "hover:bg-white/5 border border-transparent"
                            )}
                        >
                            {status === 'archived'
                                ? <FolderArchive className="w-4 h-4 text-white/20 shrink-0" />
                                : <FolderOpen className={cn("w-4 h-4 shrink-0", isSelected ? "text-aegis-cyan" : "text-white/40")} />
                            }

                            <div className="flex-1 min-w-0">
                                <p className={cn(
                                    "text-[11px] font-mono truncate",
                                    isSelected ? "text-aegis-cyan" : "text-white/70"
                                )}>
                                    {project.name}
                                </p>
                                {project.description && (
                                    <p className="text-[9px] font-mono text-white/20 truncate mt-0.5">
                                        {project.description}
                                    </p>
                                )}
                            </div>

                            <div className="flex items-center gap-2 shrink-0">
                                {agentCount > 0 && (
                                    <span className="text-[9px] font-mono text-white/30">
                                        {agentCount} agent{agentCount !== 1 ? 's' : ''}
                                    </span>
                                )}
                                {status === 'active' && (
                                    <Loader2 className="w-3 h-3 text-aegis-cyan animate-spin" />
                                )}
                                {status === 'idle' && (
                                    <Circle className="w-2 h-2 text-green-400/50 fill-green-400/30" />
                                )}
                                {status === 'archived' && (
                                    <Circle className="w-2 h-2 text-white/10" />
                                )}
                            </div>
                        </button>

                        {/* Autonomous-mode toggle: when ON, the project's agents skip
                            the path-approval gate and ask_user auto-approves. */}
                        <button
                            type="button"
                            onClick={(e) => {
                                e.stopPropagation();
                                void toggleAutonomous(project.project_id);
                            }}
                            title={isAutonomous
                                ? 'Modo autónomo: ON — sin pedir permisos en este proyecto'
                                : 'Modo autónomo: OFF — pide aprobación para accesos externos'}
                            aria-pressed={isAutonomous}
                            className={cn(
                                "shrink-0 p-1.5 rounded-lg border transition-colors",
                                isAutonomous
                                    ? "text-aegis-cyan border-aegis-cyan/30 bg-aegis-cyan/10"
                                    : "text-white/25 border-transparent hover:text-white/50 hover:bg-white/5"
                            )}
                        >
                            {isAutonomous ? <Unlock className="w-3.5 h-3.5" /> : <Lock className="w-3.5 h-3.5" />}
                        </button>
                    </div>
                );
            })}
        </div>
    );
};

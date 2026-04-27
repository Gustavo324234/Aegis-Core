import React from 'react';
import { FolderOpen, FolderArchive, Circle, Loader2 } from 'lucide-react';
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
    const { activeProjects, agentTree } = useAegisStore();

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

                return (
                    <button
                        key={project.project_id}
                        onClick={() => onSelectProject(project.project_id)}
                        className={cn(
                            "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg transition-all text-left",
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
                );
            })}
        </div>
    );
};

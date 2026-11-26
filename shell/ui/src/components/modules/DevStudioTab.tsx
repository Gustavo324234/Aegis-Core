import React, { useState } from 'react';
import {
    Code2,
    Plus,
    Trash2,
    Copy,
    Check,
    Bug,
    FolderGit2,
    Terminal,
    ArrowRight,
    ArrowLeft,
    Bot,
    Search,
} from 'lucide-react';
import {
    DEV_COLUMNS,
    DEV_COLUMN_LABELS,
    DevOverview,
    RunModuleAction,
} from './types';

export interface DevStudioTabProps {
    devData: DevOverview;
    busyTool: boolean;
    copiedKey: string | null;
    handleCopy: (key: string, text: string) => void;
    runActionAndRefresh: RunModuleAction;
    askAegisWithPrompt: (prompt: string) => void;
    onOpenWorkspace: () => void;
}

export const DevStudioTab: React.FC<DevStudioTabProps> = ({
    devData,
    busyTool,
    copiedKey,
    handleCopy,
    runActionAndRefresh,
    askAegisWithPrompt,
    onOpenWorkspace,
}) => {
    const [devTab, setDevTab] = useState<'kanban' | 'projects' | 'snippets'>('kanban');
    const [newTaskTitle, setNewTaskTitle] = useState('');
    const [newTaskKind, setNewTaskKind] = useState('feature');
    const [newTaskPriority, setNewTaskPriority] = useState('medium');
    const [newTaskFiles, setNewTaskFiles] = useState('');

    const [newProjForm, setNewProjForm] = useState({
        name: '',
        stack: 'Rust + React',
        repo_path: '',
        build_cmd: '',
        test_cmd: '',
    });
    const [newSnippetForm, setNewSnippetForm] = useState({
        title: '',
        language: 'bash',
        code: '',
        tags: '',
    });
    const [snippetSearch, setSnippetSearch] = useState('');

    return (
        <div className="space-y-5">
            <div className="grid grid-cols-2 sm:grid-cols-5 gap-3">
                <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                    <p className="text-[10px] font-mono text-white/40 uppercase">Proyectos</p>
                    <p className="text-2xl font-bold text-white mt-1">{devData.stats.total_projects}</p>
                </div>
                <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                    <p className="text-[10px] font-mono text-white/40 uppercase">Tareas Abiertas</p>
                    <p className="text-2xl font-bold text-aegis-cyan mt-1">{devData.stats.open_tasks}</p>
                </div>
                <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                    <p className="text-[10px] font-mono text-white/40 uppercase">En Curso</p>
                    <p className="text-2xl font-bold text-amber-300 mt-1">{devData.stats.in_progress_tasks}</p>
                </div>
                <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                    <p className="text-[10px] font-mono text-white/40 uppercase">Bugs Activos</p>
                    <p className="text-2xl font-bold text-red-400 mt-1">{devData.stats.bugs_count}</p>
                </div>
                <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10">
                    <p className="text-[10px] font-mono text-white/40 uppercase">Snippets</p>
                    <p className="text-2xl font-bold text-emerald-300 mt-1">{devData.stats.total_snippets}</p>
                </div>
            </div>

            <div className="flex flex-wrap items-center justify-between gap-3 border-b border-white/10 pb-3">
                <div className="flex items-center gap-2">
                    {[
                        { id: 'kanban', label: 'Kanban Técnico', icon: Bug },
                        { id: 'projects', label: 'Proyectos & Repos', icon: FolderGit2 },
                        { id: 'snippets', label: 'Snippets & Runbooks', icon: Terminal },
                    ].map((tab) => {
                        const TIcon = tab.icon;
                        return (
                            <button
                                key={tab.id}
                                onClick={() => setDevTab(tab.id as 'kanban' | 'projects' | 'snippets')}
                                className={`px-3.5 py-2 rounded-xl text-xs font-mono flex items-center gap-2 border transition-all ${
                                    devTab === tab.id
                                        ? 'bg-aegis-cyan/20 text-aegis-cyan border-aegis-cyan/40'
                                        : 'bg-white/5 text-white/55 border-white/10 hover:text-white'
                                }`}
                            >
                                <TIcon className="w-3.5 h-3.5" />
                                {tab.label}
                            </button>
                        );
                    })}
                </div>

                <button
                    onClick={onOpenWorkspace}
                    className="px-3.5 py-2 rounded-xl text-xs font-mono bg-white/5 hover:bg-white/10 border border-white/15 text-white/80 flex items-center gap-2"
                >
                    <Code2 className="w-3.5 h-3.5 text-aegis-cyan" />
                    Abrir Terminal / Git / Código
                </button>
            </div>

            {/* Dev Tab 1: Kanban */}
            {devTab === 'kanban' && (
                <div className="space-y-4">
                    <div className="p-3.5 rounded-xl bg-white/[0.03] border border-white/10 grid grid-cols-1 sm:grid-cols-12 gap-2.5 items-center">
                        <input
                            value={newTaskTitle}
                            onChange={(e) => setNewTaskTitle(e.target.value)}
                            placeholder="Nueva tarea técnica o bug..."
                            className="sm:col-span-5 px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none focus:border-aegis-cyan"
                        />
                        <select
                            value={newTaskKind}
                            onChange={(e) => setNewTaskKind(e.target.value)}
                            className="sm:col-span-2 px-2.5 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                        >
                            <option value="feature">Feature</option>
                            <option value="bug">Bug</option>
                            <option value="refactor">Refactor</option>
                            <option value="security">Security</option>
                            <option value="devops">DevOps</option>
                        </select>
                        <select
                            value={newTaskPriority}
                            onChange={(e) => setNewTaskPriority(e.target.value)}
                            className="sm:col-span-2 px-2.5 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                        >
                            <option value="critical">Crítica</option>
                            <option value="high">Alta</option>
                            <option value="medium">Media</option>
                            <option value="low">Baja</option>
                        </select>
                        <input
                            value={newTaskFiles}
                            onChange={(e) => setNewTaskFiles(e.target.value)}
                            placeholder="Archivos (opc.)"
                            className="sm:col-span-2 px-2.5 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                        />
                        <button
                            disabled={busyTool || !newTaskTitle.trim()}
                            onClick={async () => {
                                await runActionAndRefresh('aegis.domain.dev', 'dev_manage_task', {
                                    action: 'add',
                                    title: newTaskTitle.trim(),
                                    kind: newTaskKind,
                                    priority: newTaskPriority,
                                    file_refs: newTaskFiles.trim(),
                                });
                                setNewTaskTitle('');
                                setNewTaskFiles('');
                            }}
                            className="sm:col-span-1 py-2 px-3 rounded-lg bg-aegis-cyan text-black font-mono text-xs font-bold disabled:opacity-40 flex items-center justify-center"
                        >
                            <Plus className="w-4 h-4" />
                        </button>
                    </div>

                    <div className="grid grid-cols-1 md:grid-cols-4 gap-3.5">
                        {DEV_COLUMNS.map((col, colIdx) => {
                            const colTasks = devData.tasks.filter((t) => t.status === col);
                            return (
                                <div
                                    key={col}
                                    className="p-3 rounded-xl bg-white/[0.02] border border-white/10 flex flex-col gap-2.5 min-h-[260px]"
                                >
                                    <div className="flex items-center justify-between text-xs font-mono uppercase tracking-wider text-white/60 pb-1 border-b border-white/10">
                                        <span>{DEV_COLUMN_LABELS[col]}</span>
                                        <span className="px-2 py-0.5 rounded-full bg-white/5 text-[10px]">
                                            {colTasks.length}
                                        </span>
                                    </div>
                                    {colTasks.map((t) => (
                                        <div
                                            key={t.id}
                                            className="p-3 rounded-lg bg-black/50 border border-white/10 space-y-2"
                                        >
                                            <div className="flex items-center justify-between gap-1 text-[10px] font-mono">
                                                <span
                                                    className={`px-1.5 py-0.5 rounded uppercase ${
                                                        t.kind === 'bug'
                                                            ? 'bg-red-500/20 text-red-300'
                                                            : t.kind === 'security'
                                                              ? 'bg-amber-500/20 text-amber-300'
                                                              : 'bg-aegis-cyan/15 text-aegis-cyan'
                                                    }`}
                                                >
                                                    {t.kind}
                                                </span>
                                                <span className="text-white/40">{t.priority}</span>
                                            </div>
                                            <p className="text-xs font-medium text-white">{t.title}</p>
                                            {t.file_refs && (
                                                <p className="text-[10px] font-mono text-white/40 truncate">
                                                    {t.file_refs}
                                                </p>
                                            )}
                                            <div className="flex items-center justify-between pt-1 border-t border-white/10">
                                                <div className="flex items-center gap-1">
                                                    {colIdx > 0 && (
                                                        <button
                                                            onClick={() =>
                                                                void runActionAndRefresh(
                                                                    'aegis.domain.dev',
                                                                    'dev_manage_task',
                                                                    {
                                                                        action: 'update',
                                                                        id: t.id,
                                                                        status: DEV_COLUMNS[colIdx - 1],
                                                                    },
                                                                )
                                                            }
                                                            className="p-1 rounded hover:bg-white/10 text-white/50"
                                                            title="Mover atrás"
                                                        >
                                                            <ArrowLeft className="w-3 h-3" />
                                                        </button>
                                                    )}
                                                    {colIdx < DEV_COLUMNS.length - 1 && (
                                                        <button
                                                            onClick={() =>
                                                                void runActionAndRefresh(
                                                                    'aegis.domain.dev',
                                                                    'dev_manage_task',
                                                                    {
                                                                        action: 'update',
                                                                        id: t.id,
                                                                        status: DEV_COLUMNS[colIdx + 1],
                                                                    },
                                                                )
                                                            }
                                                            className="p-1 rounded hover:bg-white/10 text-white/50"
                                                            title="Avanzar etapa"
                                                        >
                                                            <ArrowRight className="w-3 h-3" />
                                                        </button>
                                                    )}
                                                </div>
                                                <div className="flex items-center gap-1">
                                                    <button
                                                        onClick={() =>
                                                            askAegisWithPrompt(
                                                                `Delegá a un agente especializado esta tarea de Aegis Dev Studio (${t.kind} / prioridad ${t.priority}): "${t.title}"${t.file_refs ? ` en ${t.file_refs}` : ''}${t.notes ? `. Contexto: ${t.notes}` : ''}. Cuando termines, actualizá su estado con dev_manage_task.`,
                                                            )
                                                        }
                                                        className="px-1.5 py-0.5 rounded bg-aegis-cyan/15 text-aegis-cyan text-[10px] font-mono flex items-center gap-1 hover:bg-aegis-cyan/25"
                                                        title="Delegar a Aegis / Supervisor"
                                                    >
                                                        <Bot className="w-3 h-3" />
                                                        Delegar
                                                    </button>
                                                    <button
                                                        onClick={() =>
                                                            void runActionAndRefresh(
                                                                'aegis.domain.dev',
                                                                'dev_manage_task',
                                                                { action: 'delete', id: t.id },
                                                            )
                                                        }
                                                        className="p-1 rounded text-white/30 hover:text-red-400"
                                                    >
                                                        <Trash2 className="w-3 h-3" />
                                                    </button>
                                                </div>
                                            </div>
                                        </div>
                                    ))}
                                </div>
                            );
                        })}
                    </div>
                </div>
            )}

            {/* Dev Tab 2: Projects */}
            {devTab === 'projects' && (
                <div className="grid grid-cols-1 lg:grid-cols-12 gap-5">
                    <div className="lg:col-span-4 p-4 rounded-2xl bg-white/[0.03] border border-white/10 space-y-2.5 h-fit">
                        <h4 className="text-xs font-mono uppercase text-aegis-cyan font-bold">
                            + Registrar Proyecto / Repo
                        </h4>
                        <input
                            value={newProjForm.name}
                            onChange={(e) => setNewProjForm({ ...newProjForm, name: e.target.value })}
                            placeholder="Nombre (ej: Aegis-Core)"
                            className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                        />
                        <input
                            value={newProjForm.stack}
                            onChange={(e) => setNewProjForm({ ...newProjForm, stack: e.target.value })}
                            placeholder="Stack (Rust, React, Python...)"
                            className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                        />
                        <input
                            value={newProjForm.repo_path}
                            onChange={(e) => setNewProjForm({ ...newProjForm, repo_path: e.target.value })}
                            placeholder="Ruta local o URL repo"
                            className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                        />
                        <input
                            value={newProjForm.build_cmd}
                            onChange={(e) => setNewProjForm({ ...newProjForm, build_cmd: e.target.value })}
                            placeholder="Build cmd (ej: cargo build --release)"
                            className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                        />
                        <input
                            value={newProjForm.test_cmd}
                            onChange={(e) => setNewProjForm({ ...newProjForm, test_cmd: e.target.value })}
                            placeholder="Test cmd (ej: cargo test)"
                            className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                        />
                        <button
                            disabled={busyTool || !newProjForm.name.trim()}
                            onClick={async () => {
                                await runActionAndRefresh('aegis.domain.dev', 'dev_manage_project', {
                                    action: 'add',
                                    ...newProjForm,
                                });
                                setNewProjForm({
                                    name: '',
                                    stack: 'Rust + React',
                                    repo_path: '',
                                    build_cmd: '',
                                    test_cmd: '',
                                });
                            }}
                            className="w-full py-2 rounded-xl bg-aegis-cyan text-black font-mono text-xs font-bold disabled:opacity-40"
                        >
                            Guardar Proyecto
                        </button>
                    </div>

                    <div className="lg:col-span-8 space-y-3">
                        {devData.projects.length === 0 ? (
                            <div className="p-8 rounded-2xl border border-dashed border-white/15 text-center text-xs font-mono text-white/40">
                                Sin proyectos registrados todavía.
                            </div>
                        ) : (
                            devData.projects.map((p) => (
                                <div
                                    key={p.id}
                                    className="p-4 rounded-xl bg-white/[0.03] border border-white/10 space-y-2"
                                >
                                    <div className="flex items-start justify-between gap-2">
                                        <div>
                                            <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-aegis-cyan/15 text-aegis-cyan mr-2">
                                                {p.stack}
                                            </span>
                                            <span className="text-[10px] font-mono text-white/45">
                                                rama: {p.default_branch}
                                            </span>
                                            <h5 className="text-sm font-bold text-white mt-1">{p.name}</h5>
                                            {p.repo_path && (
                                                <p className="text-xs font-mono text-white/45">{p.repo_path}</p>
                                            )}
                                        </div>
                                        <button
                                            onClick={() =>
                                                void runActionAndRefresh(
                                                    'aegis.domain.dev',
                                                    'dev_manage_project',
                                                    { action: 'delete', id: p.id },
                                                )
                                            }
                                            className="p-1.5 text-white/30 hover:text-red-400"
                                        >
                                            <Trash2 className="w-3.5 h-3.5" />
                                        </button>
                                    </div>
                                    <div className="flex flex-wrap gap-2 pt-1">
                                        {p.build_cmd && (
                                            <button
                                                onClick={() => handleCopy(`b-${p.id}`, p.build_cmd)}
                                                className="px-2.5 py-1 rounded-lg bg-black/50 border border-white/10 text-[11px] font-mono text-white/75 hover:text-aegis-cyan flex items-center gap-1.5"
                                            >
                                                <Copy className="w-3 h-3" />
                                                Build: {p.build_cmd}
                                            </button>
                                        )}
                                        {p.test_cmd && (
                                            <button
                                                onClick={() => handleCopy(`t-${p.id}`, p.test_cmd)}
                                                className="px-2.5 py-1 rounded-lg bg-black/50 border border-white/10 text-[11px] font-mono text-white/75 hover:text-emerald-300 flex items-center gap-1.5"
                                            >
                                                <Copy className="w-3 h-3" />
                                                Test: {p.test_cmd}
                                            </button>
                                        )}
                                    </div>
                                </div>
                            ))
                        )}
                    </div>
                </div>
            )}

            {/* Dev Tab 3: Snippets */}
            {devTab === 'snippets' && (
                <div className="grid grid-cols-1 lg:grid-cols-12 gap-5">
                    <div className="lg:col-span-4 p-4 rounded-2xl bg-white/[0.03] border border-white/10 space-y-2.5 h-fit">
                        <h4 className="text-xs font-mono uppercase text-aegis-cyan font-bold">
                            + Guardar Snippet / Runbook
                        </h4>
                        <div className="grid grid-cols-3 gap-2">
                            <input
                                value={newSnippetForm.title}
                                onChange={(e) => setNewSnippetForm({ ...newSnippetForm, title: e.target.value })}
                                placeholder="Título"
                                className="col-span-2 px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                            />
                            <input
                                value={newSnippetForm.language}
                                onChange={(e) => setNewSnippetForm({ ...newSnippetForm, language: e.target.value })}
                                placeholder="bash/rust"
                                className="px-2.5 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                            />
                        </div>
                        <textarea
                            value={newSnippetForm.code}
                            onChange={(e) => setNewSnippetForm({ ...newSnippetForm, code: e.target.value })}
                            placeholder="Pegá el comando o código..."
                            rows={5}
                            className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs font-mono text-white outline-none"
                        />
                        <input
                            value={newSnippetForm.tags}
                            onChange={(e) => setNewSnippetForm({ ...newSnippetForm, tags: e.target.value })}
                            placeholder="Tags (docker, deploy, sql)"
                            className="w-full px-3 py-2 rounded-lg bg-black/50 border border-white/15 text-xs text-white outline-none"
                        />
                        <button
                            disabled={busyTool || !newSnippetForm.title.trim()}
                            onClick={async () => {
                                await runActionAndRefresh('aegis.domain.dev', 'dev_manage_snippet', {
                                    action: 'save',
                                    ...newSnippetForm,
                                });
                                setNewSnippetForm({ title: '', language: 'bash', code: '', tags: '' });
                            }}
                            className="w-full py-2 rounded-xl bg-aegis-cyan text-black font-mono text-xs font-bold disabled:opacity-40"
                        >
                            Guardar en Bóveda
                        </button>
                    </div>

                    <div className="lg:col-span-8 space-y-3">
                        <div className="relative">
                            <Search className="w-3.5 h-3.5 text-white/35 absolute left-3 top-2.5" />
                            <input
                                value={snippetSearch}
                                onChange={(e) => setSnippetSearch(e.target.value)}
                                placeholder="Buscar snippet por título, lenguaje o tag..."
                                className="w-full pl-9 pr-3 py-2 rounded-lg bg-white/5 border border-white/10 text-xs text-white outline-none"
                            />
                        </div>
                        {devData.snippets
                            .filter(
                                (s) =>
                                    !snippetSearch.trim() ||
                                    `${s.title} ${s.language} ${s.tags}`
                                        .toLowerCase()
                                        .includes(snippetSearch.toLowerCase()),
                            )
                            .map((snip) => (
                                <div
                                    key={snip.id}
                                    className="p-4 rounded-xl bg-white/[0.03] border border-white/10 space-y-2"
                                >
                                    <div className="flex items-center justify-between gap-2">
                                        <div className="flex items-center gap-2">
                                            <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-aegis-cyan/15 text-aegis-cyan">
                                                {snip.language}
                                            </span>
                                            <h5 className="text-sm font-semibold text-white">{snip.title}</h5>
                                        </div>
                                        <div className="flex items-center gap-1.5">
                                            <button
                                                onClick={() => handleCopy(`snp-${snip.id}`, snip.code)}
                                                className="px-2.5 py-1 rounded-lg bg-white/5 hover:bg-white/10 text-[11px] font-mono text-aegis-cyan flex items-center gap-1"
                                            >
                                                {copiedKey === `snp-${snip.id}` ? (
                                                    <Check className="w-3 h-3" />
                                                ) : (
                                                    <Copy className="w-3 h-3" />
                                                )}
                                                Copiar
                                            </button>
                                            <button
                                                onClick={() =>
                                                    void runActionAndRefresh(
                                                        'aegis.domain.dev',
                                                        'dev_manage_snippet',
                                                        { action: 'delete', id: snip.id },
                                                    )
                                                }
                                                className="p-1.5 text-white/30 hover:text-red-400"
                                            >
                                                <Trash2 className="w-3.5 h-3.5" />
                                            </button>
                                        </div>
                                    </div>
                                    <pre className="p-3 rounded-lg bg-black/60 border border-white/5 text-[11px] font-mono text-white/80 overflow-x-auto">
                                        {snip.code}
                                    </pre>
                                </div>
                            ))}
                    </div>
                </div>
            )}
        </div>
    );
};

import type {
    AgentEventPayload,
    AgentNodeState,
    AgentNodeSummary,
    AgentRole,
    Message,
} from './types';

export const STORE_VERSION = 5;

export const storeRuntime: {
    telemetryInterval: number | null;
    chatReconnectTimer: ReturnType<typeof setTimeout> | null;
    chatReconnectAttempts: number;
    chatAuthRejected: boolean;
    chatManualDisconnect: boolean;
    pendingRetryTimer: ReturnType<typeof setTimeout> | null;
} = {
    telemetryInterval: null,
    chatReconnectTimer: null,
    chatReconnectAttempts: 0,
    chatAuthRejected: false,
    chatManualDisconnect: false,
    pendingRetryTimer: null,
};

export const clearTelemetryInterval = () => {
    if (storeRuntime.telemetryInterval !== null) {
        clearInterval(storeRuntime.telemetryInterval);
        storeRuntime.telemetryInterval = null;
    }
};

export const clearChatReconnectTimer = () => {
    if (storeRuntime.chatReconnectTimer !== null) {
        clearTimeout(storeRuntime.chatReconnectTimer);
        storeRuntime.chatReconnectTimer = null;
    }
};

export const clearPendingRetryTimer = () => {
    if (storeRuntime.pendingRetryTimer !== null) {
        clearTimeout(storeRuntime.pendingRetryTimer);
        storeRuntime.pendingRetryTimer = null;
    }
};

export const buildWsUrl = (path: string) => {
    const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${proto}//${window.location.host}${path}`;
};

export function resolveAutoRetryDelaySecs(message: string, explicitSecs?: number): number | undefined {
    if (typeof explicitSecs === 'number' && Number.isFinite(explicitSecs) && explicitSecs > 0) {
        return Math.min(Math.max(Math.round(explicitSecs), 3), 90);
    }
    const secMatch = message.match(/~?(\d+)\s*s\b/i) ?? message.match(/en\s+(\d+)\s*segundos/i);
    if (secMatch) {
        const parsed = parseInt(secMatch[1], 10);
        if (parsed > 0 && parsed <= 90) {
            return Math.max(parsed, 3);
        }
    }
    if (/saturad|l[íi]mite|cooldown|esper[áa]|segundos|429|timeout|no devolvi[óo] respuesta|no tengo modelos/i.test(message)) {
        return 15;
    }
    return undefined;
}

export function normalizeAgentState(rawState: unknown): AgentNodeState {
    if (
        rawState === 'Idle' ||
        rawState === 'Running' ||
        rawState === 'WaitingReport' ||
        rawState === 'WaitingQuery' ||
        rawState === 'Complete'
    ) {
        return rawState;
    }
    if (rawState === 'WaitingUser') {
        return 'WaitingQuery';
    }
    if (typeof rawState === 'string' && rawState.startsWith('Failed')) {
        return { Failed: { reason: rawState } };
    }
    if (typeof rawState === 'object' && rawState !== null && 'Failed' in rawState) {
        const failed = (rawState as { Failed?: { reason?: unknown } }).Failed;
        return { Failed: { reason: typeof failed?.reason === 'string' ? failed.reason : 'Agent failed' } };
    }
    return 'Idle';
}

export function normalizeAgentRole(rawRole: unknown, roleLabel?: string, projectId?: string): AgentRole {
    if (rawRole === 'ChatAgent') {
        return { type: 'ChatAgent' };
    }
    if (typeof rawRole === 'object' && rawRole !== null) {
        const obj = rawRole as Record<string, unknown>;
        if (typeof obj.type === 'string') {
            return rawRole as AgentRole;
        }
        if (typeof obj.ProjectSupervisor === 'object' && obj.ProjectSupervisor !== null) {
            const ps = obj.ProjectSupervisor as Record<string, unknown>;
            const pid = projectId || (typeof ps.name === 'string' ? ps.name : 'project');
            return { type: 'ProjectSupervisor', project_id: pid };
        }
        if (typeof obj.Supervisor === 'object' && obj.Supervisor !== null) {
            const sup = obj.Supervisor as Record<string, unknown>;
            return {
                type: 'Supervisor',
                name: typeof sup.name === 'string' ? sup.name : 'Supervisor',
                depth: 2,
            };
        }
        if (typeof obj.Specialist === 'object' && obj.Specialist !== null) {
            const spec = obj.Specialist as Record<string, unknown>;
            return {
                type: 'Specialist',
                scope: typeof spec.scope === 'string' ? spec.scope : 'Specialist',
            };
        }
    }

    const cleanLabel = (roleLabel ?? '').replace(/\s*\[degraded\]\s*$/i, '').trim();
    if (cleanLabel.startsWith('Project Supervisor')) {
        const namePart = cleanLabel.split('—')[1]?.trim();
        return { type: 'ProjectSupervisor', project_id: projectId || namePart || 'project' };
    }
    if (cleanLabel.startsWith('Supervisor')) {
        const namePart = cleanLabel.split('—')[1]?.trim() || 'Supervisor';
        return { type: 'Supervisor', name: namePart, depth: 2 };
    }
    if (cleanLabel.startsWith('Specialist')) {
        const scopePart = cleanLabel.split('—')[1]?.trim() || 'Specialist';
        return { type: 'Specialist', scope: scopePart };
    }
    return { type: 'ChatAgent' };
}

export function normalizeSnapshotNodes(rawNodes: unknown): AgentNodeSummary[] {
    if (!Array.isArray(rawNodes)) return [];
    const items = rawNodes.filter((n): n is Record<string, unknown> => typeof n === 'object' && n !== null);
    return items.map((n) => {
        const id = typeof n.id === 'string' ? n.id : String(n.agent_id ?? '');
        const parentId = typeof n.parent_id === 'string' ? n.parent_id : null;
        const explicitChildren = Array.isArray(n.children)
            ? n.children.filter((c): c is string => typeof c === 'string')
            : items
                  .filter((child) => child.parent_id === id && id.length > 0)
                  .map((child) => (typeof child.id === 'string' ? child.id : String(child.agent_id ?? '')))
                  .filter((cid) => cid.length > 0);
        const roleLabel = typeof n.role_label === 'string' ? n.role_label : undefined;
        const projectId = typeof n.project_id === 'string' ? n.project_id : undefined;

        return {
            id,
            role: normalizeAgentRole(n.role, roleLabel, projectId),
            state: normalizeAgentState(n.state),
            parent_id: parentId,
            children: explicitChildren,
            model: typeof n.model === 'string' ? n.model : null,
            activity: typeof n.activity === 'string' ? n.activity : null,
            last_report: typeof n.last_report === 'string' ? n.last_report : null,
        };
    });
}

export function normalizeAgentEvent(raw: Record<string, unknown>): AgentEventPayload | null {
    const rawType = typeof raw.type === 'string' ? raw.type : '';
    switch (rawType) {
        case 'tree_snapshot':
        case 'TreeSnapshot':
            return {
                type: 'TreeSnapshot',
                tree: normalizeSnapshotNodes(raw.nodes ?? raw.tree),
            };
        case 'spawned':
        case 'Spawned':
            return {
                type: 'Spawned',
                agent_id: String(raw.agent_id ?? ''),
                role: normalizeAgentRole(
                    raw.role,
                    undefined,
                    typeof raw.project_id === 'string' ? raw.project_id : undefined,
                ),
                parent_id: typeof raw.parent_id === 'string' ? raw.parent_id : null,
                model: typeof raw.model === 'string' ? raw.model : 'hybrid',
            };
        case 'state_changed':
        case 'StateChanged':
            return {
                type: 'StateChanged',
                agent_id: String(raw.agent_id ?? ''),
                state: normalizeAgentState(raw.state),
            };
        case 'activity':
        case 'Activity':
            return {
                type: 'Activity',
                agent_id: String(raw.agent_id ?? ''),
                description: typeof raw.description === 'string' ? raw.description : '',
            };
        case 'reported':
        case 'Reported':
            return {
                type: 'Reported',
                agent_id: String(raw.agent_id ?? ''),
                summary: typeof raw.summary === 'string' ? raw.summary : '',
            };
        case 'pruned':
        case 'Pruned':
            return {
                type: 'Pruned',
                agent_id: String(raw.agent_id ?? ''),
            };
        case 'restored':
        case 'Restored':
            return {
                type: 'Restored',
                project_id: String(raw.project_id ?? ''),
                node_count: typeof raw.node_count === 'number' ? raw.node_count : 0,
            };
        default:
            return null;
    }
}

export function formatAgentRole(role: AgentRole): string {
    if (!role) return 'Agente';
    if (role.type === 'ChatAgent') return 'Chat Agent';
    if (role.type === 'ProjectSupervisor') return 'Project Supervisor';
    if (role.type === 'Supervisor') return `Supervisor (${role.name})`;
    if (role.type === 'Specialist') return `Specialist (${role.scope})`;
    return 'Agente';
}

export function mapHistoricalMessages(
    items: Array<{ role: string; content: string; timestamp: string }>,
    startIndex: number,
): Message[] {
    return items.map((m, idx) => ({
        id: `hist-${startIndex + idx}-${m.timestamp}`,
        role: m.role?.toLowerCase() === 'user' ? 'user' : 'assistant',
        content: m.content,
        timestamp: new Date(m.timestamp).getTime(),
        type: 'text',
    }));
}

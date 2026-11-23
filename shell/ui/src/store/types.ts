import type { AgentEventPayload as SupervisorEventPayload } from '../types/agentEvents';

export const TAB_SESSION_KEY = 'aegis-tab-session-key';

export type AgentRole =
    | { type: 'ChatAgent' }
    | { type: 'ProjectSupervisor'; project_id: string }
    | { type: 'Supervisor'; name: string; depth: number }
    | { type: 'Specialist'; scope: string };

export type AgentNodeState =
    | 'Idle'
    | 'Running'
    | 'WaitingReport'
    | 'WaitingQuery'
    | 'Complete'
    | { Failed: { reason: string } };

export interface AgentNodeSummary {
    id: string;
    role: AgentRole;
    state: AgentNodeState;
    parent_id: string | null;
    children: string[];
    model: string | null;
    activity: string | null;
    last_report: string | null;
}

export interface ProjectSummary {
    project_id: string;
    name: string;
    description: string | null;
    status: 'active' | 'archived';
    root_agent_id: string | null;
}

export type AgentEventPayload =
    | { type: 'Spawned'; agent_id: string; role: AgentRole; parent_id: string | null; model: string }
    | { type: 'StateChanged'; agent_id: string; state: AgentNodeState }
    | { type: 'Activity'; agent_id: string; description: string }
    | { type: 'Reported'; agent_id: string; summary: string }
    | { type: 'Pruned'; agent_id: string }
    | { type: 'Restored'; project_id: string; node_count: number }
    | { type: 'TreeSnapshot'; tree: AgentNodeSummary[] };

export type TrainingStatus =
    | 'Idle'
    | 'Preparing'
    | 'Training'
    | 'Exporting'
    | 'Completed'
    | { Failed: string }
    | 'Cancelled';

export interface TrainingProgress {
    status: TrainingStatus;
    epoch: number;
    step: number;
    loss: number;
    eta_seconds: number;
    log_snippet: string;
}

export interface TrainingConfig {
    mode: 'local' | 'cloud';
    model_id: string;
    dataset_path: string;
    epochs: number;
    learning_rate: number;
    batch_size: number;
    cloud_api_key?: string | null;
}

export type MessageType = 'text' | 'thought' | 'system' | 'error';
export type SystemStatus = 'disconnected' | 'connecting' | 'idle' | 'thinking' | 'executing_syscall' | 'error' | 'listening' | 'transcribing';
export type TaskTypeValue = 'chat' | 'coding' | 'planning' | 'analysis' | 'summarization';

export interface RoutingInfo {
    model_id: string;
    provider: string;
    task_type: string;
    latency_ms: number;
}

export interface Message {
    id: string;
    role: 'user' | 'assistant' | 'system';
    content: string;
    type: MessageType;
    timestamp: number;
}

export interface SystemMetrics {
    cpu_load: number;
    vram_allocated_mb: number;
    vram_total_mb: number;
    total_processes: number;
    active_workers: number;
}

export interface PendingRetryPrompt {
    prompt: string;
    displayPrompt: string;
    modelOverride?: string | null;
    retryAfterSecs?: number;
    errorMessage?: string;
    autoRetryCount: number;
}

export interface AegisState {
    messages: Message[];
    historyTotalCount: number;
    nextHistoryBeforeIndex: number | null;
    hasMoreHistory: boolean;
    isLoadingHistory: boolean;
    cleanOnOpen: boolean;
    historyPrependedSeq: number;
    status: SystemStatus;
    system_metrics: SystemMetrics;
    tenants: string[];
    socket: WebSocket | null;
    activePid: string | null;
    isAuthenticated: boolean;
    isAdmin: boolean;
    systemState: 'STATE_INITIALIZING' | 'STATE_OPERATIONAL' | 'UNKNOWN';
    tenantId: string | null;
    sessionKey: string | null;
    isRecording: boolean;
    sirenSocket: RTCDataChannel | null;
    isEngineConfigured: boolean;
    taskType: TaskTypeValue;
    lastRoutingInfo: RoutingInfo | null;
    liveToolProgress: { stage?: string; message?: string; progress?: number; done?: boolean } | null;
    lastError: string | null;
    lastSubmittedPrompt: {
        prompt: string;
        displayPrompt: string;
        modelOverride?: string | null;
        autoRetryCount: number;
    } | null;
    pendingRetryPrompt: PendingRetryPrompt | null;
    _hydrated: boolean;
    needsPasswordReset: boolean;
    adminActiveTab: string;
    isFetchingTenants: boolean;
    lastTenantsUpdate: string | null;
    tenantsError: string | null;
    sttAvailable: boolean;
    sttProvider: string;
    sttApiKey: string;
    currentView: 'chat' | 'dashboard' | 'agents' | 'agent_thread';
    activeAgentId: string | null;
    inputMode: 'text' | 'audio' | 'conversation' | 'wakeword';
    voiceEnabled: boolean;
    agentName: string | null;
    isWakewordListening: boolean;

    // CORE-204 — agent stream state
    agentTree: AgentNodeSummary[];
    activeProjects: ProjectSummary[];
    agentSocket: WebSocket | null;
    isAgentStreamConnected: boolean;
    agentStreamError: string | null;

    // CORE-331 — native training engine state
    trainingProgress: TrainingProgress | null;
    trainingLogs: string[];
    trainingSocket: WebSocket | null;

    startTraining: (config: TrainingConfig) => Promise<boolean>;
    cancelTraining: () => Promise<boolean>;
    fetchTrainingStatus: () => Promise<void>;
    connectTrainingStream: () => void;
    disconnectTrainingStream: () => void;

    connectAgentStream: () => void;
    disconnectAgentStream: () => void;
    fetchActiveProjects: () => Promise<void>;

    setHydrated: (val: boolean) => void;
    setInputMode: (mode: 'text' | 'audio' | 'conversation' | 'wakeword') => void;
    fetchAgentName: () => Promise<void>;
    startWakewordListener: () => void;
    stopWakewordListener: () => void;
    setVoiceEnabled: (val: boolean) => void;
    setCurrentView: (view: 'chat' | 'dashboard' | 'agents' | 'agent_thread') => void;
    setActiveAgentId: (agentId: string | null) => void;
    setNeedsPasswordReset: (val: boolean) => void;
    setAdminActiveTab: (tab: string) => void;
    fetchSirenConfig: () => Promise<void>;
    connect: (tenantId: string, sessionKey: string) => Promise<void>;
    disconnect: () => void;
    sendMessage: (prompt: string, modelOverride?: string | null, isRetry?: boolean) => void;
    retryPendingMessage: (newModelOverride?: string | null) => void;
    dismissPendingRetry: () => void;
    appendToken: (msgId: string, token: string, type: MessageType) => void;
    setStatus: (status: SystemStatus) => void;
    clearHistory: () => Promise<void>;
    clearAgentTraces: () => Promise<void>;
    addSystemMessage: (content: string) => void;
    startTelemetryPolling: (tenantId: string) => void;
    fetchSystemState: () => Promise<void>;
    setAuth: (tenantId: string, sessionKey: string) => void;
    authenticate: (tenantId: string, passphrase: string) => Promise<'authenticated' | 'password_must_change' | 'failed'>;
    logout: () => void;
    fetchTenants: () => Promise<void>;
    createTenant: (targetUsername: string) => Promise<{ success: boolean; message?: string; temporary_passphrase?: string }>;
    deleteTenant: (targetId: string) => Promise<boolean>;
    resetPassword: (targetId: string, newPass: string) => Promise<boolean>;
    changeOwnPassword: (newPass: string) => Promise<boolean>;
    startSirenStream: () => Promise<void>;
    stopSirenStream: () => void;
    restartMicForConversation: () => Promise<void>;
    configureEngine: (apiUrl: string, model: string, apiKey: string, provider?: string) => Promise<boolean>;
    setEngineConfigured: (configured: boolean) => void;
    setTaskType: (taskType: TaskTypeValue) => void;
    setLastRoutingInfo: (info: RoutingInfo) => void;
    loadChatHistory: (tenantId: string, sessionKey: string) => Promise<void>;
    loadOlderHistory: (limit?: number) => Promise<number>;
    clearChatView: () => Promise<void>;
    setCleanOnOpen: (val: boolean) => Promise<void>;
}

export type StoreSet = (
    partial: AegisState | Partial<AegisState> | ((state: AegisState) => AegisState | Partial<AegisState>),
) => void;
export type StoreGet = () => AegisState;
export type { SupervisorEventPayload };

// CORE-269: store de bandeja de entrada de supervisores
import { create } from 'zustand';

export interface ThreadMessage {
  role: 'supervisor' | 'user';
  content: string;
  timestamp: string;
}

export interface AgentMessage {
  agentId: string;
  projectName: string;
  question: string;
  context?: string;
  timestamp: string;
  status: 'pending' | 'answered' | 'timed_out';
  thread: ThreadMessage[];
}

interface AgentInboxState {
  messages: AgentMessage[];
  pendingCount: number;
  addMessage: (msg: Omit<AgentMessage, 'thread'>) => void;
  markAnswered: (agentId: string) => void;
  markTimedOut: (agentId: string) => void;
  addThreadMessage: (agentId: string, msg: ThreadMessage) => void;
  getByAgentId: (agentId: string) => AgentMessage | undefined;
}

export const useAgentInboxStore = create<AgentInboxState>((set, get) => ({
  messages: [],
  pendingCount: 0,

  addMessage: (msg) =>
    set((state) => {
      const exists = state.messages.some(
        (m) => m.agentId === msg.agentId && m.status === 'pending'
      );
      if (exists) return state;
      const thread: ThreadMessage[] = [
        { role: 'supervisor', content: msg.question, timestamp: msg.timestamp },
      ];
      const updated = [...state.messages, { ...msg, thread }];
      return {
        messages: updated,
        pendingCount: updated.filter((m) => m.status === 'pending').length,
      };
    }),

  markAnswered: (agentId) =>
    set((state) => {
      const updated = state.messages.map((m) =>
        m.agentId === agentId ? { ...m, status: 'answered' as const } : m
      );
      return {
        messages: updated,
        pendingCount: updated.filter((m) => m.status === 'pending').length,
      };
    }),

  markTimedOut: (agentId) =>
    set((state) => {
      const updated = state.messages.map((m) =>
        m.agentId === agentId ? { ...m, status: 'timed_out' as const } : m
      );
      return {
        messages: updated,
        pendingCount: updated.filter((m) => m.status === 'pending').length,
      };
    }),

  addThreadMessage: (agentId, msg) =>
    set((state) => ({
      messages: state.messages.map((m) =>
        m.agentId === agentId ? { ...m, thread: [...m.thread, msg] } : m
      ),
    })),

  getByAgentId: (agentId) =>
    get().messages.find((m) => m.agentId === agentId),
}));

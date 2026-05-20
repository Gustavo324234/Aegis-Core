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
  /** First still-pending question across all agents — drives the chat modal. */
  firstPending: () => AgentMessage | undefined;
}

export const useAgentInboxStore = create<AgentInboxState>((set, get) => ({
  messages: [],
  pendingCount: 0,

  addMessage: (msg) =>
    set((state) => {
      // CORE-FIX: dedup by (agentId + question), NOT just agentId. The old
      // logic dropped a supervisor's SECOND question whenever the first was
      // still marked pending — which is exactly what blocked the user when a
      // supervisor asked sequentially (Q1 "¿puedo acceder?", then Q2 "¿cuál
      // es el repo?"). A supervisor only ever has one question in flight at a
      // time (it blocks on ask_user), so distinct question text = new prompt.
      const duplicate = state.messages.some(
        (m) =>
          m.agentId === msg.agentId &&
          m.question === msg.question &&
          m.status === 'pending'
      );
      if (duplicate) return state;
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
      // Only flip the OLDEST pending question for this agent — leaves any
      // newer queued question intact so the modal can advance to it.
      let flipped = false;
      const updated = state.messages.map((m) => {
        if (!flipped && m.agentId === agentId && m.status === 'pending') {
          flipped = true;
          return { ...m, status: 'answered' as const };
        }
        return m;
      });
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

  firstPending: () => get().messages.find((m) => m.status === 'pending'),
}));

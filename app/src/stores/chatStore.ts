import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import { nanoid } from 'nanoid/non-secure';
import type { ChatMessage } from '@/types/chat';
import * as SecureStore from 'expo-secure-store';

const secureStorage = {
  getItem: (name: string) => SecureStore.getItemAsync(name),
  setItem: (name: string, value: string) => SecureStore.setItemAsync(name, value),
  removeItem: (name: string) => SecureStore.deleteItemAsync(name),
};

interface ChatState {
  messages: ChatMessage[];
  isStreaming: boolean;
  isProcessingTool: boolean;
  isConversationMode: boolean;
  error: string | null;

  addUserMessage: (content: string) => void;
  startAssistantMessage: () => void;
  appendToken: (token: string) => void;
  finalizeMessage: () => void;
  setProcessingTool: (status: boolean) => void;
  setConversationMode: (status: boolean) => void;
  setError: (error: string) => void;
  clearError: () => void;
  reset: () => void;
}

export const useChatStore = create<ChatState>()(
  persist(
    (set) => ({
      messages: [],
      isStreaming: false,
      isProcessingTool: false,
      isConversationMode: false,
      error: null,

      addUserMessage: (content) =>
        set((s) => ({
          messages: [...s.messages, {
            id: nanoid(),
            role: 'user',
            content,
            isStreaming: false,
            timestamp: Date.now(),
          }],
        })),

      startAssistantMessage: () =>
        set((s) => ({
          isStreaming: true,
          messages: [...s.messages, {
            id: nanoid(),
            role: 'assistant',
            content: '',
            isStreaming: true,
            timestamp: Date.now(),
          }],
        })),

      appendToken: (token) =>
        set((s) => {
          const msgs = [...s.messages];
          const last = msgs.findLastIndex((m) => m.role === 'assistant');
          if (last >= 0) {
            msgs[last] = { ...msgs[last], content: msgs[last].content + token };
          }
          return { messages: msgs };
        }),

      finalizeMessage: () =>
        set((s) => {
          const msgs = [...s.messages];
          const last = msgs.findLastIndex((m) => m.role === 'assistant');
          if (last >= 0) msgs[last] = { ...msgs[last], isStreaming: false };
          return { messages: msgs, isStreaming: false, isProcessingTool: false };
        }),

      setProcessingTool: (status) => set({ isProcessingTool: status }),

      setConversationMode: (status) => set({ isConversationMode: status }),

      setError: (error) =>
        set((s) => {
          const msgs = [...s.messages];
          const last = msgs.findLastIndex((m) => m.role === 'assistant' && m.isStreaming);
          if (last >= 0) msgs[last] = { ...msgs[last], content: `Error: ${error}`, isStreaming: false };
          return { messages: msgs, isStreaming: false, isProcessingTool: false, error };
        }),

      clearError: () => set({ error: null }),
      reset: () => set({ messages: [], isStreaming: false, isProcessingTool: false, error: null }),
    }),
    {
      name: 'ank-chat-memory',
      storage: createJSONStorage(() => secureStorage),
      partialize: (state) => ({ messages: state.messages.slice(-50) }),
    }
  )
);

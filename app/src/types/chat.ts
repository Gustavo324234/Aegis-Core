export type Role = 'user' | 'assistant';

export interface ChatMessage {
  id: string;
  role: Role;
  content: string;
  isStreaming: boolean;
  timestamp: number;
}

export type UiState<T> =
  | { status: 'idle' }
  | { status: 'loading' }
  | { status: 'success'; data: T }
  | { status: 'error'; message: string };

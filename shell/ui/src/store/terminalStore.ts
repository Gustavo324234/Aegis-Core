import { create } from 'zustand';

export type LineKind = 'Stdout' | 'Stderr' | 'System';

export interface TerminalLine {
    kind: LineKind;
    content: string;
    timestamp: string;
}

interface TerminalState {
    lines: TerminalLine[];
    isRunning: boolean;
    command: string;
    args: string[];

    appendLine: (line: TerminalLine) => void;
    setRunning: (val: boolean) => void;
    clear: () => void;
    setCommand: (cmd: string, args: string[]) => void;
    addSystemLine: (content: string) => void;
}

export const useTerminalStore = create<TerminalState>()((set) => ({
    lines: [],
    isRunning: false,
    command: '',
    args: [],

    appendLine: (line) =>
        set((state) => ({ lines: [...state.lines.slice(-500), line] })),

    setRunning: (val) => set({ isRunning: val }),

    clear: () => set({ lines: [] }),

    setCommand: (cmd, args) => set({ command: cmd, args }),

    addSystemLine: (content) =>
        set((state) => ({
            lines: [
                ...state.lines.slice(-500),
                { kind: 'System', content, timestamp: new Date().toISOString() },
            ],
        })),
}));

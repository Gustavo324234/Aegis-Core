import { create } from 'zustand';

export type PrStatus =
    | 'open'
    | 'ci_running'
    | 'ci_passed'
    | 'ci_failed'
    | 'auto_fix_in_progress'
    | 'merge_ready'
    | 'merged'
    | 'closed';

export type MergeMode = 'automatic' | 'manual';

export interface ManagedPr {
    pr_number: number;
    title: string;
    branch: string;
    base_branch: string;
    url: string;
    merge_mode: MergeMode;
    auto_fix_ci: boolean;
    auto_fix_attempts: number;
    status: PrStatus;
    created_at: string;
    updated_at: string;
}

interface PrState {
    prs: ManagedPr[];
    isLoading: boolean;
    error: string | null;

    setPrs: (prs: ManagedPr[]) => void;
    updatePr: (pr_number: number, patch: Partial<ManagedPr>) => void;
    setLoading: (val: boolean) => void;
    setError: (msg: string | null) => void;

    fetchPrs: (tenantId: string, sessionKey: string) => Promise<void>;
    patchPr: (
        tenantId: string,
        sessionKey: string,
        pr_number: number,
        patch: { merge_mode?: string; auto_fix_ci?: boolean; status?: string },
    ) => Promise<void>;
    mergeNow: (tenantId: string, sessionKey: string, pr_number: number) => Promise<void>;
}

const authHeaders = (tenantId: string, sessionKey: string) => ({
    'Content-Type': 'application/json',
    'x-citadel-tenant': tenantId,
    'x-citadel-key': sessionKey,
});

export const usePrStore = create<PrState>()((set, get) => ({
    prs: [],
    isLoading: false,
    error: null,

    setPrs: (prs) => set({ prs }),
    updatePr: (pr_number, patch) =>
        set((state) => ({
            prs: state.prs.map((p) =>
                p.pr_number === pr_number ? { ...p, ...patch } : p,
            ),
        })),
    setLoading: (val) => set({ isLoading: val }),
    setError: (msg) => set({ error: msg }),

    fetchPrs: async (tenantId, sessionKey) => {
        set({ isLoading: true, error: null });
        try {
            const res = await fetch('/api/prs', {
                headers: authHeaders(tenantId, sessionKey),
            });
            if (res.ok) {
                const data = await res.json() as ManagedPr[];
                set({ prs: data });
            } else {
                set({ error: 'Failed to fetch PRs' });
            }
        } catch (e) {
            set({ error: String(e) });
        } finally {
            set({ isLoading: false });
        }
    },

    patchPr: async (tenantId, sessionKey, pr_number, patch) => {
        try {
            const res = await fetch(`/api/prs/${pr_number}`, {
                method: 'PATCH',
                headers: authHeaders(tenantId, sessionKey),
                body: JSON.stringify(patch),
            });
            if (res.ok) {
                get().updatePr(pr_number, patch as Partial<ManagedPr>);
            }
        } catch (e) {
            set({ error: String(e) });
        }
    },

    mergeNow: async (tenantId, sessionKey, pr_number) => {
        try {
            const res = await fetch(`/api/prs/${pr_number}/merge`, {
                method: 'POST',
                headers: authHeaders(tenantId, sessionKey),
            });
            if (res.ok) {
                get().updatePr(pr_number, { status: 'merged' });
            } else {
                set({ error: 'Merge failed' });
            }
        } catch (e) {
            set({ error: String(e) });
        }
    },
}));

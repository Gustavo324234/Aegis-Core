import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TAB_SESSION_KEY, useAegisStore } from '../store/useAegisStore';
import {
    mapHistoricalMessages,
    normalizeAgentEvent,
    resolveAutoRetryDelaySecs,
} from '../store/shared';

describe('useAegisStore & shared helpers', () => {
    beforeEach(() => {
        sessionStorage.clear();
        vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 200 })));
        useAegisStore.getState().logout();
    });

    it('sets auth credentials in memory and TAB_SESSION_KEY without writing sessionKey to localStorage', () => {
        useAegisStore.getState().setAuth('alice', 'token-xyz');

        const state = useAegisStore.getState();
        expect(state.isAuthenticated).toBe(true);
        expect(state.tenantId).toBe('alice');
        expect(state.sessionKey).toBe('token-xyz');
        expect(sessionStorage.getItem(TAB_SESSION_KEY)).toBe('token-xyz');

        const persistedRaw = localStorage.getItem('aegis-storage');
        if (persistedRaw) {
            expect(persistedRaw).not.toContain('token-xyz');
        }
    });

    it('merges streamed assistant tokens by pid and purges sensitive state on logout', () => {
        const store = useAegisStore.getState();
        store.setAuth('alice', 'cst_test_token');
        store.addSystemMessage('Sistema iniciado');
        store.appendToken('pid-1', 'Hola ', 'text');
        store.appendToken('pid-1', 'mundo', 'text');

        const midState = useAegisStore.getState();
        expect(midState.messages).toHaveLength(2);
        expect(midState.messages[1].content).toBe('Hola mundo');

        midState.logout();

        const cleared = useAegisStore.getState();
        expect(cleared.isAuthenticated).toBe(false);
        expect(cleared.tenantId).toBeNull();
        expect(cleared.sessionKey).toBeNull();
        expect(cleared.messages).toEqual([]);
        expect(sessionStorage.getItem(TAB_SESSION_KEY)).toBeNull();
    });

    it('parses retry delays, agent stream events, and chat history rows', () => {
        expect(resolveAutoRetryDelaySecs('Rate limit: reintentá en 25s')).toBe(25);
        expect(resolveAutoRetryDelaySecs('Error', 120)).toBe(90);
        expect(resolveAutoRetryDelaySecs('Todo bien')).toBeUndefined();

        const evt = normalizeAgentEvent({
            type: 'spawned',
            agent_id: 'ag-1',
            parent_id: null,
            model: 'gemini-2.5-flash',
            role: { Specialist: { scope: 'frontend' } },
        });
        expect(evt).toEqual({
            type: 'Spawned',
            agent_id: 'ag-1',
            parent_id: null,
            model: 'gemini-2.5-flash',
            role: { type: 'Specialist', scope: 'frontend' },
        });

        const msgs = mapHistoricalMessages(
            [
                { role: 'USER', content: 'Hola', timestamp: '2026-11-22T12:00:00Z' },
                { role: 'assistant', content: 'Buenas!', timestamp: '2026-11-22T12:00:01Z' },
            ],
            10,
        );
        expect(msgs[0]).toMatchObject({ id: 'hist-10-2026-11-22T12:00:00Z', role: 'user', content: 'Hola' });
        expect(msgs[1]).toMatchObject({ id: 'hist-11-2026-11-22T12:00:01Z', role: 'assistant', content: 'Buenas!' });
    });
});

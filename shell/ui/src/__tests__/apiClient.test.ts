import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError, apiDelete, apiGet, apiPost, apiRequest } from '../utils/apiClient';
import { useAegisStore } from '../store/useAegisStore';

describe('apiClient', () => {
    const fetchMock = vi.fn();

    beforeEach(() => {
        useAegisStore.setState({ tenantId: 'tavo', sessionKey: 'secret-key' });
        fetchMock.mockReset();
        vi.stubGlobal('fetch', fetchMock);
    });

    afterEach(() => {
        vi.unstubAllGlobals();
    });

    it('injects Citadel headers from store and serializes JSON bodies', async () => {
        fetchMock.mockResolvedValueOnce(
            new Response(JSON.stringify({ ok: true }), {
                status: 200,
                headers: { 'Content-Type': 'application/json' },
            }),
        );

        const res = await apiPost<{ ok: boolean }>('/api/example', { hello: 'world' });
        expect(res).toEqual({ ok: true });

        expect(fetchMock).toHaveBeenCalledTimes(1);
        const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
        expect(url).toBe('/api/example');
        expect(init.method).toBe('POST');
        expect(init.headers).toMatchObject({
            'x-citadel-tenant': 'tavo',
            'x-citadel-key': 'secret-key',
            'Content-Type': 'application/json',
        });
        expect(init.body).toBe(JSON.stringify({ hello: 'world' }));
    });

    it('omits auth headers when auth is false and handles 204 responses', async () => {
        fetchMock.mockResolvedValueOnce(new Response(null, { status: 204 }));

        const result = await apiDelete('/api/public/item', { auth: false });
        expect(result).toBeUndefined();

        const [, init] = fetchMock.mock.calls[0] as [string, RequestInit];
        expect(init.headers).not.toHaveProperty('x-citadel-tenant');
        expect(init.headers).not.toHaveProperty('x-citadel-key');
    });

    it('invokes onUnauthorized and throws UNAUTHORIZED ApiError on 401', async () => {
        const onUnauthorized = vi.fn();
        fetchMock.mockResolvedValueOnce(
            new Response(JSON.stringify({ error: 'Token expired' }), {
                status: 401,
                headers: { 'Content-Type': 'application/json' },
            }),
        );

        await expect(apiGet('/api/protected', { onUnauthorized })).rejects.toMatchObject({
            name: 'ApiError',
            status: 401,
            code: 'UNAUTHORIZED',
            message: 'Token expired',
        });
        expect(onUnauthorized).toHaveBeenCalledTimes(1);
    });

    it('throws FORBIDDEN ApiError on 403 and parses Retry-After on 429', async () => {
        fetchMock.mockResolvedValueOnce(new Response('Denied', { status: 403 }));
        await expect(apiGet('/api/admin')).rejects.toMatchObject({
            status: 403,
            code: 'FORBIDDEN',
        });

        fetchMock.mockResolvedValueOnce(
            new Response(JSON.stringify({ detail: 'Slow down' }), {
                status: 429,
                headers: { 'Content-Type': 'application/json', 'Retry-After': '42' },
            }),
        );
        await expect(apiGet('/api/rate-limited')).rejects.toMatchObject({
            status: 429,
            code: 'RATE_LIMITED',
            retryAfterSeconds: 42,
            message: 'Slow down',
        });
    });

    it('throws ABORTED ApiError when request is cancelled via AbortController', async () => {
        const controller = new AbortController();
        fetchMock.mockImplementationOnce(
            (_url: string, init?: RequestInit) =>
                new Promise((_resolve, reject) => {
                    init?.signal?.addEventListener('abort', () => {
                        reject(new DOMException('Aborted', 'AbortError'));
                    });
                }),
        );

        const promise = apiRequest('/api/slow', { signal: controller.signal });
        controller.abort();

        await expect(promise).rejects.toBeInstanceOf(ApiError);
        await expect(promise).rejects.toMatchObject({ code: 'ABORTED', status: 0 });
    });
});

import { useAegisStore } from '../store/useAegisStore';

export type ApiErrorCode =
    | 'UNAUTHORIZED'
    | 'FORBIDDEN'
    | 'RATE_LIMITED'
    | 'HTTP_ERROR'
    | 'NETWORK_ERROR'
    | 'ABORTED';

export interface ApiErrorOptions {
    retryAfterSeconds?: number;
    details?: unknown;
}

export class ApiError extends Error {
    readonly status: number;
    readonly code: ApiErrorCode;
    readonly retryAfterSeconds?: number;
    readonly details?: unknown;

    constructor(
        status: number,
        code: ApiErrorCode,
        message: string,
        options?: ApiErrorOptions,
    ) {
        super(message);
        this.name = 'ApiError';
        this.status = status;
        this.code = code;
        this.retryAfterSeconds = options?.retryAfterSeconds;
        this.details = options?.details;
        Object.setPrototypeOf(this, ApiError.prototype);
    }
}

export interface ApiAuthCredentials {
    tenantId?: string | null;
    sessionKey?: string | null;
}

export interface ApiRequestOptions {
    method?: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
    headers?: Record<string, string>;
    body?: unknown;
    signal?: AbortSignal;
    timeoutMs?: number;
    auth?: ApiAuthCredentials | false;
    onUnauthorized?: () => void;
}

export function createAbortableController(): AbortController {
    return new AbortController();
}

function isRawBodyInit(value: unknown): value is BodyInit {
    if (typeof value === 'string') return true;
    if (typeof FormData !== 'undefined' && value instanceof FormData) return true;
    if (typeof Blob !== 'undefined' && value instanceof Blob) return true;
    if (typeof URLSearchParams !== 'undefined' && value instanceof URLSearchParams) return true;
    if (typeof ArrayBuffer !== 'undefined' && (value instanceof ArrayBuffer || ArrayBuffer.isView(value))) {
        return true;
    }
    if (typeof ReadableStream !== 'undefined' && value instanceof ReadableStream) return true;
    return false;
}

function hasHeaderCaseInsensitive(headers: Record<string, string>, target: string): boolean {
    const lowerTarget = target.toLowerCase();
    return Object.keys(headers).some((k) => k.toLowerCase() === lowerTarget);
}

function extractErrorMessage(payload: unknown, fallback: string): string {
    if (typeof payload === 'string') {
        const trimmed = payload.trim();
        return trimmed.length > 0 ? trimmed : fallback;
    }
    if (payload && typeof payload === 'object') {
        const record = payload as Record<string, unknown>;
        for (const key of ['error', 'message', 'detail', 'msg']) {
            const val = record[key];
            if (typeof val === 'string' && val.trim().length > 0) {
                return val.trim();
            }
        }
    }
    return fallback;
}

function parseRetryAfterHeader(raw: string | null): number | undefined {
    if (!raw) return undefined;
    const numeric = Number.parseInt(raw.trim(), 10);
    if (Number.isFinite(numeric) && numeric > 0) {
        return numeric;
    }
    return undefined;
}

async function parseResponsePayload(res: Response): Promise<unknown> {
    if (res.status === 204 || res.status === 205) {
        return undefined;
    }
    const contentLength = res.headers.get('content-length');
    if (contentLength === '0') {
        return undefined;
    }

    const contentType = res.headers.get('content-type')?.toLowerCase() ?? '';
    const text = await res.text();
    if (!text) {
        return undefined;
    }

    if (contentType.includes('application/json')) {
        try {
            return JSON.parse(text) as unknown;
        } catch {
            return text;
        }
    }

    const trimmed = text.trim();
    if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
        try {
            return JSON.parse(trimmed) as unknown;
        } catch {
            return text;
        }
    }

    return text;
}

export async function apiRequest<T = unknown>(
    url: string,
    options: ApiRequestOptions = {},
): Promise<T> {
    const method = options.method ?? 'GET';
    const headers: Record<string, string> = { ...(options.headers ?? {}) };

    if (options.auth !== false) {
        const storeState = useAegisStore.getState();
        const creds = options.auth ?? {
            tenantId: storeState.tenantId,
            sessionKey: storeState.sessionKey,
        };
        if (creds.tenantId && !hasHeaderCaseInsensitive(headers, 'x-citadel-tenant')) {
            headers['x-citadel-tenant'] = creds.tenantId;
        }
        if (creds.sessionKey && !hasHeaderCaseInsensitive(headers, 'x-citadel-key')) {
            headers['x-citadel-key'] = creds.sessionKey;
        }
    }

    let requestBody: BodyInit | undefined;
    if (options.body !== undefined) {
        if (isRawBodyInit(options.body)) {
            requestBody = options.body;
        } else {
            requestBody = JSON.stringify(options.body);
            if (!hasHeaderCaseInsensitive(headers, 'content-type')) {
                headers['Content-Type'] = 'application/json';
            }
        }
    }

    const controller = new AbortController();
    const externalSignal = options.signal;
    let timeoutId: ReturnType<typeof setTimeout> | undefined;

    const handleExternalAbort = () => {
        controller.abort(externalSignal?.reason);
    };

    if (externalSignal) {
        if (externalSignal.aborted) {
            controller.abort(externalSignal.reason);
        } else {
            externalSignal.addEventListener('abort', handleExternalAbort, { once: true });
        }
    }

    if (typeof options.timeoutMs === 'number' && options.timeoutMs > 0) {
        timeoutId = setTimeout(() => {
            controller.abort(new DOMException('Request timeout exceeded', 'TimeoutError'));
        }, options.timeoutMs);
    }

    try {
        let res: Response;
        try {
            res = await fetch(url, {
                method,
                headers,
                body: requestBody,
                signal: controller.signal,
            });
        } catch (err) {
            if (controller.signal.aborted || (err instanceof DOMException && err.name === 'AbortError')) {
                throw new ApiError(0, 'ABORTED', 'La solicitud fue cancelada o excedió el tiempo límite.');
            }
            throw new ApiError(
                0,
                'NETWORK_ERROR',
                err instanceof Error ? err.message : 'Error de red comunicando con el servidor.',
                { details: err },
            );
        }

        const payload = await parseResponsePayload(res);

        if (res.status === 401) {
            options.onUnauthorized?.();
            throw new ApiError(
                401,
                'UNAUTHORIZED',
                extractErrorMessage(payload, 'Sesión expirada o credenciales inválidas.'),
                { details: payload },
            );
        }

        if (res.status === 403) {
            throw new ApiError(
                403,
                'FORBIDDEN',
                extractErrorMessage(payload, 'No tienes permisos suficientes para esta operación.'),
                { details: payload },
            );
        }

        if (res.status === 429) {
            const retryAfterSeconds = parseRetryAfterHeader(res.headers.get('retry-after'));
            throw new ApiError(
                429,
                'RATE_LIMITED',
                extractErrorMessage(payload, 'Demasiadas solicitudes. Intenta nuevamente en unos segundos.'),
                { retryAfterSeconds, details: payload },
            );
        }

        if (!res.ok) {
            throw new ApiError(
                res.status,
                'HTTP_ERROR',
                extractErrorMessage(payload, `Error HTTP ${res.status}`),
                { details: payload },
            );
        }

        return payload as T;
    } finally {
        if (timeoutId !== undefined) {
            clearTimeout(timeoutId);
        }
        if (externalSignal) {
            externalSignal.removeEventListener('abort', handleExternalAbort);
        }
    }
}

export function apiGet<T = unknown>(
    url: string,
    options?: Omit<ApiRequestOptions, 'method' | 'body'>,
): Promise<T> {
    return apiRequest<T>(url, { ...options, method: 'GET' });
}

export function apiPost<T = unknown>(
    url: string,
    body?: unknown,
    options?: Omit<ApiRequestOptions, 'method' | 'body'>,
): Promise<T> {
    return apiRequest<T>(url, { ...options, method: 'POST', body });
}

export function apiPut<T = unknown>(
    url: string,
    body?: unknown,
    options?: Omit<ApiRequestOptions, 'method' | 'body'>,
): Promise<T> {
    return apiRequest<T>(url, { ...options, method: 'PUT', body });
}

export function apiDelete<T = unknown>(
    url: string,
    options?: Omit<ApiRequestOptions, 'method' | 'body'>,
): Promise<T> {
    return apiRequest<T>(url, { ...options, method: 'DELETE' });
}

import { TAB_SESSION_KEY, type AegisState, type StoreGet, type StoreSet } from '../types';

export type AdminSlice = Pick<AegisState, 'fetchTenants' | 'createTenant' | 'deleteTenant' | 'resetPassword'>;

export const createAdminSlice = (set: StoreSet, get: StoreGet): AdminSlice => ({
    fetchTenants: async () => {
        const { tenantId, sessionKey } = get();
        if (!tenantId || !sessionKey) return;
        set({ isFetchingTenants: true, tenantsError: null });
        try {
            const res = await fetch('/api/admin/tenants', {
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                }
            });
            if (res.ok) {
                const data = await res.json();
                const rawTenants = data.tenants || [];
                const ids = rawTenants
                    .map((t: string | { tenant_id: string }) => typeof t === 'string' ? t : t.tenant_id)
                    .filter((id: string) => !!id && id !== 'root');
                set({ tenants: ids, lastTenantsUpdate: new Date().toLocaleTimeString() });
            } else {
                const err = await res.json().catch(() => ({ detail: 'Servidor no respondió con JSON válido' }));
                set({ tenantsError: err.detail || 'Error al listar enclaves' });
            }
        } catch (e) {
            console.error('🛡️ Citadel Fetch Error:', e);
            set({ tenantsError: 'Fallo de conexión (Network Error)' });
        } finally {
            set({ isFetchingTenants: false });
        }
    },
    createTenant: async (targetUsername: string) => {
        const { tenantId, sessionKey } = get();
        if (!tenantId || !sessionKey) return { success: false, message: 'No admin session' };
        try {
            const res = await fetch('/api/admin/tenant', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({ username: targetUsername })
            });
            const contentType = res.headers.get('content-type') || '';
            const data = contentType.includes('application/json')
                ? await res.json()
                : { detail: await res.text() };
            if (res.ok) {
                get().fetchTenants();
                return { success: true, temporary_passphrase: data.temporary_passphrase };
            } else {
                let errMsg = data.detail || data.error || 'Error desconocido al crear Tenant';
                if (typeof errMsg === 'string' && (errMsg.includes('already exists') || errMsg.includes('Duplicate')))
                    errMsg = `El Tenant "${targetUsername}" ya existe en el Ring 0.`;
                return { success: false, message: String(errMsg) };
            }
        } catch (e) {
            console.error('Create tenant error:', e);
            return { success: false, message: 'Fallo crítico en la comunicación con el Citadel' };
        }
    },

    deleteTenant: async (targetId: string) => {
        const { tenantId, sessionKey } = get();
        if (!tenantId || !sessionKey) return false;
        try {
            const res = await fetch(`/api/admin/tenant/${encodeURIComponent(targetId)}`, {
                method: 'DELETE',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                }
            });
            if (res.ok) {
                set({ messages: [] });
                get().fetchTenants();
                return true;
            }
            return false;
        } catch (e) { console.error('Delete tenant error:', e); return false; }
    },

    resetPassword: async (targetId: string, newPass: string) => {
        const { tenantId, sessionKey } = get();
        if (!tenantId || !sessionKey) return false;
        try {
            const res = await fetch('/api/admin/reset_password', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({ tenant_id: targetId, new_passphrase: newPass })
            });
            if (res.ok) {
                if (tenantId === targetId) {
                    try { sessionStorage.setItem(TAB_SESSION_KEY, newPass); } catch { /* ignore */ }
                    set({ sessionKey: newPass });
                }
                return true;
            }
            return false;
        } catch (e) { console.error('Reset password error:', e); return false; }
    },
});

# CORE-359: Sandbox Tenant `project_root`, Add Default Workspace Fallback, and Prune RateLimiter/Onboarding State

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Asignado a:** Kernel Engineer

## Description
1. In `routes/workspace.rs` and `routes/fs.rs`, non-master tenants could configure `project_root` to any arbitrary directory on the host OS or another tenant's workspace and read/browse its files via `/api/fs/tree` and `/api/fs/file`. Furthermore, if `project_root` was not explicitly set in `TenantDB`, `/api/fs/tree` and `/api/fs/file` returned `500 Internal Server Error ("project_root not configured")` instead of defaulting to `<data_dir>/users/<tenant_id>/workspace`.
2. `ONBOARDING_SETTLED_TENANTS` in `ws/chat.rs` was never cleared when a tenant was deleted in `routes/admin.rs`, and `AuthRateLimiter::store` in `rate_limiter.rs` never evicted expired windows when `store.len()` grew large.

## Acceptance Criteria
- [x] Restrict non-master tenants from setting or reading a `project_root` outside `<data_dir>/users/<tenant_id>/workspace`, and default to `<data_dir>/users/<tenant_id>/workspace` when `project_root` is unset.
- [x] Clear `ONBOARDING_SETTLED_TENANTS` entry when deleting a tenant in `routes/admin.rs`.
- [x] Prune expired entries in `AuthRateLimiter` when `store.len()` exceeds capacity threshold.

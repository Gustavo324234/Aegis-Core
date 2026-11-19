# CORE-358: Enforce `CitadelAuthenticated` Verification and Multi-Tenant Agent/Persona Isolation

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Asignado a:** Kernel Engineer

## Description
1. In `ank-http/src/citadel.rs`, `CitadelAuthenticated::from_request_parts` called `citadel.enclave.authenticate_tenant(...).await.map_err(...)?;` and ignored the `bool` return value (`Ok(false)` on wrong password or unknown tenant), allowing invalid credentials to pass `CitadelAuthenticated`. Additionally, `CitadelAuthenticated`, `ws/chat.rs`, and `ws/siren.rs` did not check `authenticate_master` before `authenticate_tenant`.
2. `routes/agents.rs` (`list_projects`, `get_agent_tree`, `get_agent`) used `CitadelCredentials` (unvalidated headers) instead of `CitadelAuthenticated`, and both `routes/agents.rs` and `ws/agents.rs` called `tree_snapshot()` (returning all tenants' agents) instead of `tree_snapshot_for_tenant(&tenant_id)`. `reply_to_agent` also did not verify tenant ownership of `agent_id`, and `routes/persona_api.rs` opened `TenantDB` from raw headers without `CitadelAuthenticated`.

## Acceptance Criteria
- [x] Enforce `authenticate_master || authenticate_tenant == Ok(true)` in `CitadelAuthenticated` (exposing `pub is_master: bool`), `ws/chat.rs`, and `ws/siren.rs`, rejecting `Ok(false)` with `CitadelError::Unauthorized`.
- [x] Require `CitadelAuthenticated` across `routes/agents.rs` and `routes/persona_api.rs`.
- [x] Filter agent snapshots by tenant via `tree_snapshot_for_tenant` in `routes/agents.rs` and `ws/agents.rs`, and verify tenant ownership in `reply_to_agent`.

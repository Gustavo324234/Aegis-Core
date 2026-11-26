# CORE-371: Harden Bootstrap Setup & Cognitive Engine Auth, Purge Tenant Keys on Delete, and Clean Tracked Test Databases

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Asignado a:** Kernel Engineer + Shell Engineer

## Description
1. In `ank-http/src/routes/auth.rs`, `POST /api/auth/setup` remained publicly mounted and invoked `initialize_master` without verifying `setup_token`, allowing unauthenticated Ring-0 takeover on uninitialized instances. Additionally, `setup`, `setup_token`, and `change_password` bypassed `auth_rate_limiter` (enabling unlimited brute-forcing of `setup_token` and tenant passwords) and lacked minimum passphrase length checks.
2. In `ank-http/src/routes/engine.rs`, `POST /api/engine/configure` allowed any non-admin tenant (`!auth.is_master`) to overwrite global `state.hal` cloud credentials and `engine_config.json` without SSRF validation (and clobbered the global HAL API key with `""` during tenant onboarding when linking to the global engine), while unauthenticated `GET /api/engine/status` leaked internal `api_url`, `provider`, and `model_name`.
3. Deleting a tenant (`admin.rs` / `server.rs`) left orphaned tenant API keys inside `router_keys.db` (`KeyPool`), and 16 test/dev SQLite `.db` files were tracked in Git (`test_dev_data/*.db` and `kernel/crates/ank-core/users/test_maxlen_user_*/memory.db`) due to open Windows SQLite handles during test cleanup.

## Acceptance Criteria
- [x] Enforce `auth_rate_limiter`, `admin_exists` pre-checks, minimum 8-char passphrases, and mandatory `setup_token` validation/consumption across `/api/auth/setup`, `/api/auth/setup-token`, and `/api/auth/change_password`.
- [x] Restrict global `state.hal` / `engine_config.json` mutation in `POST /api/engine/configure` to `auth.is_master` with URL/SSRF validation, isolate non-admin tenant keys into `KeyPool` (`add_tenant_key`) without clobbering global credentials, redact unauthenticated `GET /api/engine/status` metadata, and pass Citadel headers from `App.tsx` and `EngineSetupWizard.tsx`.
- [x] Add `KeyPool::purge_tenant_keys` / `CognitiveRouter::purge_tenant_keys` and invoke on tenant deletion, scope `TenantDB` drops before directory removal in `enclave/mod.rs` tests, update `.gitignore`, and untrack the 16 residual `.db` files from Git.

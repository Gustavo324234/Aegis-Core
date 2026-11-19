# CORE-370: Harden ank-server gRPC: Mandatory Setup Token & Hashing, Process Auth/IDOR Guard, Loopback Default Bind, Live Telemetry, and Full Tenant Purge

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Asignado a:** Kernel Engineer

## Description
1. `ank-server` bound its unencrypted (`h2c`) gRPC listener to `0.0.0.0:50051` by default (`main.rs`), exposing `AnkRpcServer` (`server.rs`) across the network.
2. `initialize_master_admin` skipped `setup_token` validation whenever `req.setup_token` was empty (`if !req.setup_token.is_empty()`), allowing remote unauthenticated takeover of Ring-0 Master Admin on fresh instances, and passed `req.passphrase` unhashed to `initialize_master` (likewise `reset_tenant_password` passed `req.new_passphrase` unhashed), breaking Citadel login after gRPC setup or password reset.
3. `list_processes` extracted `CitadelAuth` metadata without calling `validate_auth`, allowing any unauthenticated caller to enumerate any tenant's active PIDs and process names. `watch_task` neither verified that `req.pid` belonged to `auth.tenant_id` (cross-tenant stream IDOR) nor checked whether `pid` existed before inserting a `1024`-slot broadcast channel into `event_broker` (unbounded memory DoS). Additionally, `create_tenant` omitted `tenant_id` in `TenantCreateResponse`, `delete_tenant` failed to purge tenant `KeyPool` entries or invalidate onboarding/sticky caches, and `get_system_status` returned hardcoded zeros.

## Acceptance Criteria
- [x] Default `ank-server` gRPC bind address to `127.0.0.1` unless explicitly overridden via `AEGIS_GRPC_BIND`.
- [x] Require a non-empty valid `setup_token` and minimum 8-char hashed passphrase in `initialize_master_admin`, hash `new_passphrase` in `reset_tenant_password`, and populate `tenant_id` in `create_tenant`.
- [x] Enforce `validate_auth` in `list_processes`, verify scheduler PID existence and tenant ownership in `watch_task` without leaking `event_broker` channels on unknown PIDs, populate live telemetry in `get_system_status` for authenticated/loopback callers, and purge tenant router keys + caches in `delete_tenant`.

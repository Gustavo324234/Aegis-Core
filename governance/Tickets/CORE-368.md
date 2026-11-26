# CORE-368: Harden ank-cli & WS/System Endpoints: Citadel gRPC Headers, Authenticated Agent/Train WS Handshakes, and Connection Info Guard

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Asignado a:** Kernel Engineer

## Description
1. `ank-cli` (`CitadelInterceptor`) injected `x-aegis-tenant-id` and `x-aegis-session-key`, whereas `ank-server::server::auth_interceptor` validates `x-citadel-tenant` and `x-citadel-key`, causing every authenticated gRPC CLI subcommand (`ps`, `run`, `admin create-tenant`) to fail with `Unauthenticated`. Additionally, `aegis version` queried `/health` (which lacks `version` instead of `/api/system/sync_version`), `aegis token` hardcoded port `8000`, `aegis update` ignored `--beta` on Windows and used a non-existent `latest` release tag instead of resolving `v*` stable tags, `aegis keygen` overwrote existing keys without `--force` and lacked `0600` Unix permissions, `create_client` had no `connect_timeout`, and `aegis run` did not flush streamed tokens to `stdout`.
2. In `ank-http`, `/ws/agents/{tenant_id}` (`ws/agents.rs`) neither echoed the `Sec-WebSocket-Protocol: session-key.<key>` header (causing browser WebSocket handshakes to fail continuously) nor verified Citadel credentials (exposing tenant agent trees to unauthenticated raw WS clients). Similarly, `/api/train/progress` (`routes/train.rs`) upgraded WebSockets without Citadel authentication and allowed cross-tenant training cancellation/progress observation, while `/api/system/connection-info` (`routes/status.rs`) spawned `cloudflared --version` on every request, hardcoded port `8000`, and exposed LAN IP + public tunnel URLs to unauthenticated remote callers.

## Acceptance Criteria
- [x] Align `CitadelInterceptor` gRPC metadata keys to `x-citadel-tenant` / `x-citadel-key`, add channel `connect_timeout`, query `/api/system/sync_version` in `aegis version`, use `get_http_port()` in `aegis token`, resolve real `v*` / `nightly` tags on Windows & Linux in `aegis update`, enforce `--force` + `0600` permissions in `aegis keygen`, and flush `stdout` in `aegis run`.
- [x] Authenticate `/ws/agents/{tenant_id}` and `/api/train/progress/{tenant_id}` against Citadel Enclave while echoing `session-key.<key>` subprotocols (and fixing Axum 0.7 `/:tenant_id` / `/:provider` route params), isolate active training ownership/cancellation per tenant in `TrainingManager` + `routes/train.rs`, cache `cloudflared` detection in `OnceLock<bool>`, use `state.config.port`, and restrict `/api/system/connection-info` to authenticated Citadel callers or direct non-proxied loopback CLI probes.

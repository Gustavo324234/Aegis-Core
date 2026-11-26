# CORE-366: Harden aegis-connect-relay & TunnelClient: Token Binding, Reconnect Race Fix, Bounded Queues, Query Preservation, and WSS TLS

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Asignado a:** Kernel Engineer

## Description
1. `aegis-connect-relay/src/main.rs` accepted any arbitrary string starting with `"orion_id_tok_live_aegis_"` of length ≥ 30 for any `x-citadel-tenant` slug (allowing trivial tenant tunnel hijacking), suffered a reconnect race where an older closing WebSocket removed a newer live tunnel via `tunnels.remove(&tenant)`, used unbounded channels without body caps, dropped HTTP query strings in `proxy_request`, and left in-flight browser requests hanging until the 30s timeout when a tunnel dropped.
2. `TunnelClient` (`ank-core/src/tunnel/client.rs`) hardcoded `ws://` even when configured with a `wss://` or `https://` relay URL (breaking TLS handshakes and exposing bearer tokens in cleartext), leaked `token=` in URL query logs, spawned unbounded local tasks without origin validation on `path`, and used fixed reconnect delays.

## Acceptance Criteria
- [x] Add HMAC-SHA256 (`AEGIS_RELAY_SECRET`) / allowlist (`AEGIS_RELAY_TOKENS_JSON`) verification and per-tenant token fingerprint binding in `aegis-connect-relay`, track `connection_id` so stale disconnects never evict newer tunnels, bound channels/payloads (16 MiB), preserve HTTP query strings, fail-fast pending requests on tunnel disconnect, and add `/health`.
- [x] Support `wss://` TLS connections in `TunnelClient`, remove `token` from query strings, enforce strict same-origin local URL resolution with `redirect::Policy::none()` and bounded concurrency (`Semaphore`), and apply exponential reconnect backoff.

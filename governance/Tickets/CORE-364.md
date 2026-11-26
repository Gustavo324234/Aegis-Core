# CORE-364: Harden ank-mcp Client Protocol Handshake, Long-Lived SSE Timeouts, Safe Env Isolation, and Server Bootstrap

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Asignado a:** Kernel Engineer

## Description
1. `McpClientSession` (`ank-mcp/src/client.rs`) lacked JSON-RPC notification support (`notify`), the mandatory MCP `initialize` + `notifications/initialized` handshake, and liveness tracking (`is_closed`) when a transport disconnects.
2. `SseTransport` (`ank-mcp/src/sse.rs`) applied `ClientBuilder::timeout` globally to the `reqwest::Client`, causing long-lived `GET /sse` streams to time out and kill the session after `timeout` seconds, and lacked SSE buffer bounds and multi-line `data:` event assembly.
3. `StdioTransport` (`ank-mcp/src/stdio.rs`) stripped essential OS runtime variables (`SystemRoot`, `PATH`, `COMSPEC`, `TEMP`) via `env_clear()`, breaking Winsock/Node/Python MCP subprocess startup on Windows, and read unbounded stdout lines. Additionally, `ank-server` never bootstrapped MCP servers from `{data_dir}/mcp_servers.json`.

## Acceptance Criteria
- [x] Add `notify`, `initialize` handshake, and `is_closed` tracking to `McpClientSession`, and validate `required` input schema properties plus dead-session detection in `McpToolRegistry` / `McpToolDispatcher`.
- [x] Separate connect/POST timeouts from the long-lived SSE stream in `SseTransport`, support multi-line `data:` events, and cap line/event buffers in both `SseTransport` and `StdioTransport`.
- [x] Preserve non-secret OS runtime environment variables and `kill_on_drop(true)` in `StdioTransport`, and bootstrap configured MCP servers from `{data_dir}/mcp_servers.json` at `ank-server` startup.

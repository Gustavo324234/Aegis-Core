# CORE-004 — ank-mcp: cliente MCP

**Épica:** 32 — Unified Binary
**Fase:** 1 — Fundación Rust
**Repo:** Aegis-Core — `kernel/crates/ank-mcp/`
**Asignado a:** Kernel Engineer
**Prioridad:** 🟡 Media
**Estado:** CLOSED
**Depende de:** CORE-002

---

## Contexto

`ank-mcp` implementa el cliente MCP (Model Context Protocol) con dos transportes:
StdIO (subprocesos) y SSE (servidores remotos). Lo usa `ank-core` para tool discovery.

**Referencia legacy:** `Aegis-ANK/crates/ank-mcp/src/`

---

## Trabajo requerido

Portar `Aegis-ANK/crates/ank-mcp/src/` a `kernel/crates/ank-mcp/src/` sin cambios.

### `kernel/crates/ank-mcp/Cargo.toml`

```toml
[package]
name = "ank-mcp"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio.workspace      = true
serde.workspace      = true
serde_json.workspace = true
anyhow.workspace     = true
thiserror.workspace  = true
tracing.workspace    = true
async-trait.workspace = true
futures.workspace    = true
reqwest.workspace    = true
bytes.workspace      = true
uuid.workspace       = true
```

---

## Criterios de aceptación

- [ ] `cargo build -p ank-mcp` compila sin errores
- [ ] `McpTransport` trait existe y es público
- [ ] `StdioTransport` y `SseTransport` implementan `McpTransport`
- [ ] `cargo clippy -p ank-mcp -- -D warnings -D clippy::unwrap_used` → 0 warnings

## Referencia

`Aegis-ANK/crates/ank-mcp/src/` — fuente completa

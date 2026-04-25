# kernel/

Aegis Neural Kernel — Rust/Tokio

El motor cognitivo del sistema. Implementación nueva basada en el legacy
`Aegis-ANK` como referencia, con la arquitectura final: un único proceso
que sirve HTTP/WS (Axum :8000) y gRPC (Tonic :50051) simultáneamente.

## Crates

| Crate | Descripción |
|---|---|
| `ank-core` | Motor cognitivo: scheduler, DAG, VCM, Citadel, HAL, AgentOrchestrator, TerminalExecutor, GitHubBridge, PrManager |
| `ank-http` | Servidor HTTP/WS (Axum): REST API, WebSocket streaming, SPA embebida |
| `ank-proto` | Contratos Protobuf + stubs Rust generados |
| `ank-server` | Entrypoint: levanta Axum + Tonic en un proceso único |
| `ank-cli` | CLI administrativa vía gRPC |
| `ank-mcp` | Cliente MCP (StdIO + SSE) |
| `aegis-supervisor` | Process manager: start/stop/health |
| `aegis-sdk` | SDK Wasm para autores de plugins |

## Referencia legacy

`Aegis-ANK` — leer para entender la lógica existente, no modificar.

## Build

```bash
cargo build --release -p ank-server
```

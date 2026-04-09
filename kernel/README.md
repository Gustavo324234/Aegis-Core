# kernel/

Aegis Neural Kernel — Rust/Tokio

El motor cognitivo del sistema. Implementación nueva basada en el legacy
`Aegis-ANK` como referencia, con la arquitectura final: un único proceso
que sirve HTTP/WS (Axum :8000) y gRPC (Tonic :50051) simultáneamente.

## Crates

| Crate | Descripción | Fuente |
|---|---|---|
| `ank-core` | Motor cognitivo: scheduler, DAG, VCM, Citadel, HAL | Migrar desde Aegis-ANK |
| `ank-http` | Servidor HTTP/WS (Axum) | **NUEVO — Epic 32** |
| `ank-proto` | Contratos Protobuf + stubs Rust generados | Migrar desde Aegis-ANK |
| `ank-server` | Entrypoint: levanta Axum + Tonic en un proceso | Reescribir sobre legacy |
| `ank-cli` | CLI administrativa vía gRPC | Migrar desde Aegis-ANK |
| `ank-mcp` | Cliente MCP (StdIO + SSE) | Migrar desde Aegis-ANK |
| `aegis-supervisor` | Process manager: start/stop/health | Migrar desde Aegis-ANK |
| `aegis-sdk` | SDK Wasm para autores de plugins | Migrar desde Aegis-ANK |

## Referencia legacy

`Aegis-ANK` — leer para entender la lógica existente, no modificar.

## Build

```bash
cargo build --release -p ank-server
```

# CORE-353 — gRPC Channel Pool for Microkernel Domain Modules

**Tipo:** perf
**Prioridad:** Alta
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

Tanto `SyscallExecutor::execute_external_module_tool` (`ank-core`) como `execute_module_tool` (`ank-http`) abrían una nueva conexión TCP/HTTP2 desde cero (`Endpoint::from_shared(...).connect().await`) en cada invocación a una herramienta de un módulo del Microkernel, duplicando lógica y añadiendo latencia de handshake en cada llamada.

## Solución propuesta

1. Implementar un pool compartido de canales gRPC (`call_module_tool_pooled`) en `ank_core::router::modules` respaldado por `LazyLock<RwLock<HashMap<String, tonic::transport::Channel>>>` con keep-alive HTTP/2.
2. Evictar y reconectar automáticamente el canal si una llamada falla por error de transporte (`Unavailable` / `Unknown`).
3. Unificar `ank-core/src/syscalls/mod.rs` y `ank-http/src/routes/router_api.rs` para utilizar el pool compartido.

## Criterios de aceptación

- [x] Pool de canales gRPC implementado en `ank_core::router::modules::call_module_tool_pooled` con reconexión automática ante caída del módulo.
- [x] `SyscallExecutor` y `router_api` reutilizan el mismo pool sin duplicar código de transporte.
- [x] Pruebas unitarias validando normalización de endpoints y manejo de errores de conexión.

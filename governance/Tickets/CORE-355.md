# CORE-355 — Modularización de `syscalls/mod.rs` en `parser.rs` y `oauth_integrations.rs`

- **Tipo:** refactor
- **Estado:** ✅ Done
- **Prioridad:** Media
- **Asignado a:** Kernel Engineer

## Contexto y Problema
El archivo `kernel/crates/ank-core/src/syscalls/mod.rs` alcanzó 1,858 líneas combinando en un único archivo la definición de `Syscall`, el ejecutor `SyscallExecutor`, las integraciones HTTP OAuth (Spotify, YouTube, Google Calendar, Google Drive, Gmail), los 15 patrones Regex de parsing legacy/JSON y el `StreamInterceptor`.

## Solución Implementada
1. Extraer toda la lógica de parsing de syscalls y el interceptor de streaming (`init_syscall_regexes`, `parse_task_type`, `parse_syscall`, `StreamItem`, `StreamInterceptor`) a `kernel/crates/ank-core/src/syscalls/parser.rs`.
2. Extraer las integraciones HTTP OAuth de Google y Spotify (`search_spotify`, `search_youtube_oauth`, `google_calendar`, `google_drive`, `gmail`) a `kernel/crates/ank-core/src/syscalls/oauth_integrations.rs`.
3. Re-exportar públicamente los símbolos desde `syscalls/mod.rs` manteniendo compatibilidad 100% transparente con el resto del workspace.

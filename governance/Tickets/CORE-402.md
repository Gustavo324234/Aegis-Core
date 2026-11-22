# CORE-402: Tokens de Sesión Citadel, Argon2id en `spawn_blocking` y Pool LRU de `TenantDB` (ADR-034)

- **Tipo:** perf
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 61
- **Asignado a:** Kernel Engineer

---

## Description

Eliminar los cuellos de botella de autenticación y apertura de SQLCipher detectados en la auditoría:
- Emitir tokens de sesión efímeros firmados/opacos tras `/api/auth/login` con TTL y revocación (evitando re-ejecutar Argon2id bajo mutex global y enviar la contraseña en claro en cada request/WS).
- Ejecutar las verificaciones Argon2id en `tokio::task::spawn_blocking` sin retener el mutex de SQLite ni exponer oráculo de timing en usuarios inexistentes.
- Implementar un pool LRU de conexiones `TenantDB` por `(tenant_id, key_hash)` ejecutado vía `spawn_blocking` (cumpliendo ADR-034) y ejecutando `init_schema()` solo una vez por inicialización.

## Acceptance Criteria

- [x] Los requests autenticados no ejecutan Argon2id ni `TenantDB::open` síncrono en el runtime async de Tokio.
- [x] Latencia por request autenticado reducida y sin bloqueo del executor Tokio bajo concurrencia.
- [x] Pruebas de carga/concurrencia y revocación de sesión pasando.

## Implementation & Verification

- **Argon2id no bloqueante (`enclave/master.rs`):** `hash_password_async` y `verify_password_async` sobre `tokio::task::spawn_blocking`; el mutex de SQLite solo se retiene durante el `SELECT`/`INSERT`/`UPDATE` y nunca durante el hashing/verificación; uso de `DUMMY_ARGON2_HASH` para mitigar oráculos de timing en usuarios inexistentes; `evict_tenant` previo a purgar archivos en `delete_tenant`.
- **Pool LRU SQLCipher (`enclave/mod.rs`):** `TENANT_DB_POOL` en memoria por `(db_path, sha256(key))` (capacidad por defecto `64` / `AEGIS_TENANT_DB_POOL_SIZE`, TTL idle 15 min), `initialized_schemas` por ruta, generaciones + `TenantDB::evict_tenant` en rotaciones/cuarentenas/borrado, y helper `TenantDB::with_async`.
- **Tokens efímeros Citadel (`ank-http/src/citadel.rs`, `state.rs`, `auth.rs`, `admin.rs`, `routes/*.rs`, `ws/*.rs`, UI):** `CitadelSessionStore` (`cst_*`, TTL 12h / `AEGIS_SESSION_TTL_SECS`) emitido en `/api/auth/login` y `/api/auth/quick-personal-setup`, revocable vía `/api/auth/logout`, `/api/auth/change_password`, `/api/admin/reset_password` y borrado de tenant; validación O(1) en `CitadelAuthenticated`, helpers compartidos `authenticate_headers` / `authenticate_master_headers` y WebSockets; clonado temprano de `MasterEnclave` liberando `state.citadel` antes de cualquier `.await`.
- **Handlers HTTP/WS sin bloqueo (`ank-http`):** migrados todos los `TenantDB::open` en rutas async y `ws/chat.rs` a `TenantDB::with_async` o `spawn_blocking`; UI (`useAegisStore.ts`, `PersonalQuickSetup.tsx`) adopta el token efímero y llama a `/api/auth/logout`.
- **Verificación:** `cargo fmt --all -- --check`, `cargo clippy --workspace` (CI flags), `cargo test -p ank-core --lib enclave` (`21 passed`), `cargo test -p ank-http` (`44 passed`), `npm run lint` (`0` warnings) y `npm run build` (`exit 0`).

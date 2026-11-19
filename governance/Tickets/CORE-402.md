# CORE-402: Tokens de Sesión Citadel, Argon2id en `spawn_blocking` y Pool LRU de `TenantDB` (ADR-034)

- **Tipo:** perf
- **Estado:** 📥 Todo
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

- [ ] Los requests autenticados no ejecutan Argon2id ni `TenantDB::open` síncrono en el runtime async de Tokio.
- [ ] Latencia por request autenticado reducida y sin bloqueo del executor Tokio bajo concurrencia.
- [ ] Pruebas de carga/concurrencia y revocación de sesión pasando.

# CORE-091 — Rate Limiting en Autenticación

**Epic:** 35 — Hardening Post-Launch  
**Área:** `kernel/crates/ank-http/src/routes/auth.rs`  
**Agente:** Kernel Engineer  
**Prioridad:** P1 — Seguridad  
**Estado:** TODO  
**Origen:** REC-003 / big-pickle DEBT-004

---

## Contexto

El endpoint `/api/auth/login` y el handshake WebSocket no tienen ningún mecanismo
de rate limiting. Un atacante puede realizar intentos de fuerza bruta contra
credenciales de tenant sin ningún throttling ni bloqueo.

---

## Cambios requeridos

1. Agregar dependencia `tower_governor` al workspace o implementar rate limiter
   manual con `DashMap<IpAddr, (u32, Instant)>` en `AppState`.

2. Opciones (elegir la más simple que pase CI):
   - **Opción A:** `tower_governor` como middleware Axum en la ruta `/api/auth/login`
     con límite de 5 intentos / 60 segundos por IP.
   - **Opción B:** `DashMap<String, AtomicU32>` en `AppState` con ventana deslizante
     por `tenant_id`. Reset automático a los 60 segundos. Retornar HTTP 429 al superar
     el umbral.

3. Loguear intentos bloqueados con `tracing::warn!` incluyendo IP y tenant_id
   (sin loguear el passphrase).

4. El límite debe ser configurable vía variable de entorno `AEGIS_AUTH_RATE_LIMIT`
   (default: 5 intentos / 60s).

---

## Criterios de aceptación

- [ ] `POST /api/auth/login` retorna HTTP 429 después de N intentos fallidos consecutivos
      desde la misma IP en la ventana de tiempo configurada
- [ ] HTTP 429 incluye header `Retry-After` con segundos hasta el reset
- [ ] Los intentos exitosos resetean el contador para ese tenant/IP
- [ ] `cargo build -p ank-http` sin errores ni warnings de clippy
- [ ] Sin `.unwrap()` ni `.expect()` en código nuevo

---

## Dependencias

Ninguna.

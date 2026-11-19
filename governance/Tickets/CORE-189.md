# CORE-189 — fix: Enclave se reinicializa cada 126 segundos — session keep-alive abre conexión SQLCipher innecesariamente

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-http + shell/ui  
**Tipo:** fix  
**Estado:** ✅ Done  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Dependencias:** ninguna

---

## Problema

Los logs muestran `Secure Enclave initialized successfully. tenant_id=Tavo` cada
~126 segundos durante más de 4 horas sin ninguna tarea procesada:

```
23:04 → Secure Enclave initialized
23:06 → Secure Enclave initialized
23:08 → Secure Enclave initialized
... (continúa cada 2m 6s hasta las 03:00)
```

`TenantDB::open()` llama `info!("Secure Enclave initialized successfully.")` — por lo tanto
**hay algo que abre el Enclave periódicamente sin procesar ninguna tarea**. Esto es:
- CPU innecesaria (~600ms por apertura SQLCipher)
- Leak de conexiones de base de datos
- Ruido en los logs que oculta eventos reales

---

## Causa raíz

El frontend (shell web o app) envía un request al servidor cada ~126 segundos para
renovar la sesión o mantener el token activo. Ese request llega a un endpoint de
autenticación que abre el `TenantDB` para verificar la sesión, loguea
"Secure Enclave initialized" y devuelve OK — pero no hay ninguna tarea que procesar.

El patrón exacto apunta a un **session refresh endpoint** que abre el Enclave solo
para validar que la session_key sigue siendo válida.

---

## Localización

Buscar con `search_code("Secure Enclave initialized")` para encontrar dónde se llama
`TenantDB::open()` fuera del flujo de scheduler. Luego buscar en `ank-http/src/routes/`
el endpoint que recibe el keep-alive.

---

## Cambios Requeridos

### Opción A — Cachear la conexión del Enclave (recomendada)

En lugar de abrir una nueva conexión SQLCipher en cada request, mantener una caché
de conexiones por tenant en el `AppState`. El `TenantDB` ya es `!Send` (SQLite
connection), así que hay que usar `Arc<Mutex<TenantDB>>` o mover la caché a un
`DashMap<String, Arc<Mutex<TenantDB>>>`.

En `AppState` (ank-http):

```rust
// Agregar al AppState
pub enclave_cache: Arc<DashMap<String, Arc<tokio::sync::Mutex<TenantDB>>>>,
```

En el helper que actualmente abre el Enclave por cada request:

```rust
// ANTES — abre conexión nueva en cada request
let db = TenantDB::open(&tenant_id, &session_key)?;

// DESPUÉS — reusar conexión cacheada
let db_arc = state.enclave_cache
    .entry(cache_key.clone())
    .or_try_insert_with(|| {
        TenantDB::open(&tenant_id, &session_key)
            .map(|db| Arc::new(tokio::sync::Mutex::new(db)))
    })?
    .clone();
let db = db_arc.lock().await;
```

La clave del cache debe ser `"{tenant_id}:{session_key_hash}"` para invalidar
automáticamente cuando cambia la session key.

### Opción B — Reducir frecuencia del keep-alive en el frontend (complementaria)

En `shell/ui/src/store/useAegisStore.ts`, el intervalo del session keep-alive
(probablemente en un `setInterval`) debe cambiarse de ~126s a 5 minutos mínimo,
o mejor: **eliminar el keep-alive activo** y renovar la sesión solo cuando hay
actividad del usuario.

Buscar en el store con `search_code("setInterval")` o `search_code("keepalive")`.

---

## Criterios de Aceptación

- [x] Los logs NO muestran `Secure Enclave initialized` en períodos de inactividad (sin mensajes del usuario)
- [x] El Enclave SÍ se inicializa correctamente cuando el usuario envía su primer mensaje tras login
- [x] Con la caché implementada, múltiples requests del mismo tenant en la misma sesión NO abren múltiples conexiones SQLCipher
- [x] Al hacer logout o cambiar session_key, la entrada del cache se invalida
- [x] `cargo build` sin warnings

---

## Commit message

```
fix(ank-http): CORE-189 cachear conexión TenantDB — evitar apertura SQLCipher en cada keep-alive
```

---

*Arquitecto IA — 2026-04-26*

---

## Resolución (2026-06-12)
Resuelto en feat/router-cmr-v3-hardening: cache in-process ONBOARDING_SETTLED_TENANTS en ws/chat.rs evita reabrir el enclave SQLCipher en cada reconexión WS; el keepalive de 30s (CORE-279) ya había eliminado las reconexiones periódicas.

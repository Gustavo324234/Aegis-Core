# CORE-090 — Fix: `admin_exists()` no lee el WAL de SQLite — STATE_INITIALIZING post-setup

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Kernel Engineer
**Prioridad:** 🔴 CRÍTICA — BLOQUEANTE PRODUCCIÓN
**Estado:** TODO

---

## Contexto

En producción, el flujo de primer arranque falla de la siguiente forma:

1. El proceso arranca → crea `admin.db` → escribe setup token en WAL
2. El usuario accede al setup URL con token → `initialize_master` escribe el admin en el WAL
3. La UI redirige al login → `GET /api/system/state` responde `STATE_INITIALIZING`
4. El usuario no puede loguearse

El `admin.db` principal tiene timestamp **00:05:16** (creación).
El `admin.db-wal` tiene timestamp **00:17:33** (escritura del admin vía setup token).

**El dato del admin está en el WAL pero `admin_exists()` no lo ve.**

## Root cause

`MasterEnclave` usa una única `Arc<Mutex<Connection>>` abierta al inicio.
SQLite en modo WAL usa snapshots de lectura: una conexión que estaba abierta
ANTES de que se escribiera el WAL puede no ver los datos nuevos hasta que
haga un nuevo `BEGIN` o se reconecte.

En el flujo actual, la misma conexión que hace `store_setup_token` (escritura)
también hace `admin_exists()` (lectura). Con `Arc<Mutex<Connection>>` bloqueante
en Tokio via `spawn_blocking`, los reads pueden quedar en un snapshot anterior
al write si no se fuerza un checkpoint o se usa `BEGIN IMMEDIATE`.

## Evidencia en producción

```
admin.db     modificado: 00:05:16  ← solo tiene el schema
admin.db-wal modificado: 00:17:33  ← tiene los datos del master admin
/api/system/state → STATE_INITIALIZING  ← admin_exists() devuelve false
```

Reiniciar el servicio (que fuerza reconexión y checkpoint del WAL) resuelve
el problema temporalmente.

---

## Fix

**Archivo:** `kernel/crates/ank-core/src/enclave/master.rs`

### Opción A — Forzar WAL checkpoint después de cada write (recomendada)

Agregar un helper privado `checkpoint` y llamarlo al final de `initialize_master`
y `store_setup_token`:

```rust
async fn checkpoint(&self) -> Result<()> {
    let conn = self.connection.lock().await;
    conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);")?;
    Ok(())
}
```

```rust
pub async fn initialize_master(&self, username: &str, passphrase_sha256: &str) -> Result<()> {
    // ... código existente ...
    info!("Master admin {} successfully configured.", username);
    // Forzar checkpoint para que la escritura sea visible a reads inmediatos
    self.checkpoint().await?;
    Ok(())
}
```

```rust
pub async fn store_setup_token(&self, token: &str, ttl_minutes: i64) -> Result<()> {
    // ... código existente ...
    self.checkpoint().await?;
    Ok(())
}
```

### Opción B — Cambiar journal_mode a DELETE (más simple, menos performance)

En `init_schema`, deshabilitar WAL:

```rust
async fn init_schema(&self) -> Result<()> {
    let conn = self.connection.lock().await;
    // Usar journal mode DELETE para evitar issues de WAL con conexión única
    conn.execute_batch("PRAGMA journal_mode=DELETE;")?;
    // ... resto del schema ...
}
```

**Usar Opción A** — WAL es más performante y el checkpoint resuelve el race sin
sacrificar el modo de journal.

---

## Criterios de aceptación

- [ ] Después de crear el Master Admin vía setup token, `GET /api/system/state`
  responde inmediatamente `STATE_OPERATIONAL` sin reiniciar el servicio
- [ ] El flujo completo funciona: setup token → crear admin → login
- [ ] `cargo build` pasa sin errores
- [ ] No regresión en los tests de `master.rs`

---

## Workaround inmediato (hasta que se mergee el fix)

Reiniciar el servicio después de hacer el setup inicial:
```bash
sudo systemctl restart aegis
```
Esto fuerza el checkpoint del WAL y `admin_exists()` verá el registro.

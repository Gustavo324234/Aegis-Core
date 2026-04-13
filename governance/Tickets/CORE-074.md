# CORE-074 — Fix: `get_sync_version` usa path relativo `VERSION` — usar `CARGO_PKG_VERSION`

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Kernel Engineer
**Prioridad:** 🟡 MEDIA
**Estado:** TODO

---

## Contexto

El endpoint `GET /api/system/sync_version` en `status.rs` lee la versión desde
un archivo `VERSION` con path relativo:

```rust
pub async fn get_sync_version() -> Json<Value> {
    let version = std::fs::read_to_string("VERSION")
        .unwrap_or_else(|_| "0.1.0".to_string())
        .trim()
        .to_string();
    Json(json!({ "version": version }))
}
```

**Problema:** `std::fs::read_to_string("VERSION")` resuelve el path relativo al
**working directory del proceso**, no al directorio del binario. En instalación
nativa con systemd, el proceso corre con `WorkingDirectory` no definido, lo que
lo hace imprevisible. En Docker, el archivo `VERSION` no está en la imagen.

El fallback `"0.1.0"` siempre se activa en producción, haciendo el endpoint inútil.

---

## Solución

Rust embebe la versión del `Cargo.toml` en tiempo de compilación via la macro
`env!("CARGO_PKG_VERSION")`. Es la forma idiomática y correcta:

```rust
pub async fn get_sync_version() -> Json<Value> {
    Json(json!({ "version": env!("CARGO_PKG_VERSION") }))
}
```

Esto:
- No requiere ningún archivo en el filesystem
- Es inmutable en tiempo de ejecución (la versión está embebida en el binario)
- Funciona igual en Docker, nativo y desarrollo

---

## Cambios requeridos

**Archivo:** `kernel/crates/ank-http/src/routes/status.rs`

Reemplazar la función `get_sync_version` completa:

```rust
pub async fn get_sync_version() -> Json<Value> {
    Json(json!({ "version": env!("CARGO_PKG_VERSION") }))
}
```

La función es `async` por consistencia con el router — puede mantenerse así
aunque no haya operaciones async.

---

## Criterios de aceptación

- [ ] `GET /api/system/sync_version` retorna la versión del `Cargo.toml` de `ank-http`
- [ ] No hay ninguna lectura de archivo `VERSION` en `status.rs`
- [ ] La función compila sin warnings de clippy
- [ ] `cargo build` pasa sin errores

---

## Dependencias

Ninguna. Cambio de 3 líneas, riesgo cero.

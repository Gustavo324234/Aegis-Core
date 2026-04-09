# [CORE-016] ank-http: Static File Serving (SPA)
**Status:** DONE
embebida

**Épica:** 32 — Unified Binary
**Fase:** 2 — Servidor HTTP nativo
**Repo:** Aegis-Core — `kernel/crates/ank-http/src/static_files.rs`
**Asignado a:** Kernel Engineer
**Prioridad:** 🟡 Media
**Estado:** TODO
**Depende de:** CORE-010

---

## Contexto

`ank-server` debe servir la SPA React directamente. En modo producción, el
`dist/` de la UI se embebe en el binario usando `include_dir!` o se sirve
desde un path configurable. En modo desarrollo, la UI usa Vite dev server
con proxy (no necesita este endpoint).

---

## Dos modos

### Modo producción (default)
Los archivos estáticos se sirven desde `config.ui_dist_path` si está configurado,
o embebidos en el binario con `include_dir!` si se compila con `--features embed-ui`.

### Modo desarrollo
Si `config.dev_mode == true`, el handler retorna 404 en `/assets/*` y deja que
Vite dev server en `:5173` sirva la UI (con proxy configurado en `vite.config.ts`).

---

## Trabajo requerido

### `src/static_files.rs`

```rust
// Catch-all handler: cualquier path no-API sirve index.html (SPA routing)
pub async fn spa_handler(
    State(state): State<AppState>,
    uri: axum::http::Uri,
) -> impl IntoResponse {
    if state.config.dev_mode {
        return StatusCode::NOT_FOUND.into_response();
    }
    // Servir desde ui_dist_path o retornar 404 claro
    // ...
}
```

### En `routes/mod.rs` — registrar el catch-all al final del router

```rust
// IMPORTANTE: debe ser el último handler registrado
.fallback(static_files::spa_handler)
```

### `HttpConfig` — agregar campo

```rust
pub struct HttpConfig {
    // ...existentes...
    pub ui_dist_path: Option<std::path::PathBuf>,
}
```

El path se resuelve: primero `UI_DIST_PATH` env var, luego
`{binary_dir}/../shell/ui/dist/`, luego embebido.

---

## Criterios de aceptación

- [ ] `GET /` sirve `index.html` en modo producción
- [ ] `GET /assets/index-abc123.js` sirve el asset correcto
- [ ] `GET /cualquier/ruta/spa` sirve `index.html` (SPA client-side routing)
- [ ] `GET /api/inexistente` retorna 404 JSON (no index.html)
- [ ] En `dev_mode`, el fallback retorna 404 (Vite sirve la UI)
- [ ] `cargo clippy -p ank-http -- -D warnings -D clippy::unwrap_used` → 0 warnings

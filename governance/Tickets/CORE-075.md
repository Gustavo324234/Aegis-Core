# CORE-075 — Fix: `engine_config.json` con path relativo — persistir en `data_dir`

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Kernel Engineer
**Prioridad:** 🔴 CRÍTICA
**Estado:** TODO

---

## Contexto

Los endpoints `/api/engine/status` y `/api/engine/configure` en
`kernel/crates/ank-http/src/routes/engine.rs` leen y escriben la configuración
del motor cognitivo en un path relativo:

```rust
// get_status
let config_path = "engine_config.json";
fs::read_to_string(config_path)

// configure
fs::write("engine_config.json", config_json)
```

**Impacto en producción:** El archivo se escribe relativo al working directory
del proceso `ank-server`. En instalación nativa con systemd, el cwd no está
definido (por defecto `/`). En Docker, el cwd es `/`. En ambos casos el archivo
se pierde o no se encuentra tras un restart.

**Resultado observable para el usuario:** El operador configura su provider de
IA (API key, modelo, URL), el servicio se reinicia (update, reboot del servidor),
y Aegis "olvida" la configuración — vuelve a `{ "configured": false }` y el
wizard de setup aparece de nuevo.

---

## Cambios requeridos

**Archivo:** `kernel/crates/ank-http/src/routes/engine.rs`

### 1. Usar `state.config.data_dir` para resolver el path

```rust
pub async fn get_status(
    State(state): State<AppState>
) -> Result<Json<Value>, AegisHttpError> {
    let config_path = state.config.data_dir.join("engine_config.json");
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        if let Ok(val) = serde_json::from_str::<Value>(&content) {
            return Ok(Json(val));
        }
    }
    Ok(Json(json!({ "configured": false })))
}
```

### 2. Escribir en `data_dir` al configurar

```rust
pub async fn configure(
    State(state): State<AppState>,
    Json(body): Json<EngineConfig>,
) -> Result<Json<Value>, AegisHttpError> {
    // ... auth igual que antes ...

    let config_path = state.config.data_dir.join("engine_config.json");
    fs::write(&config_path, config_json)
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    Ok(Json(json!({ "success": true, "message": "Cognitive Engine dynamically configured." })))
}
```

### 3. Eliminar el `use std::fs;` estático y usar `tokio::fs` (ya importado en workspace) o `std::fs` — consistente con el resto del archivo.

---

## Criterios de aceptación

- [ ] `engine_config.json` se escribe en `state.config.data_dir` (ej: `/var/lib/aegis/engine_config.json` en nativo, `/data/engine_config.json` en Docker)
- [ ] Tras un restart del servicio, `GET /api/engine/status` retorna `configured: true` si fue configurado previamente
- [ ] No hay ningún path relativo `"engine_config.json"` en el archivo
- [ ] `cargo build` pasa sin errores

---

## Dependencias

`state.config.data_dir` ya está disponible en `AppState` via `HttpConfig`.
`AEGIS_DATA_DIR` se setea en `/etc/aegis/aegis.env` por el installer.

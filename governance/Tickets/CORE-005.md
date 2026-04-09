# CORE-005 — aegis-sdk: Wasm plugin SDK

**Épica:** 32 — Unified Binary
**Fase:** 1 — Fundación Rust
**Repo:** Aegis-Core — `kernel/crates/aegis-sdk/`
**Asignado a:** Kernel Engineer
**Prioridad:** 🟡 Media
**Estado:** DONE
**Depende de:** CORE-001

---

## Contexto

El SDK Wasm permite a autores externos escribir plugins para Aegis.
Se compila a `.wasm` y se carga dinámicamente en el kernel.

**Referencia legacy:** `Aegis-ANK/crates/aegis-sdk/`

---

## Trabajo requerido

Portar `Aegis-ANK/crates/aegis-sdk/src/` a `kernel/crates/aegis-sdk/src/` sin cambios.
Portar también `Aegis-ANK/plugins_src/` a `kernel/plugins_src/` (plugins estándar).

### `kernel/crates/aegis-sdk/Cargo.toml`

```toml
[package]
name = "aegis-sdk"
version = "0.1.0"
edition = "2021"

[dependencies]
serde.workspace      = true
serde_json.workspace = true
```

---

## Criterios de aceptación

- [x] `cargo build -p aegis-sdk` compila sin errores
- [x] Los macros y traits del SDK son públicos
- [x] `cargo clippy -p aegis-sdk -- -D warnings` → 0 warnings

## Referencia

`Aegis-ANK/crates/aegis-sdk/` — fuente completa
`Aegis-ANK/plugins_src/` — plugins estándar

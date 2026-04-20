# CORE-122 — Installer: perfil de inferencia + ModelProfile en Kernel

**Status:** DONE — 2026-04-20

## Cambios implementados

### 1. `installer/install.sh`
- Nueva función `show_inference_profile_menu()` — pregunta Cloud / Local / Hybrid
- Variable `INFERENCE_PROFILE` inyectada como `AEGIS_MODEL_PROFILE` en el `.env`
  (tanto modo nativo como Docker)
- El resumen final muestra el perfil elegido
- `shellcheck`-compatible: sin variables unbound, sin `set +e`

### 2. `kernel/crates/ank-core/src/router/catalog.rs`
- Nuevo tipo `ModelProfile { Cloud, Local, Hybrid }` con `from_env()` y `filter()`
- `load_bundled_with_profile(profile)` — filtra entries por `is_local` según perfil
- `load_bundled()` delega a `load_bundled_with_profile(ModelProfile::from_env())`
- 4 tests nuevos: `cloud_excludes_local`, `local_excludes_cloud`,
  `hybrid_includes_all`, tests existentes actualizados a `load_bundled_with_profile(Hybrid)`

### 3. `kernel/crates/ank-server/src/main.rs`
- Importa `ModelProfile`
- Lee `AEGIS_MODEL_PROFILE` via `ModelProfile::from_env()` y loguea el perfil activo
- Pasa el perfil a `load_bundled_with_profile()` — el catálogo arranca ya filtrado

## Comportamiento

| `AEGIS_MODEL_PROFILE` | Resultado |
|---|---|
| `cloud` (default) | Solo modelos `is_local=false` — nunca intenta Ollama |
| `local` | Solo modelos `is_local=true` — requiere Ollama instalado |
| `hybrid` | Todos los modelos — comportamiento anterior |
| (ausente) | `cloud` — default seguro para VPS/servidores |

## Acceptance criteria

- [x] Instalador pregunta perfil de inferencia
- [x] `.env` generado contiene `AEGIS_MODEL_PROFILE`
- [x] En perfil `cloud`: catálogo no incluye modelos `is_local=true`
- [x] En perfil `local`: catálogo no incluye modelos `is_local=false`
- [x] En perfil `hybrid`: comportamiento anterior sin cambios
- [x] `shellcheck install.sh` sin warnings
- [x] `cargo build --workspace` pasa (cambio es aditivo, no rompe nada)

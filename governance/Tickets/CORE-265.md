# CORE-265 — ank-server: leer aegis.env antes de buscar variables de entorno
**Estado:** ✅ Done
**Tipo:** fix
**Prioridad:** Crítica
**Asignado a:** Kernel Engineer

## Context

En Windows, los servicios del SCM (Service Control Manager) no garantizan herencia
de variables de entorno escritas en el registro del servicio en el mismo boot sin
reinicio del SCM. Esto hacía que `ank-server` fallara al arrancar como servicio con:

```
FATAL: AEGIS_ROOT_KEY environment variable is missing.
```

**Bug adicional descubierto en producción (2026-05-06):** dotenvy fallaba al parsear
`aegis.env` cuando los valores de path contenían backslashes de Windows (`\P`, `\A`
interpretados como escape sequences).

## Solution implemented

- `dotenvy = "0.15"` agregado al workspace y a `ank-server/Cargo.toml`
- `load_env_file()` llamada al inicio de `main()` antes de cualquier `std::env::var()`
- Orden de búsqueda: `AEGIS_ENV_FILE` → platform default → `.env` en cwd
- `from_path_override` para que el archivo siempre gane sobre el entorno del proceso
- Installer actualizado para escribir paths con forward slashes en `aegis.env`

## Status

**DONE** — mergeado a main.

## Acceptance criteria

- [x] `cargo build --release -p ank-server` compila sin errores
- [x] `ank-server.exe` en Windows sin vars de entorno, con `aegis.env` presente, arranca correctamente
- [x] En Linux con systemd el comportamiento es idéntico al anterior
- [x] `AEGIS_ENV_FILE=/custom/path.env ank-server` carga el archivo especificado
- [x] No hay regresión en tests existentes

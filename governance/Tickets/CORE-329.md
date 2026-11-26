# CORE-329 — Installer: self-update atómico, canal stable funcional y versión de producto

**Tipo:** fix
**Prioridad:** Crítica
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure
**Estado:** ✅ Done
**Asignado a:** DevOps Engineer

---

## Problema

Reportado en producción (VPS srv1659876) al correr `aegis update` tras el lanzamiento v0.2.0:

1. **Corrupción del script en ejecución**: el self-update hacía `sudo cp` sobre el archivo vivo. Bash lee los scripts perezosamente del fd abierto; `cp` trunca y reescribe el MISMO inode, así que la instancia corriendo siguió leyendo en su offset viejo dentro del contenido nuevo y ejecutó basura: `line 596: ho: command not found` (un `echo` con los primeros bytes comidos). El comentario del código ("bash loads the script into memory") era falso.
2. **Salida basura en "Última disponible"**: `grep -oP '"\K[^"]+(?=")'` sobre la línea del JSON devolvía TODAS las cadenas (`published_at`, `: `, la fecha) — imprimía `published_at\n:\n2026-...`.
3. **El canal stable nunca funcionó**: la URL `releases/download/latest/...` no existe en GitHub — el alias correcto es `releases/latest/download/...` (orden invertido). `aegis update --stable` siempre devolvía 404.
4. **Confusión de versión**: el binario reporta la versión del crate (`v0.1.89`) sin indicar a qué release de producto corresponde — el usuario actualizó esperando "0.2.0" y no podía saber que ya lo tenía.

## Solución implementada

1. **Rename atómico**: el self-update escribe a `${self_path}.new` (mismo directorio/filesystem) y hace `mv -f` — rename(2) apunta la entrada de directorio a un inode NUEVO; el bash corriendo termina limpio desde el viejo.
2. **Parseo correcto**: `grep -oP '"tag_name":\s*"\K[^"]+'` / `"published_at"` con patrón anclado a la clave; muestra `v0.2.0 (fecha)`.
3. **URL del alias latest** corregida + endpoint API `releases/latest` para el canal stable.
4. **Canal default → stable** (existe v0.2.0 con todos los assets); `--nightly` queda como opt-in. Help y sugerencias de error actualizados.
5. **Versión de producto**: `publish-native` inyecta `AEGIS_PRODUCT_VERSION` (tag `vX.Y.Z` o `nightly`) y `ank-server --version` imprime `Aegis Core v0.1.89 (v0.2.0)`.

## Criterios de aceptación

- [x] `aegis update` no corrompe la instancia en ejecución (replace por rename, no por cp).
- [x] "Última disponible" muestra tag + fecha, no claves del JSON.
- [x] El canal stable (default) descarga desde `releases/latest/download/`.
- [x] `bash -n` y shellcheck del CI en verde; YAML del workflow parsea; `cargo check -p ank-server` en verde.
- [x] Los binarios de CI reportan la versión de producto en `--version`.

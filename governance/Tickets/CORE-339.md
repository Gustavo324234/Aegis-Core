# CORE-339 — Instalador Windows: CLI roto en PowerShell 5.1, canal de update divergente y docs de CLI desactualizadas

**Tipo:** fix
**Prioridad:** Crítica
**Épica:** EPIC 56 — Public MVP / Thesis Validation
**Estado:** ✅ Done
**Asignado a:** DevOps Engineer

---

## Problema

Auditoría del 2026-07-10 detectó que el CLI de Windows (`installer/aegis.ps1`) estaba **completamente roto** en todas las instalaciones de Windows, más una regresión latente en el WIP de `install.ps1` y documentación de CLI desactualizada:

1. **`aegis.ps1` no parsea en PowerShell 5.1** (12 errores en cascada): la línea del dispatch de `trace` usaba el operador `??` (solo PS7+) y referenciaba `$Arg2`, que no existía en el bloque `param()`. El wrapper `aegis.cmd` invoca `powershell.exe` (siempre 5.1), por lo que **ningún comando del CLI funcionaba** tras la instalación.
2. **`cmd_trace` fallaba siempre en runtime** aunque se arreglara el parseo: su parámetro se llamaba `$Pid`, que colisiona con la variable automática de solo lectura `$PID` de PowerShell ("No se puede sobrescribir la variable Pid").
3. **`aegis update --stable` en Windows hacía update nightly silenciosamente**: PowerShell bindea `--stable` como argumento posicional (no como switch), y el default de `cmd_update` era nightly — divergente del wrapper bash, que desde CORE-329 usa stable por defecto.
4. **Regresión de encoding en el WIP de `install.ps1`**: al quitarse el BOM UTF-8, PowerShell 5.1 decodifica el archivo como ANSI/CP1252; los caracteres multi-byte mal decodificados producen comillas tipográficas (U+2019/U+201D) que PS interpreta como delimitadores de string reales → el instalador no parsea (verificado: 3 errores con decode ANSI, 0 con UTF-8).
5. **Bug en el WIP de `install.ps1`**: se eliminó la asignación de `$hasPluginKey` pero se dejó el `if (-not $hasPluginKey)` → condición siempre verdadera.
6. `aegis.ps1` sin privilegios de admin imprimía un error rojo de `Acceso denegado` al leer `aegis.env` (ACLs restringidas) incluso para `help`/`version`.

## Solución aplicada

- `installer/aegis.ps1`:
  - Se agregó `[Parameter(Position=2)] [string]$Arg2` al `param()`.
  - Dispatch de `trace` reescrito con sintaxis compatible 5.1 (sin `??`); acepta `trace [N] [PID]` y también `trace <pid>` directo.
  - `cmd_trace`: parámetro renombrado `$Pid` → `$PidFilter` (con nota para no reintroducirlo).
  - `cmd_update`: default = **stable** (paridad CORE-329 con Linux); acepta `-Nightly`/`-Stable` y también los flags estilo bash `--nightly`/`--beta`/`--stable` que llegan como posicionales.
  - Texto de ayuda actualizado (`update` = stable, `update --nightly` = nightly).
  - Lectura de `aegis.env` con `-ErrorAction SilentlyContinue` (degradación grácil sin admin).
  - BOM UTF-8 restaurado.
- `installer/install.ps1`: BOM UTF-8 restaurado; `$hasPluginKey` se define de nuevo (detecta `AEGIS_PLUGIN_ROOT_KEY=`) y el comentario obsoleto del backfill se actualizó a la semántica de auto-generación.
- Verificación: ambos scripts parsean con 0 errores (tokenizer 5.1 y parser moderno); smoke-test `aegis help` OK con exit 0.

## Criterios de aceptación

- [x] `powershell.exe -File aegis.ps1 help` corre sin errores de parseo ni de acceso en PS 5.1.
- [x] `aegis update` (Windows) usa canal stable por defecto; `--nightly`/`-Nightly` opt-in.
- [x] `aegis trace [N] [PID]` funcional (sin colisión con `$PID`).
- [x] `install.ps1` y `aegis.ps1` con BOM UTF-8 (requerido por el decode ANSI de PS 5.1).
- [x] `docs/CLI_REFERENCE.md`, `README.md` y `README.es.md` reflejan la semántica real de canales de update y la existencia del CLI en Windows.

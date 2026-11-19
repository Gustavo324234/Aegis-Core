# CORE-341 — Release pipeline: build Windows roto por CMake 4.x y fallos de plataforma silenciados

**Tipo:** fix
**Prioridad:** Crítica
**Épica:** EPIC 56 — Public MVP / Thesis Validation
**Estado:** ✅ Done
**Asignado a:** DevOps Engineer

---

## Problema

El release `v0.2.1` y el nightly vigente se publicaron **sin binarios de Windows** y el pipeline reportó éxito:

1. **Build roto**: el runner `windows-latest` actualizó a Visual Studio 18 2026 + CMake 4.x, que rechaza el `cmake_minimum_required` antiguo del CMakeLists de Opus embebido en `audiopus_sys` (dependencia del protocolo de voz Siren). El build script panickeaba en `cmake::Config::build`.
2. **Fallo silenciado**: `build-windows-x86_64` (y ambos jobs de macOS) tenían `continue-on-error: true`, así que el run quedaba verde y el job `release` publicaba igual (su `if` solo exige ui + linux-x86_64). Resultado: instalaciones frescas de Windows rotas (el `install.ps1` descarga del nightly, que no tenía el zip) sin ninguna señal en CI.

## Solución aplicada

En `.github/workflows/publish-native.yml`:
- Job Windows: `CMAKE_POLICY_VERSION_MINIMUM: "3.5"` en el env (honrado por CMake ≥4.0; restaura el piso de policy para el build de Opus).
- Eliminado `continue-on-error: true` de los jobs Windows y macOS (ARM64 y x86_64): un fallo de plataforma ahora pinta el run de rojo. El diseño de "release parcial" se conserva — el job `release` sigue publicando las plataformas que compilaron (su condición no cambió).

## Criterios de aceptación

- [x] YAML del workflow válido.
- [x] Job Windows compila `audiopus_sys` bajo CMake 4.x (verificar en el próximo run del pipeline).
- [x] Un fallo de plataforma deja el run en rojo sin bloquear la publicación de las demás.
- [ ] `v0.2.1` re-publicado con los 4 assets de Windows (requiere mover el tag tras el merge).

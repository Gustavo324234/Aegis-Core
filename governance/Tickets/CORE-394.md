# CORE-394: Promoción de Mejoras del Sandbox al Código General (Creator Studio, Hub de Módulos, Respuestas con Descargas y Textarea Autoajustable)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 61
- **Asignado a:** Kernel Engineer + Shell Engineer

---

## Description

Consolidar formalmente dentro del código general (`ank-core`, `ank-http`, `kernel/modules/`, `shell/ui`) y verificar de punta a punta las capacidades validadas en el entorno Sandbox (`:8090`):
1. **Creator Studio (`aegis-creator` + `aegis-video` + `VideoStudioPanel.tsx`):** generación de guiones, escenas, assets y render MP4/SRT integrado con `ModuleRuntime`.
2. **Mejora del Hub de Módulos (`ModulesAppHub.tsx`, `kernel/crates/ank-core/src/router/modules.rs`, `module_runtime.rs`, `modules_handlers.rs`):** descubrimiento, activación bajo demanda, carga en prompt y ejecución de herramientas de dominio.
3. **Respuestas en Chat con Tarjetas de Descarga (`ChatTerminal.tsx`):** parsing de `[AEGIS_VIDEO_CARD:<project_id>]` / IDs legacy y componente `ChatVideoResultCard` con preview y descarga autenticada de MP4/SRT.
4. **Cuadro de Texto Autoajustable (`ChatTerminal.tsx`):** `textarea` con ajuste automático de altura (`scrollHeight` acotado entre 44px y 280px).

## Acceptance Criteria

- [x] Paridad completa verificada entre las funcionalidades probadas en sandbox (`:8090`) y el código general del workspace (`ank-core`, `ank-http`, `kernel/modules/`, `shell/ui`).
- [x] Módulos `aegis-creator`, `aegis-video`, `aegis-biz` y `aegis-dev` integrados, documentados y cubiertos por la promoción hacia `main` (`CORE-396`) y el empaquetado de producción (`CORE-397`).
- [x] Compilación y tests pasando en `ank-core`, `ank-http` y `shell/ui`.

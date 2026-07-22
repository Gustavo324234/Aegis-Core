# CORE-342 — Modularización de NixOS Flake & Configuración de Entrega Dual (Server vs. Kiosk)

**Tipo:** feat
**Prioridad:** Alta
**Épica:** EPIC 57 — Linux Distro & Native Hardware Integration
**Estado:** ✅ Done
**Asignado a:** DevOps Engineer

---

## Problema

Para convertir Aegis OS en una distribución Linux nativa inmutable y ligera, el sistema debe ser capaz de arrancar tanto en modo servidor headless sin entorno gráfico como en modo consola interactiva de quiosco sobre computadoras portátiles o hardware dedicado.

## Solución propuesta

1. Actualizar `distro/nixos/flake.nix` para exponer dos configuraciones principales: `aegis-server` y `aegis-kiosk`.
2. Consolidar `distro/nixos/profile-server.nix` para optimización de energía al cerrar la tapa de la laptop, deshabilitando el subsistema gráfico.
3. Consolidar `distro/nixos/profile-kiosk.nix` lanzando el compositor Wayland Cage en modo Kiosk ejecutando Chromium hacia `http://localhost:8000`.

## Criterios de aceptación

- [x] `flake.nix` declara limpiamente ambas salidas (`aegis-server`, `aegis-kiosk` y `aegis-iso`).
- [x] Módulos de NixOS probados contra sintaxis `nix flake check` / evaluación sin errores.
- [x] Documentación de perfiles incluida en la configuración del repositorio.

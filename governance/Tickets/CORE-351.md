# CORE-351 — Create Missing `profile-server.nix` for Headless NixOS Flake

**Tipo:** fix
**Prioridad:** Alta
**Épica:** EPIC 57 — Linux Distro & Native Hardware Integration
**Estado:** ✅ Done
**Asignado a:** DevOps Engineer

---

## Problema

`distro/nixos/flake.nix` importa `./profile-server.nix` tanto en `nixosConfigurations.aegis-server` como en `nixosConfigurations.aegis-iso`, y la documentación (`distro/README.md` y `docs/ROADMAP_COGNITIVE_OS.md`) lo referencia, pero el archivo `distro/nixos/profile-server.nix` no existía en el repositorio, rompiendo la evaluación del Flake de NixOS.

## Solución propuesta

1. Crear `distro/nixos/profile-server.nix` con la configuración declarativa headless para servidores y laptops en modo nodo:
   - Desactivación del entorno gráfico X11/Wayland y sonido innecesario en modo headless.
   - Configuración de `services.logind` (`lidSwitch = "ignore"`, `lidSwitchExternalPower = "ignore"`) para permitir que una laptop actúe como Home Server con la tapa cerrada sin suspender el bucle de agentes.
   - Optimización de energía y parámetros de kernel orientados a servidor 24/7.
2. Limpiar enlaces absolutos locales (`file:///e:/Aegis/...`) en `distro/README.md`, `docs/ROADMAP_COGNITIVE_OS.md` y `docs/NATIVE_TRAINING_ENGINE.md` por rutas relativas limpias.

## Criterios de aceptación

- [x] `distro/nixos/profile-server.nix` existe y cumple con las especificaciones de `flake.nix` y `ROADMAP_COGNITIVE_OS.md`.
- [x] Documentación en `distro/` y `docs/` libre de rutas absolutas locales (`file:///e:/...`).

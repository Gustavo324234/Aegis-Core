# EPIC 57 — Linux Distro & Native Hardware Integration

**Estado:** 🚧 In Progress

## 🎯 Objetivo

Evolucionar Aegis OS desde un demonio de servidor sobre un SO host hacia una **distribución Linux inmutable y declarativa basada en NixOS (`distro/nixos`)**, integrando soporte de entrega dual (Servidor Headless vs. Consola de Quiosco Kiosk con compositor Wayland Cage y audio PipeWire) y exponiendo herramientas de telemetría y control de hardware físico directamente al kernel de agentes (`ank-core`).

---

## 🏗️ Alcance y Criterios de Aceptación

1. **Arquitectura NixOS Dual (`CORE-342`):**
   - Configuración declarativa en `distro/nixos/flake.nix`, `profile-server.nix` y `profile-kiosk.nix`.
   - Soporte de modo Headless (ahorro de energía con tapa de laptop cerrada) y modo Kiosk (Wayland Cage + Chromium Kiosk + PipeWire).
2. **Telemetría de Hardware Nativa (`CORE-343`):**
   - Implementar syscall `SYS_HW_TELEMETRY` y herramienta para agentes en `ank-core` que reporte batería, temperatura de CPU, carga de memoria y estado térmico.
3. **Control de Audio Nativo (`CORE-344`):**
   - Implementar syscall `SYS_AUDIO_CONTROL` y módulo de control PipeWire para listar nodos de audio, silenciar/desmutear micrófono e inspeccionar volumen.
4. **Guía de Compilación de Distro (`CORE-345`):**
   - Crear `distro/README.md` con instrucciones de compilación de ISO, prueba en máquina virtual y flasheo en USB.

---

## 🛠️ Listado de Tickets Mapeados

* **CORE-342:** feat — Modularización de NixOS Flake & Configuración de Entrega Dual (Server vs. Kiosk).
* **CORE-343:** feat — Kernel Syscall & Tool: Telemetría de Hardware Nativa (`SYS_HW_TELEMETRY`).
* **CORE-344:** feat — Kernel Syscall & Tool: Control de Audio PipeWire (`SYS_AUDIO_CONTROL`).
* **CORE-345:** docs — Guía de compilación de imagen ISO de NixOS y despliegue bare-metal.

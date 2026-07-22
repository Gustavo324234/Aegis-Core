# CORE-345 — Guía de Compilación de Imagen ISO de NixOS y Despliegue Bare-Metal

**Tipo:** docs
**Prioridad:** Alta
**Épica:** EPIC 57 — Linux Distro & Native Hardware Integration
**Estado:** ✅ Done
**Asignado a:** DevOps Engineer

---

## Problema

Para permitir a la comunidad y desarrolladores probar Aegis OS directamente en computadoras portátiles o hardware dedicado, se necesita una guía clara y reproducible que explique cómo compilar la imagen ISO booteable mediante NixOS y flashearla en un medio USB.

## Solución propuesta

1. Crear `distro/README.md` documentando los requisitos de Nix (Flakes habilitados), comandos de generación de ISO (`nix build .#nixosConfigurations.aegis-iso.config.system.build.isoImage`).
2. Documentar los pasos para flashear la imagen en USB (`dd` o Rufus) y los pasos de instalación en disco con cifrado LUKS2.

## Criterios de aceptación

- [x] Creación de `distro/README.md` con la guía paso a paso de compilación e instalación.
- [x] Verificación de comandos e instrucciones de prueba en entorno local o máquina virtual.

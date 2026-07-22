# CORE-335 — B4 — Honestidad de alcance: Clarificación de características en desarrollo

**Tipo:** docs
**Prioridad:** Alta
**Épica:** EPIC 56 — Public MVP / Thesis Validation
**Estado:** ✅ Done
**Asignado a:** Arquitecto IA

---

## Problema

Declarar en la documentación pública que el 100% de la superficie del proyecto (incluyendo audio WebRTC, aplicación móvil React Native o compilación dinámica de scripts) está completada, sin advertir que algunas partes están en fase experimental, genera expectativas falsas. Un desarrollador senior perderá la confianza en el monorepo al encontrarse con fallos en módulos que se promocionan como estables.

## Solución propuesta

Revisar exhaustivamente el README público y la documentación de introducción para etiquetar de manera explícita el estado de madurez de cada componente:
* **Estables / Listos para Producción:** Kernel determinista (`ank-server`), motor cognitivo ReAct, ruteador de modelos local/nube (CMR), base de datos local cifrada Citadel, y interfaz web de la Shell.
* **Experimentales / Roadmap:** App móvil (Orion ID linkage), streaming bidireccional de voz Siren en WebRTC, compilación dinámica de código en sandbox (Maker Capability), y el motor de sincronización distribuida.

## Criterios de aceptación

- [x] Incorporación de tags visuales (ej. `[Core / Stable]` vs `[Experimental / Roadmap]`) en el listado de funcionalidades del README principal.
- [x] Sección de limitaciones conocidas detallando el estado actual de las pruebas de la app móvil y el protocolo de audio.

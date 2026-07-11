# EPIC 56 — Public MVP / Thesis Validation

**Estado:** 🚧 In Progress

## 🎯 Objetivo

Preparar el repositorio y el instalador de Aegis Core para su primer lanzamiento público (MVP). El foco exclusivo de esta épica no es añadir nuevas funcionalidades de software, sino **validar la tesis técnica del kernel cognitivo** ante desarrolladores de software experimentados, reduciendo las fricciones de instalación, publicando evidencia de rendimiento de CMR (Cognitive Model Router) y mitigando cualquier duda de seguridad de Citadel.

---

## 🏗️ Alcance y Criterios de Aceptación del MVP

El MVP se centrará en el modo de entrega de **Aegis Overlay/Satélite** (instalación directa del binario `ank-server` y la UI web sobre un sistema operativo host existente). El modo bare-metal de **Aegis OS (NixOS Distro)** se presentará como parte del roadmap de la v2.

Para lograr una validación exitosa ante un público de desarrolladores escépticos (como usuarios de Hacker News, Reddit o GitHub), se deben completar las siguientes metas:

1.  **Demostración Visual Unificada (B1):** Proveer un video interactivo o GIF que muestre el ciclo cognitivo en tiempo real (Chat maestro ➔ Specialist spawn ➔ Ejecución de herramientas locales ➔ Reporte final).
2.  **Transparencia de Datos (B2):** Publicación del benchmark PinchBench detallando latencias, costes y fiabilidad de CMR frente a tareas complejas.
3.  **Higiene y Credibilidad de Código (B3 + B4):** Visualización de diagramas del Cognitive Loop y badges de integración continua, junto con la clarificación explícita de qué partes del sistema siguen en fase experimental (voz Siren, app móvil, maker).
4.  **On-boarding Seguro y Confiable (B5):** Soporte oficial para Docker, hashes SHA256SUMS de binarios y versiones estables taggeadas.
5.  **Cierre de Citadel (B6):** Eliminar el bypass por defecto de firmas del SDK instalando una utilidad de clave ed25519 (`keygen`) por tenant de forma transparente.

---

## 🛠️ Listado de Tickets Mapeados

*   **CORE-332:** B1 — Asset de prueba: Grabación de video demostrativo/GIF interactivo.
*   **CORE-333:** B2 — Benchmarks: Publicación de resultados de PinchBench.
*   **CORE-334:** B3 — Credibilidad: Incorporación de diagramas e insignias de tests en README.
*   **CORE-335:** B4 — Honestidad de alcance: Clarificación de características en desarrollo.
*   **CORE-336:** B5 — On-ramp: Instalador robusto, releases taggeadas y verificación SHA256.
*   **CORE-337:** B6 — Citadel: Generación de firmas ed25519 automáticas al arranque local (keygen).
*   **CORE-338:** B8 — Transparencia: Publicación de notas honestas de dogfooding.

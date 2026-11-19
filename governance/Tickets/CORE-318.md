# CORE-318 — UX fixes: skeletons de telemetría y accesibilidad ARIA en DynamicModulePanel

**Tipo:** chore  
**Prioridad:** Alta  
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure  
**Estado:** ✅ Done  
**Asignado a:** UI Engineer  

---

## Problema
El panel dinámico de módulos y los widgets de telemetría muestran estados inestables o vacíos mientras se cargan los datos, afectando la experiencia de usuario. Además, se requiere mayor accesibilidad para lectores de pantalla.

## Solución propuesta
1. Implementar skeletons (marcadores de posición visuales de carga) en la UI de telemetría de la shell.
2. Añadir atributos de accesibilidad ARIA en `DynamicModulePanel` para cumplir con las mejores prácticas de UX.

## Criterios de aceptación
- [x] Skeletons de carga suaves y estilizados implementados en los widgets de telemetría.
- [x] Atributos ARIA (ej. `aria-label`, `aria-busy`, `role`) añadidos a los componentes interactivos del panel.

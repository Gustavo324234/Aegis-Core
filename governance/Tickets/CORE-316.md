# CORE-316 — postcss >= 8.5.10 en shell/ui para mitigar XSS (GHSA-qx2v-qp2m-jg93)

**Tipo:** fix  
**Prioridad:** Alta  
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure  
**Estado:** ✅ Done  
**Asignado a:** UI Engineer  

---

## Problema
Se identificó una vulnerabilidad de Cross-Site Scripting (XSS) en la dependencia `postcss` inferior a `8.5.10` en la interfaz gráfica del Shell (GHSA-qx2v-qp2m-jg93).

## Solución propuesta
Actualizar la versión de `postcss` a `8.5.10` o superior en el paquete npm `shell/ui/package.json` para mitigar la vulnerabilidad XSS y garantizar la seguridad de la interfaz gráfica.

## Criterios de aceptación
- [x] Dependencia `postcss` actualizada en `shell/ui/package.json` a versión >= `8.5.10`.
- [x] Comprobación de que no hay regresiones visuales o de compilación en el build del frontend.

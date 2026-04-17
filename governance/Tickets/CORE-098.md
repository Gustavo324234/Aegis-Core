# CORE-098 — Investigación y Roadmap para LanceDB (VCM L3)

**Epic:** 35 — Hardening Post-Launch  
**Área:** `kernel/crates/ank-core/` — VCM  
**Agente:** Kernel Engineer  
**Prioridad:** P2 — Memoria a largo plazo  
**Estado:** TODO  
**Origen:** REC-010 / LIM-001

---

## Contexto

LanceDB lleva desactivado como capa L3 del VCM (Vector Cognitive Memory) por
"conflictos de compilación" sin fecha de resolución ni criterios técnicos definidos.
Sin L3, el VCM opera únicamente con L1 (contexto de ventana) y L2 (caché en
memoria), limitando la capacidad de memoria a largo plazo del sistema.

Este ticket es de **investigación** — no de implementación. El output es un análisis
técnico y una decisión documentada.

---

## Trabajo requerido

1. Ejecutar `cargo tree -p ank-core | grep lancedb` para identificar las dependencias
   en conflicto. Documentar cuáles crates son incompatibles y por qué (versiones de
   Arrow, conflictos de features, incompatibilidad con el target de compilación).

2. Evaluar alternativas a LanceDB para L3:
   - `qdrant-client` embebido (modo in-process)
   - `usearch` (librería de vector search en Rust puro)
   - `hnsw_rs` (implementación HNSW en Rust)
   - Mantener LanceDB pero aislar el conflicto via feature flag y resolver dependencia

3. Para cada alternativa, documentar:
   - Compatibilidad con el workspace actual
   - Madurez y mantenimiento del proyecto
   - API necesaria (insert, search por similitud coseno, delete by tenant)
   - Impacto en el tiempo de compilación

4. Documentar la decisión final en `governance/AEGIS_CONTEXT.md` como ADR-038
   ("Estrategia para VCM L3 post-launch") con la opción elegida y la razón.

5. Si la solución es simple (solo actualizar una versión de dependencia):
   implementarla directamente en este ticket y marcarlo DONE.

6. Si requiere trabajo significativo: crear un ticket separado `CORE-099` con
   la implementación, y marcar este ticket DONE con el ADR documentado.

---

## Criterios de aceptación

- [ ] `governance/AEGIS_CONTEXT.md` contiene ADR-038 con la decisión y justificación
- [ ] `governance/AEGIS_CONTEXT.md` LIM-001 actualizado: ya no dice "post-launch"
      indefinido — tiene un ticket o fecha asociada
- [ ] Si se implementó: `cargo build -p ank-core` con LanceDB (o alternativa) activado
      sin errores

---

## Dependencias

Ninguna.

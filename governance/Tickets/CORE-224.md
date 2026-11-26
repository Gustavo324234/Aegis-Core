# CORE-224 — Limpiar directorios temporales

**Tipo:** chore  
**Prioridad:** Baja  
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure  
**Estado:** ✅ Done
**Asignado a:** Tavo  

---

## Contexto

El espacio de trabajo contiene carpetas y archivos temporales residuales de builds y pruebas anteriores. El ticket `CORE-224` propone archivar las ramas y limpiar los directorios temporales como `scratch/` o similares.

## Estado actual

- `scratch/` sigue presente en el monorepo.
- Es necesario revisar si `scratch/` debe archivarse o si contiene scripts útiles como `cmr_playground.py` que deban ser formalizados.

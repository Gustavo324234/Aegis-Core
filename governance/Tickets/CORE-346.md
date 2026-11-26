# CORE-346 — Purge Physical Tenant Directory & L3 Swap on Delete

**Tipo:** fix
**Prioridad:** Crítica
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

Al eliminar un tenant mediante `MasterEnclave::delete_tenant`, el registro se borraba de la tabla `tenants` en `admin.db`, pero los archivos físicos en disco (`users/<tenant_id>` y `.aegis_swap/<tenant_id>`) permanecían intactos, permitiendo fugas de estado residual si se recreaba un tenant con el mismo identificador.

## Solución propuesta

1. Purgar el directorio físico `users/<tenant_id>` al eliminar un tenant en `MasterEnclave::delete_tenant`.
2. Purgar también el directorio de memoria vectorial L3 `.aegis_swap/<tenant_id>` para no dejar índices USearch ni fragmentos residuales en disco.

## Criterios de aceptación

- [x] `MasterEnclave::delete_tenant` elimina físicamente `users/<tenant_id>` y `.aegis_swap/<tenant_id>`.
- [x] Tests unitarios verifican la purga de directorios en disco tras eliminar un tenant.

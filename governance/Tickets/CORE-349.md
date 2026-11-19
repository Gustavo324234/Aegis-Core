# CORE-349 — Encrypt L3 Swap Fragments in SQLCipher and Fix Vector Dimension Initialization

**Tipo:** fix
**Prioridad:** Crítica
**Épica:** EPIC 49 — Cognitive Loop
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

1. **Violación Citadel (Datos en reposo sin cifrar) y escritura $O(N)$:** `LanceSwapManager` en `kernel/crates/ank-core/src/vcm/swap.rs` persistía todos los fragmentos de memoria semántica L3 del tenant en un archivo JSON en texto plano (`.aegis_swap/<tenant_id>/fragments.json`), reescribiendo todo el archivo en cada inserción.
2. **Bug de inicialización de dimensiones:** `store_fragment` invocaba `self.init_tenant(tenant_id)` *antes* de actualizar `self.dimension` con `vector.len()`. Si `dimension` era `0` (o si `search` se llamó antes con un vector dummy de 128 dimensiones sobre un índice vacío), el índice USearch quedaba fijado en 128 dimensiones y fallaba al insertar embeddings reales (ej. 384, 768 o 1536 dimensiones).

## Solución propuesta

1. Reemplazar la persistencia en texto plano `fragments.json` por una base de datos SQLCipher por tenant (`fragments.db`) cifrada con una llave derivada por tenant (`SHA-256(master_key : tenant_id)`), manteniendo migración automática y borrado seguro si existe un `fragments.json` heredado.
2. Persistir cada nuevo fragmento de manera incremental ($O(1)$ `INSERT OR REPLACE`) en lugar de serializar todo el `HashMap` en cada escritura.
3. Corregir el orden de inicialización de dimensiones en `store_fragment` y `search`, re-inicializando automáticamente el índice USearch si está vacío y recibe un vector de dimensión distinta, y evitando búsquedas sobre índices vacíos o con dimensión incompatible.

## Criterios de aceptación

- [x] Los fragmentos L3 se almacenan cifrados en SQLCipher (`fragments.db`) con inserción incremental $O(1)$.
- [x] Migración automática desde `fragments.json` legacy (si existe) eliminando el archivo en texto plano.
- [x] `store_fragment` soporta vectores de cualquier dimensión (ej. 384, 768, 1536) incluso tras llamadas previas a `search` con vectores de dimensión por defecto (128).
- [x] Tests unitarios cubriendo vectores de dimensión dinámica (384-d) y persistencia cifrada en disco.

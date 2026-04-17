# CORE-109 — Implementar usearch en LanceSwapManager (VCM L3)

**Epic:** 35 — Hardening Post-Launch  
**Área:** `kernel/crates/ank-core/` — VCM  
**Agente:** Kernel Engineer  
**Prioridad:** P2 — Memoria a largo plazo  
**Estado:** TODO  
**Origen:** CORE-098 — ADR-038  
**Depende de:** CORE-098 (investigación completada)

---

## Contexto

CORE-098 definió que `usearch` es la alternativa correcta para VCM L3. Este ticket implementa la integración.

---

## Trabajo requerido

1. Agregar `usearch = "2.24"` como dependencia en `kernel/crates/ank-core/Cargo.toml`

2. Modificar `src/vcm/swap.rs`:
   - Reemplazar los stubs de `LanceSwapManager` por implementación real con usearch
   - `init_tenant()`: Crear/recuperar index usearch por tenant en `data_dir/.aegis_swap/{tenant_id}`
   - `store_fragment()`: Usar `index.add()` con el vector cuantizado
   - `search()`: Usar `index.search()` y des-cuantizar resultados
   - Persistir el index via `index.save()` al cerrar o periódicamente

3. El error `SwapError::ConnectionError` debe renombrarse a `SwapError::IndexError`

4. Actualizar tests existentes en `swap.rs` para usar la implementación real

5. Verificar que `cargo build -p ank-core` compila sin errores

---

## Criterios de aceptación

- [ ] `cargo build -p ank-core` compila sin errores
- [ ] Tests en `src/vcm/swap.rs` pasan
- [ ] Tests en `src/vcm/mod.rs` pasan (VCM assemble con L3 real)

---

## Notas técnicas

```rust
// Estructura de persistencia:
// data_dir/.aegis_swap/{tenant_id}/memory.usearch
```


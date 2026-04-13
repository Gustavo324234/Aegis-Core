# CORE-078 — Fix: Documentar y remover `AEGIS_DEV_MASTER_BYPASS` de producción

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Kernel Engineer
**Prioridad:** 🟠 ALTA
**Estado:** TODO

---

## Contexto

En `kernel/crates/ank-core/src/enclave/master.rs`, la función
`authenticate_master` contiene un bypass de seguridad activable por variable
de entorno:

```rust
pub async fn authenticate_master(&self, username: &str, ...) -> Result<bool> {
    // ANK-SEC-BYPASS: Desarrollo / Emergencia
    if std::env::var("AEGIS_DEV_MASTER_BYPASS").unwrap_or_default() == "true"
        && username == "root"
    {
        tracing::warn!("ANK-SECURITY-WARNING: Master Admin BYPASS enabled for user 'root'.");
        return Ok(true);
    }
    // ... auth real ...
}
```

**Riesgos:**

1. **No documentado:** `AEGIS_DEV_MASTER_BYPASS` no aparece en `.env.example`,
   `AEGIS_CONTEXT.md`, ni en ningún README. Un contribuidor que copie un `.env`
   de dev a producción lo activa sin saberlo.

2. **Bypass total de Citadel:** Con esta variable activa, cualquier cliente que
   conozca que el admin se llama `"root"` puede ejecutar todas las operaciones
   admin (crear tenants, eliminarlos, resetear contraseñas) sin contraseña.

3. **Hardcodeo de `"root"`:** Incluso como herramienta de dev, está hardcodeado
   al nombre `"root"` — inútil si el admin tiene otro nombre.

4. **`tracing::warn` en lugar de `tracing::error`:** La severidad del log
   no refleja la gravedad del evento.

---

## Decisión de diseño

El bypass fue útil durante el desarrollo para evitar tener que conocer el hash
real. Ahora que el flujo de setup token (CORE-073, ANK-29-001) funciona y los
tests de integración en `master.rs` cubren el flujo completo, el bypass ya no
es necesario.

**Acción:** Eliminar completamente el bloque de bypass de producción.
Para desarrollo local, el `.env.example` documenta `AEGIS_ROOT_KEY` y el
flujo de setup token es suficiente.

---

## Cambios requeridos

### 1. `kernel/crates/ank-core/src/enclave/master.rs`

Eliminar el bloque completo:

```rust
// ELIMINAR — estas líneas:
if std::env::var("AEGIS_DEV_MASTER_BYPASS").unwrap_or_default() == "true"
    && username == "root"
{
    tracing::warn!("ANK-SECURITY-WARNING: Master Admin BYPASS enabled for user 'root'.");
    return Ok(true);
}
```

### 2. `installer/.env.example`

Asegurarse de que `AEGIS_DEV_MASTER_BYPASS` **no aparece** en el archivo de
ejemplo. Si existe, eliminarlo.

### 3. Comentario en `authenticate_master`

Agregar un comentario que deja constancia de la decisión:

```rust
// SECURITY: No development bypass is provided. Use the setup token flow
// (store_setup_token / validate_and_consume_setup_token) for first-time access.
// See ADR-023.
```

---

## Criterios de aceptación

- [ ] No existe ninguna referencia a `AEGIS_DEV_MASTER_BYPASS` en el codebase
- [ ] `grep -r "AEGIS_DEV_MASTER_BYPASS" .` retorna cero resultados
- [ ] `.env.example` no contiene `AEGIS_DEV_MASTER_BYPASS`
- [ ] `authenticate_master` no tiene ningún early-return que bypasee la validación
- [ ] `cargo build` pasa sin errores
- [ ] Los tests existentes en `master.rs` siguen pasando (no dependen del bypass)

---

## Dependencias

Ninguna. Cambio de eliminación pura, riesgo mínimo.
Los tests en `master.rs` usan `new_in_memory()` y flujo real — no dependen
del bypass.

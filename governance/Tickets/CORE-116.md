# CORE-116 — Verificar y documentar flujo SHA-256 → Argon2id en autenticación

**Epic:** Epic 35 — Hardening Pre-Launch
**Agente:** Kernel Engineer
**Prioridad:** 🟠 MEDIA — Seguridad / Claridad arquitectónica
**Estado:** TODO
**Origen:** REC-008 / Auditoría multi-modelo 2026-04-16

---

## Contexto

El Protocolo Citadel define que el passphrase viaja como SHA-256 en el transporte
hacia el kernel (`x-citadel-key` contiene el hash, no el plaintext). El kernel
luego verifica contra el hash almacenado en el enclave (Argon2id).

La pregunta crítica es: **¿qué recibe exactamente `authenticate_tenant`?**

- **Escenario A:** El kernel recibe el SHA-256 del passphrase y lo compara
  directamente contra el hash SHA-256 almacenado → Argon2id no se usa en la verificación.
- **Escenario B:** El kernel recibe el SHA-256 y aplica Argon2id sobre ese hash para
  comparar contra el hash Argon2id almacenado → funciona pero Argon2id protege
  un input de entropía fija (256 bits), no el passphrase directamente.
- **Escenario C (correcto):** El kernel recibe el plaintext, aplica SHA-256
  internamente si necesita y luego Argon2id → la capa de hash es una responsabilidad
  única del kernel.

Esta ambigüedad fue identificada en la auditoría arquitectónica como un punto
que requiere verificación explícita antes del lanzamiento.

**Archivos a revisar:**
- `kernel/crates/ank-core/src/enclave/master.rs` — `authenticate_tenant()`
- `kernel/crates/ank-http/src/citadel.rs` — `CitadelCredentials`, `hash_passphrase()`
- `kernel/crates/ank-http/src/ws/chat.rs` — `handle_chat()`, línea ~69

## Tarea del Kernel Engineer

Este ticket es de **verificación y documentación**, no necesariamente de cambio.

### Paso 1 — Leer el flujo completo

1. En `citadel.rs`: identificar qué hace `hash_passphrase()` y dónde se llama.
2. En `authenticate_tenant()`: identificar qué recibe el primer argumento
   (hash SHA-256 o plaintext) y qué operación se aplica.
3. Trazar el flujo completo desde el header `x-citadel-key` hasta la comparación
   en el enclave.

### Paso 2 — Documentar con comentarios `// SECURITY:` en el código

Una vez verificado el flujo, agregar comentarios explícitos:

```rust
// SECURITY: x-citadel-key contiene SHA-256(passphrase_plaintext).
// authenticate_tenant recibe el hash y lo verifica contra el valor
// almacenado con Argon2id. El passphrase plaintext nunca viaja al kernel.
// Entropía efectiva: 2^256 (SHA-256 output space) — no hay pérdida de
// seguridad práctica vs. enviar el plaintext directamente.
pub async fn authenticate_tenant(
    &self,
    tenant_id: &str,
    session_key_hash: &str,
) -> Result<bool> {
```

### Paso 3 — Si se detecta un bug

Si el flujo real no coincide con el diseño documentado en `AEGIS_CONTEXT.md`,
reportar el bug con el fix específico. El fix debe asegurar que:
- El kernel nunca almacena passwords en texto plano
- La verificación usa Argon2id correctamente
- El formato del `session_key_hash` que llega al enclave es consistente con
  el formato almacenado

### Paso 4 — Actualizar `AEGIS_CONTEXT.md`

Agregar en la sección "Protocolo Citadel" una descripción explícita del flujo
de hashing con el formato exacto en cada etapa:

```
Cliente → SHA-256(passphrase) → header x-citadel-key
Kernel  → recibe SHA-256 → [operación X] → compara contra enclave
Enclave → almacena [formato Y] generado en create_tenant
```

## Criterios de aceptación

- [ ] El Kernel Engineer documenta el flujo real en un comentario en el código
- [ ] Si hay bug: el fix está implementado y `cargo build` pasa
- [ ] Si no hay bug: `AEGIS_CONTEXT.md` tiene la sección de flujo de hashing actualizada
- [ ] El comentario `// SECURITY:` explica la decisión de diseño sin ambigüedad
- [ ] `cargo build` pasa sin errores

## Dependencias

Ninguna. Es una tarea de auditoría de código con posible fix pequeño.

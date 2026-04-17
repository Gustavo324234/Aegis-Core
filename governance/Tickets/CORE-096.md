# CORE-096 — Verificar y Documentar el Flujo SHA-256 + Argon2id en Citadel

**Epic:** 35 — Hardening Post-Launch  
**Área:** `kernel/crates/ank-core/src/enclave/master.rs` + `kernel/crates/ank-http/src/citadel.rs`  
**Agente:** Kernel Engineer  
**Prioridad:** P1 — Seguridad  
**Estado:** TODO  
**Origen:** REC-008 / claude-sonnet-4-6 sección 3.2

---

## Contexto

El Protocolo Citadel define que `ank-http` aplica SHA-256 al passphrase antes de
pasarlo al enclave para validación. La pregunta es: ¿el enclave almacena y verifica
contra `Argon2id(passphrase_original)` o contra `Argon2id(sha256(passphrase))`?

Si es lo segundo, Argon2id protege un input de 256 bits de entropía fija (el hash
SHA-256), no el passphrase original. Esto no es inseguro en la práctica, pero es
una inconsistencia arquitectónica que debe documentarse explícitamente para evitar
confusión en contribuidores futuros.

---

## Cambios requeridos

1. Leer `authenticate_tenant` en `master.rs` y `hash_passphrase` en `citadel.rs`.
   Determinar exactamente qué recibe el enclave: ¿el plaintext o el SHA-256?

2. **Si el flujo es correcto** (Argon2id sobre SHA-256, comportamiento intencional):
   - Agregar un comentario `// SECURITY: passphrase arrives pre-hashed as SHA-256.`
     `// Argon2id is applied over the hash. This is intentional — see AEGIS_CONTEXT.md §4.`
     en `authenticate_tenant`.
   - Actualizar `governance/AEGIS_CONTEXT.md` sección del Protocolo Citadel para
     documentar explícitamente: "El enclave recibe SHA-256(passphrase) y aplica
     Argon2id sobre ese valor."

3. **Si hay un bug** (el enclave recibe plaintext cuando debería recibir SHA-256, o
   viceversa): corregirlo y documentarlo en el commit con referencia a este ticket.

4. Verificar que el mismo comportamiento aplica al flujo WebSocket (subprotocol
   `session-key.<value>`) — confirmar qué valor viaja en `<value>`.

---

## Criterios de aceptación

- [ ] El código de `authenticate_tenant` tiene un comentario `// SECURITY:` que
      documenta qué recibe (plaintext o SHA-256)
- [ ] `governance/AEGIS_CONTEXT.md` sección Protocolo Citadel describe el flujo
      exacto de hashing sin ambigüedad
- [ ] Si se detectó y corrigió un bug: el test de autenticación existente pasa
- [ ] `cargo build -p ank-core` sin errores ni warnings de clippy

---

## Dependencias

Ninguna.

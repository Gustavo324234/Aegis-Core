# CORE-094 — Reemplazar std::sync::Mutex por tokio::sync::Mutex en CognitiveHAL

**Epic:** 35 — Hardening Post-Launch  
**Área:** `kernel/crates/ank-core/src/chal/mod.rs`  
**Agente:** Kernel Engineer  
**Prioridad:** P1 — Estabilidad  
**Estado:** TODO  
**Origen:** REC-006 / minimax-m2.5-free

---

## Contexto

`CognitiveHAL` usa `std::sync::Mutex` en un contexto async de Tokio (línea ~87).
Mantener un `std::sync::Mutex` bloqueado a través de un punto `.await` produce un
deadlock: el hilo del runtime de Tokio queda bloqueado esperando el lock mientras
otro task en el mismo hilo también espera. Clippy con `--workspace` puede no
detectar este patrón si el lock se toma y libera antes del `.await`, pero es
frágil ante futuros refactors.

---

## Cambios requeridos

1. Identificar todos los usos de `std::sync::Mutex` y `std::sync::RwLock` en
   `ank-core/src/chal/mod.rs`.

2. Para cada mutex que protege datos accedidos desde código async:
   - Reemplazar `std::sync::Mutex<T>` → `tokio::sync::Mutex<T>`
   - Reemplazar `std::sync::RwLock<T>` → `tokio::sync::RwLock<T>`
   - Actualizar los `.lock()` sincrónicos a `.lock().await`

3. Verificar que no haya otros usos de `std::sync::Mutex` en el resto de `ank-core`
   en contextos async. Si los hay, corregirlos en el mismo commit.

4. Excepción válida: `std::sync::Mutex` es correcto si el lock se toma y libera
   dentro de un bloque síncrono sin ningún `.await` intermedio. En ese caso, agregar
   un comentario `// SYNC: lock held only in sync context, no .await within guard`.

---

## Criterios de aceptación

- [ ] No hay `std::sync::Mutex` ni `std::sync::RwLock` en código async de `chal/mod.rs`
      sin el comentario de justificación
- [ ] `cargo build -p ank-core` sin errores ni warnings de clippy
- [ ] Sin `.unwrap()` ni `.expect()` en código nuevo o modificado

---

## Dependencias

Ninguna.

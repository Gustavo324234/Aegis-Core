# CORE-113 — Reemplazar `std::sync::Mutex` por `tokio::sync::Mutex` en `CognitiveHAL`

**Epic:** Epic 35 — Hardening Pre-Launch
**Agente:** Kernel Engineer
**Prioridad:** 🟠 MEDIA-ALTA — Estabilidad
**Estado:** TODO
**Origen:** REC-006 / Auditoría multi-modelo 2026-04-16

---

## Contexto

`CognitiveHAL` (o sus campos internos) usa `std::sync::Mutex` en un contexto
async de Tokio. Esto es problemático porque `std::sync::Mutex` es un mutex
bloqueante: si un task de Tokio entra al lock y luego hace `.await`, el hilo
del runtime queda bloqueado hasta que se libere el lock. Bajo carga concurrente,
esto puede saturar los hilos del runtime y producir deadlocks.

**Archivo afectado:** `kernel/crates/ank-core/src/chal/mod.rs` línea ~87

El patrón correcto en código async es usar `tokio::sync::Mutex` (o
`tokio::sync::RwLock` si el acceso es mayoritariamente de lectura), que
suspende el task en lugar de bloquear el hilo.

## Cambios requeridos

**Archivo:** `kernel/crates/ank-core/src/chal/mod.rs`

### Identificar el Mutex afectado

Buscar en `chal/mod.rs` usos del patrón:
```rust
use std::sync::Mutex;
// o
std::sync::Mutex::new(...)
```

### Reemplazar por `tokio::sync::RwLock` si el acceso es read-heavy

```rust
// Antes
use std::sync::Mutex;
struct CognitiveHAL {
    some_field: Mutex<SomeType>,
}

// Después
use tokio::sync::RwLock;
struct CognitiveHAL {
    some_field: RwLock<SomeType>,
}
```

Para lecturas:
```rust
// Antes
let guard = self.some_field.lock().unwrap();

// Después
let guard = self.some_field.read().await;
```

Para escrituras:
```rust
// Antes
let mut guard = self.some_field.lock().unwrap();

// Después
let mut guard = self.some_field.write().await;
```

### Si el acceso es write-heavy o requiere exclusión mutua estricta

Usar `tokio::sync::Mutex`:
```rust
use tokio::sync::Mutex;
// ... mismo patrón pero con lock().await
```

### Verificar que no queden `.unwrap()` en el lock

`tokio::sync::Mutex::lock()` retorna directamente el guard (no `Result`),
por lo que no necesita `.unwrap()`:
```rust
let guard = self.some_field.lock().await; // No .unwrap()
```

## Criterios de aceptación

- [ ] No hay `std::sync::Mutex` usado con `.await` en el codebase de `ank-core`
- [ ] El HAL puede manejar requests concurrentes sin deadlock bajo carga
- [ ] `cargo build` pasa sin errores
- [ ] `cargo clippy` no reporta `clippy::await_holding_lock` ni warnings relacionados
- [ ] No regresión en los tests existentes

## Dependencias

Puede implementarse junto con CORE-112 ya que ambos tocan `chal/mod.rs`.

# CORE-021 — aegis-supervisor: portado y actualizado para un solo proceso

**Épica:** 32 — Unified Binary
**Fase:** 3 — Entrypoint unificado
**Repo:** Aegis-Core — `kernel/crates/aegis-supervisor/`
**Asignado a:** Kernel Engineer
**Prioridad:** 🟡 Media
**Estado:** COMPLETED
**Depende de:** CORE-020

---

## Contexto

En el legacy, el supervisor levanta dos procesos: `ank-server` y `python uvicorn`.
En Aegis-Core, el supervisor levanta **un solo proceso**: `ank-server`.

El BFF Python desaparece. `start_shell()` se elimina. El supervisor simplifica.

**Referencia:** `Aegis-ANK/crates/aegis-supervisor/src/`

---

## Cambios respecto al legacy

| Aspecto | Legacy | Aegis-Core |
|---|---|---|
| Procesos gestionados | `ank-server` + `python uvicorn` | Solo `ank-server` |
| Health check | gRPC :50051 + HTTP :8000 | Un solo check: HTTP :8000/health |
| `start_shell()` | Levanta uvicorn | **Eliminado** |
| `SupervisorConfig.bff_dir` | Path al BFF Python | **Eliminado** |
| Restart logic | Si ANK cae → reiniciar todo | Si ank-server cae → reiniciar |

---

## Trabajo requerido

Portar `Aegis-ANK/crates/aegis-supervisor/src/` con las simplificaciones arriba.

### `SupervisorConfig` simplificado

```rust
pub struct SupervisorConfig {
    pub ank_bin: PathBuf,   // path al binario ank-server
    pub data_dir: PathBuf,
    pub root_key: String,
    pub port: u16,          // antes port_ank + port_shell, ahora uno solo
    pub dev_mode: bool,
}
```

### Health check unificado

```rust
pub async fn health_check(&self) -> bool {
    // Un solo check: GET /health en el puerto configurado
    reqwest::get(format!("http://localhost:{}/health", self.config.port))
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}
```

---

## Criterios de aceptación

- [x] `cargo build -p aegis-supervisor` compila sin errores
- [x] `aegis start` levanta un solo proceso `ank-server`
- [x] `aegis status` muestra UP/DOWN basado en `/health`
- [x] `aegis stop` termina el proceso limpiamente
- [x] `aegis dev` levanta en modo dev (`AEGIS_MTLS_STRICT=false`, `DEV_MODE=true`)
- [x] No hay referencias a Python, uvicorn, ni BFF en el código
- [x] `cargo clippy -p aegis-supervisor -- -D warnings -D clippy::unwrap_used` → 0 warnings

## Referencia

`Aegis-ANK/crates/aegis-supervisor/src/` — portar y simplificar

# CORE-080 — Fix: gRPC `server.rs` tiene ~15 métodos `unimplemented!()` que deberían funcionar

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Kernel Engineer
**Prioridad:** 🟠 ALTA
**Estado:** TODO

---

## Contexto

`kernel/crates/ank-server/src/server.rs` implementa el `KernelService` gRPC.
La mayoría de los métodos del servicio retornan `Status::unimplemented`:

```rust
async fn reset_tenant_password(...) -> ... {
    Err(Status::unimplemented("Not implemented"))
}
async fn configure_engine(...) -> ... {
    Err(Status::unimplemented("Not implemented"))
}
async fn list_tenants(...) -> ... {
    Err(Status::unimplemented("Not implemented"))
}
async fn delete_tenant(...) -> ... {
    Err(Status::unimplemented("Not implemented"))
}
async fn add_global_key(...) -> ... {
    Err(Status::unimplemented("Not implemented"))
}
async fn list_global_keys(...) -> ... {
    Err(Status::unimplemented("Not implemented"))
}
async fn delete_key(...) -> ... {
    Err(Status::unimplemented("Not implemented"))
}
async fn list_my_keys(...) -> ... {
    Err(Status::unimplemented("Not implemented"))
}
async fn sync_router_catalog(...) -> ... {
    Err(Status::unimplemented("Not implemented"))
}
async fn list_router_models(...) -> ... {
    Err(Status::unimplemented("Not implemented"))
}
async fn get_siren_config(...) -> ... {
    Err(Status::unimplemented("Not implemented"))
}
async fn set_siren_config(...) -> ... {
    Err(Status::unimplemented("Not implemented"))
}
```

**Contexto de la consolidación:** Durante la migración a monorepo (Epic 32),
el servidor gRPC Tonic fue portado como stub mínimo. Toda la lógica real fue
implementada en `ank-http` (Axum). La CLI (`ank-cli`) y cualquier integración
externa que use gRPC directamente recibe `UNIMPLEMENTED` para la mayoría de
operaciones.

**Impacto real actual:** La UI web usa HTTP/WS (`ank-http`) exclusivamente,
por lo que estos `unimplemented!` no bloquean el flujo principal. Sin embargo:
- `ank-cli` no puede listar tenants, reset passwords, ni gestionar keys via gRPC
- Multi-nodo (swarm) requiere gRPC funcional entre nodos
- Es deuda técnica que crece si no se aborda antes del lanzamiento público

---

## Priorización

No todos los métodos tienen la misma urgencia. Clasificación:

### Prioridad 1 — Bloquean funcionalidad existente
| Método | Impacto |
|--------|---------|
| `reset_tenant_password` | CLI no puede resetear passwords |
| `list_tenants` | CLI no puede listar tenants |
| `delete_tenant` | CLI no puede eliminar tenants |

### Prioridad 2 — Necesarios para router/keys via CLI
| Método | Impacto |
|--------|---------|
| `add_global_key` | CLI no puede agregar keys |
| `list_global_keys` | CLI no puede listar keys |
| `delete_key` | CLI no puede eliminar keys |
| `list_my_keys` | CLI no puede listar keys propias |
| `sync_router_catalog` | CLI no puede sincronizar catálogo |
| `list_router_models` | CLI no puede listar modelos |

### Prioridad 3 — Post-lanzamiento
| Método | Impacto |
|--------|---------|
| `configure_engine` | CLI no puede configurar engine |
| `get_siren_config` / `set_siren_config` | CLI no puede configurar voz |
| `teleport_process` | Multi-nodo |

---

## Cambios requeridos

**Archivo:** `kernel/crates/ank-server/src/server.rs`

Para cada método de Prioridad 1 y 2, implementar delegando al mismo
`AppState` que ya usa `ank-http`. El estado ya tiene `citadel`, `router`,
`persistence` y `catalog_syncer` disponibles.

Patrón de implementación (ejemplo `reset_tenant_password`):

```rust
async fn reset_tenant_password(
    &self,
    request: Request<ank_proto::v1::PasswordResetRequest>,
) -> Result<Response<Empty>, Status> {
    let auth = request.extensions().get::<CitadelAuth>()
        .cloned()
        .ok_or_else(|| Status::unauthenticated("Missing Citadel context"))?;
    self.validate_auth(&auth).await?;

    let req = request.into_inner();
    let citadel = self.state.citadel.lock().await;
    citadel
        .enclave
        .reset_tenant_password(&req.tenant_id, &req.new_passphrase)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    Ok(Response::new(Empty {}))
}
```

---

## Criterios de aceptación

- [ ] Los 7 métodos de Prioridad 1 y 2 tienen implementación real (no `unimplemented!`)
- [ ] `ank-cli` puede ejecutar: `list-tenants`, `reset-password`, `delete-tenant`,
  `add-key`, `list-keys`, `delete-key`, `sync-catalog` sin error UNIMPLEMENTED
- [ ] Los métodos de Prioridad 3 pueden mantener `unimplemented!` con comentario
  `// TODO(CORE-080-P3): post-launch`
- [ ] `cargo build` pasa sin errores

---

## Dependencias

El `AppState` ya tiene todos los recursos necesarios. No requiere cambios en
`ank-core` ni en `ank-http`.

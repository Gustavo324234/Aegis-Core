# CORE-088 — Fix: `ChatTerminal` envía `session_key` en FormData de file upload

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Shell Engineer
**Prioridad:** 🟠 ALTA
**Estado:** DONE

---

## Contexto

`ChatTerminal.tsx` envía archivos al workspace via `multipart/form-data`.
El FormData incluye la contraseña del usuario como campo de texto plano:

```typescript
// handleFileUploadChange y handleDrop — ambos tienen el mismo patrón:
const formData = new FormData();
formData.append('tenant_id', tenantId);
formData.append('session_key', sessionKey);  // ← contraseña en multipart
formData.append('file', file);

const response = await fetch('/api/workspace/upload', {
    method: 'POST',
    body: formData,
});
```

**El backend (`workspace.rs`) lee `session_key` del multipart body** — esta
es la excepción donde el backend sí espera las credenciales en el body,
porque `multipart/form-data` no permite mezclar headers de auth con archivos
de la forma estándar.

Sin embargo, hay una solución limpia: el endpoint puede leer `x-citadel-tenant`
y `x-citadel-key` de los **HTTP headers** mientras sigue recibiendo el archivo
en el multipart. Los headers HTTP coexisten con cualquier `Content-Type` del body.

---

## Cambios requeridos

### Shell — `shell/ui/src/components/ChatTerminal.tsx`

Migrar a headers Citadel en el upload:

```typescript
const formData = new FormData();
// ELIMINAR: formData.append('tenant_id', tenantId);
// ELIMINAR: formData.append('session_key', sessionKey);
formData.append('file', file);

const response = await fetch('/api/workspace/upload', {
    method: 'POST',
    headers: {
        'x-citadel-tenant': tenantId,
        'x-citadel-key': sessionKey,
    },
    body: formData,
    // No incluir 'Content-Type' — el browser lo setea automáticamente con el boundary
});
```

Aplicar el cambio en los **dos lugares**: `handleFileUploadChange` y `handleDrop`.

### Kernel — `kernel/crates/ank-http/src/routes/workspace.rs`

Actualizar el handler para leer auth de headers en lugar del multipart:

```rust
async fn upload(
    State(state): State<AppState>,
    auth: CitadelAuthenticated,   // ← extractor de headers
    mut multipart: Multipart,
) -> Result<Json<Value>, AegisHttpError> {
    // Eliminar extracción manual de tenant_id y session_key del multipart
    // Usar auth.tenant_id directamente

    let mut file_data = None;
    let mut original_name = None;

    while let Some(field) = multipart.next_field().await ... {
        let name = field.name().unwrap_or("").to_string();
        // Solo procesar el campo "file"
        if name == "file" {
            original_name = field.file_name()...;
            file_data = Some(field.bytes().await...);
        }
    }
    // Usar auth.tenant_id para el path del workspace
    let base = state.config.data_dir
        .join("users")
        .join(&auth.tenant_id)
        .join("workspace");
    // ...
}
```

---

## Criterios de aceptación

- [x] `ChatTerminal` no incluye `session_key` en el FormData del upload
- [x] El upload usa headers `x-citadel-tenant` / `x-citadel-key`
- [x] `workspace.rs` extrae auth de headers via `CitadelAuthenticated`
- [x] El upload de archivos funciona correctamente end-to-end
- [x] `cargo build` y `npm run build` pasan sin errores

---

## Dependencias

Cambio coordinado Shell + Kernel — implementar ambos en el mismo commit/PR.

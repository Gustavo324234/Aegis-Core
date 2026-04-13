# CORE-082 — Fix: `auth_interceptor` gRPC no falla si faltan headers Citadel

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Kernel Engineer
**Prioridad:** 🟡 MEDIA
**Estado:** TODO

---

## Contexto

El interceptor de autenticación gRPC en `server.rs`:

```rust
pub fn auth_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    let metadata = req.metadata();

    let tenant_id = match metadata.get("x-citadel-tenant") {
        Some(v) => v.to_str().map(|s| s.to_string()).unwrap_or_default(),
        None => return Ok(req),   // ← DEJA PASAR sin autenticar
    };

    let session_key = match metadata.get("x-citadel-key") {
        Some(v) => v.to_str().map(|s| s.to_string()).unwrap_or_default(),
        None => return Ok(req),   // ← DEJA PASAR sin autenticar
    };
    // ...
}
```

Cuando un cliente gRPC llama sin los headers `x-citadel-tenant` o
`x-citadel-key`, el interceptor retorna `Ok(req)` — la request **pasa al
handler sin ningún contexto de autenticación**.

Los handlers individuales protegidos hacen:
```rust
let auth = request.extensions().get::<CitadelAuth>()
    .ok_or_else(|| Status::unauthenticated("Citadel Protocol context missing"))?;
```

Esto funciona para la mayoría — devuelven `UNAUTHENTICATED` si falta el
contexto. Pero `get_system_status` y `initialize_master_admin` **no requieren
auth** y procesan requests sin headers. Eso está bien para esos métodos.

**El problema real:** El comentario de intención dice que el interceptor
"permite pasar" requests sin auth para que los handlers decidan. Pero el
patrón actual mezcla dos responsabilidades: el interceptor debería solo
extraer y adjuntar credenciales, y los handlers decidir si requieren auth.
Si se agrega un handler nuevo y se olvida el check de `CitadelAuth`, pasa
sin autenticación.

Adicionalmente: si `x-citadel-tenant` está presente pero `x-citadel-key`
no, el interceptor retorna `Ok(req)` sin adjuntar ningún `CitadelAuth` —
comportamiento inconsistente (headers parciales se ignoran silenciosamente).

---

## Cambios requeridos

**Archivo:** `kernel/crates/ank-server/src/server.rs`

### Lógica corregida del interceptor

```rust
pub fn auth_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    let metadata = req.metadata();

    let tenant_id = metadata.get("x-citadel-tenant")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let session_key = metadata.get("x-citadel-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Si ambos headers están presentes, adjuntar CitadelAuth
    // Si ninguno está presente, dejar pasar (para endpoints públicos)
    // Si solo uno está presente, es un error de protocolo
    match (tenant_id, session_key) {
        (Some(tid), Some(key)) => {
            let hash = ank_http::citadel::hash_passphrase(&key);
            let mut req = req;
            req.extensions_mut().insert(CitadelAuth {
                tenant_id: tid,
                session_key: hash,
                public_id: "obfuscated".to_string(),
            });
            Ok(req)
        }
        (None, None) => Ok(req), // Request pública, handler decide si requiere auth
        _ => Err(Status::unauthenticated(
            "Citadel Protocol violation: partial credentials"
        )),
    }
}
```

---

## Criterios de aceptación

- [ ] Un request sin ningún header Citadel pasa al handler (para endpoints públicos)
- [ ] Un request con ambos headers adjunta `CitadelAuth` correctamente
- [ ] Un request con solo uno de los dos headers retorna `UNAUTHENTICATED`
- [ ] `cargo build` pasa sin errores
- [ ] Los tests existentes de gRPC siguen pasando

---

## Dependencias

Ninguna.

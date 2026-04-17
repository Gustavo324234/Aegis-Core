# CORE-107 — Documentación OpenAPI/Swagger para endpoints HTTP

**Epic:** 35 — Hardening Post-Launch  
**Área:** `kernel/crates/ank-http/`  
**Agente:** Kernel Engineer  
**Prioridad:** P2 — Onboarding contribuidores  
**Estado:** TODO  
**Origen:** REC-014 / Gemini 3 Flash

---

## Contexto

No existe documentación de API autogenerada. Los contribuidores externos deben leer
el código Axum para entender los endpoints disponibles, sus parámetros y respuestas.
Para un proyecto open source esto es una barrera de adopción significativa.

---

## Cambios requeridos

1. Agregar dependencias en `ank-http/Cargo.toml`:

   ```toml
   utoipa = { version = "4", features = ["axum_extras"] }
   utoipa-swagger-ui = { version = "7", features = ["axum"] }
   ```

2. Anotar los siguientes endpoints con `#[utoipa::path]` (orden de prioridad):
   - `POST /api/auth/login`
   - `POST /api/admin/tenant`, `GET /api/admin/tenants`, `DELETE /api/admin/tenant/:id`
   - `GET /api/engine/status`, `POST /api/engine/configure`
   - `GET /api/router/models`, `GET /api/router/keys/*`
   - `GET /api/status`

3. Agregar la ruta `GET /api/docs` con Swagger UI integrado.

4. Los tipos de request/response deben derivar `utoipa::ToSchema` donde aplique.
   Usar `#[schema(format = "password")]` para campos de credenciales.
   Nunca incluir credenciales reales en los ejemplos de esquema.

5. No es necesario cubrir todos los endpoints en este ticket. Al menos los
   listados en el punto 2 deben estar documentados.

---

## Criterios de aceptación

- [ ] `GET /api/docs` devuelve Swagger UI con los endpoints anotados
- [ ] Al menos los 9 endpoints de la lista tienen documentación generada
- [ ] Los campos de credenciales usan `#[schema(format = "password")]` o equivalente
- [ ] `cargo build -p ank-http` sin errores ni warnings de clippy
- [ ] Sin `.unwrap()` ni `.expect()` en código nuevo

---

## Dependencias

Ninguna.

# CORE-311 — Orion ID login tab and automatic domain resolution

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** EPIC 55 — Mobile App (Orion ID & Web Redirection)  
**Estado:✅ Done
**Asignado a:** App Developer  

---

## Contexto

El cliente móvil necesita poder iniciar sesión contra Orion ID y resolver automáticamente el dominio y la dirección local del kernel para conectarse como satélite.

## Cambios Realizados

- Pantalla y flujo de login de Orion ID en la aplicación móvil React Native.
- Mecanismo de resolución automática de dominio basado en el token de Orion ID.
- Almacenamiento seguro del token cifrado de sesión en `SecureStore`.

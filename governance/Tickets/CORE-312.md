# CORE-312 — connected-accounts Web redirect button via expo-web-browser

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** EPIC 55 — Mobile App (Orion ID & Web Redirection)  
**Estado:✅ Done
**Asignado a:** App Developer  

---

## Contexto

Para completar vinculaciones complejas de cuentas externas (Spotify, etc.) en mobile, se requiere redirigir al usuario de forma segura a una vista web integrada utilizando `expo-web-browser`.

## Cambios Realizados

- Integración del botón de vinculación en la sección de cuentas conectadas.
- Apertura segura del flujo OAuth en el navegador del sistema (`expo-web-browser`).
- Control e intercepción del callback deep link para consolidar la sesión en la app.

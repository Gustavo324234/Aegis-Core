# CORE-310 — Fix TypeScript Icon type compiler error (Spotify icon)

**Tipo:** fix  
**Prioridad:** Alta  
**Épica:** EPIC 55 — Mobile App (Orion ID & Web Redirection)  
**Estado:✅ Done
**Asignado a:** UI Engineer  

---

## Contexto

El compilador de TypeScript en la app de React Native (Expo) reportaba un error de tipo al renderizar el ícono de Spotify en las cuentas vinculadas, bloqueando el build en CI.

## Cambios Realizados

- Solucionado error de tipos en la definición del prop `name` en el componente de íconos.
- Importación y mapeo correcto del asset vectorial SVG para Spotify.
- Confirmación de compilación exitosa (`tsc --noEmit`) en la app móvil.

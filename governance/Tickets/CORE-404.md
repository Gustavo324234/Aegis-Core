# CORE-404: Optimización Frontend (`shell/ui`: selectores `useAegisStore`, `React.lazy()`, optimización de SVGs y `apiClient.ts`)

- **Tipo:** perf
- **Estado:** 📥 Todo
- **Prioridad:** Media
- **Épica:** EPIC 61
- **Asignado a:** Shell Engineer

---

## Description

Mejorar el rendimiento y mantenibilidad del cliente web React (`shell/ui`):
- Reemplazar las 35 llamadas sin selector `useAegisStore()` por selectores granulares (`useAegisStore(s => ...)` / `useShallow`) para evitar re-renders masivos durante streaming y telemetría.
- Introducir `React.lazy()` + `Suspense` en `App.tsx` (y/o chunking en `vite.config.ts`) para dividir el chunk monolítico de ~1.04 MB.
- Optimizar/comprimir los assets SVG de logo (`aegis_logo.svg` y `logo_icon.svg`, actualmente ~234 KB combinados).
- Crear un `apiClient.ts` tipado centralizado con inyección de credenciales Citadel, manejo uniforme de 401/403/429 y cancelación vía `AbortController`.

## Acceptance Criteria

- [ ] Ningún componente suscribe todo el store con `useAegisStore()` sin selector.
- [ ] Chunk principal de Vite por debajo del umbral de 500 KB o dividido por rutas/paneles con `React.lazy()`.
- [ ] `npm run lint` y `npm run build` pasan con exit code `0`.

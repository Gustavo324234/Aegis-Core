# CORE-404: Optimización Frontend (`shell/ui`: selectores `useAegisStore`, `React.lazy()`, optimización de SVGs y `apiClient.ts`)

- **Tipo:** perf
- **Estado:** ✅ Done
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

- [x] Ningún componente suscribe todo el store con `useAegisStore()` sin selector.
- [x] Chunk principal de Vite por debajo del umbral de 500 KB o dividido por rutas/paneles con `React.lazy()`.
- [x] `npm run lint` y `npm run build` pasan con exit code `0`.

## Verification

- Llamadas bare `useAegisStore()` en `shell/ui/src`: **0** (35/35 migradas a selectores directos / `useShallow` en 30 archivos; `App.tsx` y `ConnectionQR.tsx` usan `apiGet` + `AbortController` vía `src/utils/apiClient.ts`).
- `npm run lint` (`eslint . --ext ts,tsx --max-warnings 0`): OK (exit `0`, 0 warnings).
- `npm run build` (`tsc && vite build`): OK (exit `0`, sin advertencias de chunks > 500 kB; chunk inicial `index` reducido de `1,040.21 kB` a `67.18 kB`; chunk JS más grande `markdown-vendor` `153.42 kB`; `SettingsPanel` `109.01 kB`, `Dashboard` `63.19 kB`, `ModulesAppHub` `61.09 kB`, `VideoStudioPanel` `52.22 kB`).
- Assets SVG optimizados: `logo_icon.svg` (`125.77 kB` → `16.27 kB`) y `aegis_logo.svg` (`108.11 kB` → `27.38 kB`).

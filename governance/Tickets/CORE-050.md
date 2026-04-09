# CORE-050 — app: setup React Native + Expo SDK 52

**Épica:** 32 — Unified Binary
**Fase:** 6 — Mobile App
**Repo:** Aegis-Core — `app/`
**Asignado a:** Mobile Engineer
**Prioridad:** 🟡 Media
**Estado:** DONE
**Depende de:** CORE-001

---

## Contexto

Inicializar el proyecto mobile en `app/` con el mismo stack del legacy.
La app es idéntica al legacy — habla con `ank-server` HTTP/WS en modo
Satellite, y con providers cloud directamente en modo Cloud.
El cambio de BFF Python a `ank-server` es transparente para la app
porque los endpoints son exactamente los mismos.

**Referencia:** `Aegis-App/package.json`, `app.json`, `tsconfig.json`

---

## Trabajo requerido

Portar la configuración base desde `Aegis-App/`:
- `package.json`
- `app.json`
- `tsconfig.json`
- `eas.json`
- `expo-env.d.ts`
- `.eslintrc.js`

---

## Criterios de aceptación

- [ ] `npm install` en `app/` termina sin errores
- [ ] `npx expo start` inicia el servidor de desarrollo
- [ ] `npx expo export` genera el bundle sin errores TypeScript

## Referencia

`Aegis-App/` — configuración base a portar

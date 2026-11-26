# CORE-231 — fix(shell): Micrófono falla silenciosamente en HTTP — falta feedback al usuario

**Tipo:** fix  
**Prioridad:** Crítica  
**Asignado a:** Shell Engineer  
**Epic:** Bugs pre-lanzamiento  
**Estado:** ✅ Completo

---

## Contexto

El micrófono no funciona cuando el usuario accede por IP directa (HTTP) en lugar de
Cloudflare (HTTPS). El navegador bloquea `getUserMedia()` y `WebSpeech API` en contextos
no seguros (HTTP sobre IP no-localhost).

El código en `useAegisStore.ts` (`startSirenStream`) hace:
```typescript
const stream = await navigator.mediaDevices.getUserMedia({ audio: ... });
```
Esto lanza una excepción `NotAllowedError` o `NotSupportedError` que el catch captura
y solo hace `set({ isRecording: false })` sin mostrar ningún mensaje al usuario. El
micrófono simplemente "no pasa nada" al hacer click.

Hay un warning de HTTPS en `VozTab` del `SettingsPanel`, pero ese warning no aparece
donde el usuario hace click en el botón del micrófono.

## Cambios requeridos

### 1. Mostrar error visible en `InputModeSelector` cuando el mic falla por HTTPS

**Archivo:** `shell/ui/src/components/InputModeSelector.tsx`

Cuando `startSirenStream()` lanza una excepción:

- Si el error es `NotAllowedError` → mostrar: "El navegador bloqueó el micrófono.
  Verificá los permisos de esta página."
- Si `window.location.protocol === 'http:'` y el hostname no es localhost/127.0.0.1 →
  mostrar inline: "El micrófono requiere HTTPS. Accedé via el link de Cloudflare."
- Si `SpeechRecognitionCtor` es `undefined` → mostrar: "Tu navegador no soporta voz.
  Usá Chrome o Edge."

El mensaje debe ser visible en el chat, no solo en la consola.

### 2. Detectar contexto inseguro al activar modo audio

**Archivo:** `shell/ui/src/store/useAegisStore.ts`

Al inicio de `startSirenStream()`, antes de cualquier llamada a `getUserMedia`,
agregar:

```typescript
const isInsecureContext = 
  window.location.protocol === 'http:' && 
  !['localhost', '127.0.0.1'].includes(window.location.hostname);

if (isInsecureContext) {
  throw new Error('INSECURE_CONTEXT: Microphone requires HTTPS');
}
```

El llamador (`InputModeSelector`) debe capturar este error y mostrar el mensaje apropiado.

### 3. Persistir `sttProvider` en el store

**Archivo:** `shell/ui/src/store/useAegisStore.ts`

El `sttProvider` no está en la lista `partialize` del `persist` middleware, por lo que
se resetea a `'browser'` en cada recarga, ignorando la configuración del usuario.

Agregar `sttProvider` al objeto retornado por `partialize`:

```typescript
partialize: (state) => ({
    // ... campos existentes ...
    sttProvider: state.sttProvider,  // ← AGREGAR
    inputMode: state.inputMode,      // ya existe
}),
```

## Criterios de aceptación

- [x] Al intentar activar el micrófono en HTTP/IP, el usuario ve un mensaje claro indicando que necesita HTTPS
- [x] Al acceder por Cloudflare (HTTPS), el micrófono funciona normalmente
- [x] Si el usuario configuró `sttProvider: 'groq'` en los settings, la recarga de página mantiene esa configuración
- [x] El error no desaparece en 0.1 segundos — es visible el tiempo suficiente para leerlo

## Dependencias

Ninguna.

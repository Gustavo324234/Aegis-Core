# CORE-182 — fix: TTS ausente en modo local — invocar voiceService.speak en WebSocket onDone

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / app  
**Tipo:** fix  
**Estado:** ✅ Done  
**Prioridad:** Alta  
**Asignado a:** Shell Engineer  
**Dependencias:** ninguna

---

## Problema

En modo **local** (WebSocket contra servidor propio), el chat no responde con voz
aunque `isConversationMode` esté activo. El usuario puede hablar, el mensaje se envía
y se procesa, pero la respuesta del asistente nunca se vocaliza.

En modo **cloud** la voz funciona correctamente.

**Causa raíz — `app/app/(main)/chat.tsx`:**

El callback `onDone` del WebSocket local solo llama a `finalizeMessage()` y no invoca
`voiceService.speak()`. La implementación cloud sí lo hace:

```tsx
// Cloud mode ✅ — habla cuando termina (en handleSendMessage, rama cloud)
onDone: () => {
  finalizeMessage();
  const lastMsg = useChatStore.getState().messages.at(-1);
  if (isConversationMode && lastMsg?.role === 'assistant') {
    voiceService.speak(lastMsg.content, language === 'es' ? 'es-ES' : 'en-US', () => {
      if (useChatStore.getState().isConversationMode) toggleVoice();
    });
  }
},

// Local mode (WebSocket) ❌ — no habla (en bffClient.connectChat, onDone)
onDone: () => finalizeMessage(), // ← falta llamar a voiceService.speak
```

---

## Cambios Requeridos

### `app/app/(main)/chat.tsx` — agregar TTS en el `onDone` del WebSocket local

En el `useEffect` que llama a `bffClient.connectChat`, el callback `onDone` debe
invocar `voiceService.speak()` de forma idéntica a como lo hace el modo cloud:

```tsx
// ANTES
const ws = bffClient.connectChat(serverUrl, tenantId, sessionKey, {
  onToken: (token) => {
    hasReceivedMessage.current = true;
    appendToken(token);
  },
  onDone: () => finalizeMessage(),
  onError: (msg) => { ... },
});

// DESPUÉS
const ws = bffClient.connectChat(serverUrl, tenantId, sessionKey, {
  onToken: (token) => {
    hasReceivedMessage.current = true;
    appendToken(token);
  },
  onDone: () => {
    finalizeMessage();
    const lastMsg = useChatStore.getState().messages.at(-1);
    if (isConversationMode && lastMsg?.role === 'assistant') {
      voiceService.speak(lastMsg.content, language === 'es' ? 'es-ES' : 'en-US', () => {
        if (useChatStore.getState().isConversationMode) toggleVoice();
      });
    }
  },
  onError: (msg) => { ... },
});
```

**Nota:** `useChatStore.getState().messages.at(-1)` es necesario porque en el closure
del `useEffect` el array `messages` puede estar desactualizado (closure stale).
Usar el state selector directo del store es el patrón correcto ya establecido en
el código cloud existente.

---

## Criterios de Aceptación

- [ ] En modo local con `isConversationMode` activo, el asistente vocaliza su respuesta al finalizar el stream
- [ ] En modo local con `isConversationMode` inactivo, no hay vocalización (comportamiento no cambia)
- [ ] Después de que el TTS termina, si `isConversationMode` sigue activo, se reactiva el micrófono (`toggleVoice()`)
- [ ] El comportamiento en modo cloud no cambia
- [ ] `npm run build` / TypeScript compila sin errores

---

## Commit message

```
fix(app): CORE-182 TTS ausente en modo local — invocar voiceService.speak en WebSocket onDone
```

---

*Arquitecto IA — 2026-04-25*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: TTS en onDone implementado: useAegisStore.ts:528+ reproduce TTS cuando voiceEnabled (incluye fix del feedback loop).

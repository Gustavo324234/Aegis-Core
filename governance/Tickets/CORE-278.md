# CORE-278 — Shell: TTS en modo texto + simplificar configuración de voz

**Tipo:** feat + fix  
**Prioridad:** Alta  
**Épica:** EPIC 50 — Agent Inbox  
**Asignado a:** Shell Engineer  
**Componentes:**
- `shell/ui/src/store/useAegisStore.ts`
- `shell/ui/src/components/` (selector de modo de voz)
**Estado:** ✅ Completo

---

## Contexto

Dos problemas relacionados que se resuelven juntos:

**1. TTS no habla cuando el usuario escribe texto**
El `ttsPlayer` existe y funciona. El canal `siren_event → tts_audio_chunk` funciona en modo
`conversation`. Pero cuando el usuario escribe texto en el chat, la respuesta llega por
`kernel_event.output` y nadie llama al TTS — el texto aparece en pantalla y silencio.

**2. Demasiadas opciones de configuración de voz**
El usuario tiene que elegir entre `sttProvider` (browser/groq/local), configurar API keys,
y entender `inputMode` (text/audio/conversation). Son decisiones técnicas invisibles para
un usuario normal.

---

## Cambios requeridos

### 1. TTS en el arm `STATE_COMPLETED` del store

En `useAegisStore.ts`, en el arm `kernel_event → status_update → STATE_COMPLETED`:

```typescript
if (su.state === 'STATE_COMPLETED') {
    set({ status: 'idle', activePid: null });

    // CORE-278: Leer respuesta en voz alta si el modo de voz está activo
    if (get().voiceEnabled) {
        const messages = get().messages;
        const lastAssistant = [...messages].reverse().find(m => m.role === 'assistant');
        if (lastAssistant?.content) {
            // Usar Web Speech API directamente — no requiere HTTPS adicional
            // ya que si voiceEnabled está activo, el usuario ya tiene HTTPS
            const utterance = new SpeechSynthesisUtterance(lastAssistant.content);
            utterance.lang = detectLanguage(lastAssistant.content);
            utterance.rate = 1.0;
            window.speechSynthesis.cancel(); // cancelar cualquier TTS previo
            window.speechSynthesis.speak(utterance);
        }
    }

    // ... resto del arm (reactivar mic en modo conversation) ...
}
```

Helper `detectLanguage` en el store (mismo criterio que `voiceService.ts`):
```typescript
function detectLanguage(text: string): string {
    const spanishChars = /[áéíóúñÁÉÍÓÚÑ]/;
    const commonSpanish = /\b(el|la|los|las|un|una|en|es|y|o|pero|por|para|si|no)\b/i;
    return (spanishChars.test(text) || commonSpanish.test(text)) ? 'es-ES' : 'en-US';
}
```

### 2. Simplificar configuración — reemplazar por `voiceEnabled: boolean`

En el store, reemplazar `sttProvider` + `sttApiKey` + `inputMode` por una sola variable:

```typescript
voiceEnabled: boolean  // false por defecto
```

Persistir en localStorage (agregar a `partialize`).

**Comportamiento según `voiceEnabled`:**

| voiceEnabled | STT | TTS |
|---|---|---|
| false | texto manual | sin voz |
| true | Web Speech API (browser) | `speechSynthesis` |

La elección de proveedor (groq/local) queda como configuración avanzada — visible
solo en Settings, no en el flujo principal.

`inputMode` se mantiene internamente para el modo conversación pero no se expone
al usuario como opción en la UI principal.

### 3. Control de voz en la UI

Reemplazar el selector de `sttProvider` visible en el chat por un único toggle:

```
🔇 / 🔊  (icono de voz — toggle on/off)
```

Al activar por primera vez:
- Si está en HTTP no-localhost: mostrar mensaje "La voz requiere HTTPS. Configurá
  tu dominio con HTTPS para habilitar esta función."
- Si está en HTTPS o localhost: activar directamente, sin más preguntas.

Al activar, `voiceEnabled = true` y el botón del micrófono aparece en el input.

---

## Criterios de aceptación

- [x] Cuando el usuario escribe texto y `voiceEnabled: true`, la respuesta se lee en voz alta al llegar `STATE_COMPLETED`
- [x] Cuando `voiceEnabled: false`, no hay TTS — comportamiento actual
- [x] El toggle de voz está visible en la UI del chat
- [x] En HTTP no-localhost, el toggle muestra un mensaje explicativo en lugar de activarse
- [x] `voiceEnabled` persiste entre sesiones
- [x] La configuración avanzada de STT (groq/local) sigue disponible en Settings para usuarios técnicos

## Dependencias

- CORE-280 (HTTPS) — el toggle funciona antes de HTTPS pero avisa al usuario

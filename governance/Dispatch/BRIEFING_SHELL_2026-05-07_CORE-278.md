# BRIEFING — Shell: TTS en modo texto + simplificar voz

**Fecha:** 2026-05-07  
**Para:** Shell Engineer (Antigravity)  
**Tickets:** CORE-278

---

## Prerequisito

Leer el ticket antes de implementar:
- `governance/Tickets/CORE-278.md`

---

## Branch

```
fix/core-278-tts-voice-simplify
```

---

## Objetivo

### CORE-278 — TTS en modo texto + simplificar configuración de voz

**Archivo:** `shell/ui/src/store/useAegisStore.ts`

**Cambio 1 — Nueva variable `voiceEnabled: boolean`**

Agregar al estado inicial: `voiceEnabled: false`

Agregar al `partialize` para persistir en localStorage.

Agregar setter: `setVoiceEnabled: (val: boolean) => void`

**Cambio 2 — TTS en `STATE_COMPLETED`**

En el arm `kernel_event → status_update → STATE_COMPLETED`, después de
`set({ status: 'idle', activePid: null })`, agregar:

```typescript
// CORE-278: leer respuesta en voz alta si voiceEnabled está activo
if (get().voiceEnabled && typeof window !== 'undefined' && window.speechSynthesis) {
    const messages = get().messages;
    const lastAssistant = [...messages].reverse().find(m => m.role === 'assistant');
    if (lastAssistant?.content) {
        const text = lastAssistant.content;
        const lang = /[áéíóúñÁÉÍÓÚÑ]|\b(el|la|los|las|un|una|en|es|y|o|pero|por|para)\b/i.test(text)
            ? 'es-ES' : 'en-US';
        const utterance = new SpeechSynthesisUtterance(text);
        utterance.lang = lang;
        utterance.rate = 1.0;
        window.speechSynthesis.cancel();
        window.speechSynthesis.speak(utterance);
    }
}
```

Este bloque va ANTES del código existente de reactivación de mic en modo
`conversation`, para que ambos coexistan sin conflicto.

**Cambio 3 — Toggle de voz en la UI**

En el componente del chat input (buscar donde está el botón de micrófono),
agregar un toggle de voz simple:

```typescript
// Botón toggle de voz
const { voiceEnabled, setVoiceEnabled } = useAegisStore();

const handleVoiceToggle = () => {
    // Verificar contexto seguro
    const isInsecure = window.location.protocol === 'http:' &&
        !['localhost', '127.0.0.1'].includes(window.location.hostname);

    if (!voiceEnabled && isInsecure) {
        // Mostrar toast o mensaje inline
        alert('La voz requiere HTTPS. Configurá tu dominio con HTTPS para habilitar esta función.');
        return;
    }
    setVoiceEnabled(!voiceEnabled);
};
```

El toggle es un icono de volumen (🔊 / 🔇) o similar, junto al botón de micrófono.
Usar los iconos y clases existentes del proyecto para mantener consistencia visual.

**Lo que NO cambiar:**
- La lógica de `sttProvider` (groq/local) — mantenerla para usuarios técnicos en Settings
- La lógica de `inputMode` — mantenerla internamente para modo `conversation`
- El flujo de `startSirenStream` — no tocarlo

---

## Verificación

```bash
npm run build    # en shell/ui/
npm run lint
```

Sin errores de TypeScript ni lint.

---

## Commit y PR

**Commit message:**
```
fix(shell): CORE-278 TTS en modo texto, toggle de voz simplificado
```

**PR title:**
```
fix(shell): CORE-278 — TTS responde en voz + toggle voiceEnabled
```

**PR description:**
```
## CORE-278 — TTS en modo texto + simplificar voz

### Problema
Cuando el usuario escribe texto en el chat, la respuesta llega visualmente
pero nunca se lee en voz alta. El ttsPlayer existía pero nadie lo llamaba
para el flujo de texto.

### Cambios
- Nueva variable `voiceEnabled: boolean` (default false, persiste en localStorage)
- En STATE_COMPLETED: si voiceEnabled, lee la última respuesta con speechSynthesis
- Toggle de voz en el chat input (🔊/🔇)
- En HTTP no-localhost: toggle muestra mensaje explicativo en lugar de activarse

### Lo que NO cambia
- Configuración avanzada de STT (groq/local) — sigue en Settings
- inputMode y lógica de conversación — sin cambios

## Verificación
npm run build ✅  npm run lint ✅
```

**Target branch:** `main`

---

*Briefing creado por Arquitecto IA — 2026-05-07*

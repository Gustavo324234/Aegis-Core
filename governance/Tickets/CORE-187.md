# CORE-187 — feat: Wake Word Detection — activación por voz "Aegis" sin botón

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-core + kernel/crates/ank-http + shell/ui  
**Tipo:** feat  
**Prioridad:** Media  
**Asignado a:** Kernel Engineer (backend) + Shell Engineer (frontend)  
**Dependencias:** CORE-185, CORE-186 (TTS pipeline debe estar funcionando)
**Estado:** ✅ Completo

---

## Contexto

El modo `conversation` actual requiere presionar un botón para activar el micrófono.
Este ticket implementa detección de wake word — al decir "Aegis" (o variantes: "Hey Aegis",
"Oye Aegis") el sistema activa automáticamente el pipeline de voz sin interacción manual.

Similar a Siri ("Hey Siri") y Google Assistant ("Ok Google").

---

## Arquitectura

La detección de wake word **no puede correr en el servidor** porque necesita acceso
continuo al micrófono del usuario. Corre en el **frontend** usando la Web Speech API
o un modelo ligero en WebAssembly.

Hay dos enfoques:

### Opción A — Web Speech API continua (simple, sin modelo extra)

Usar `SpeechRecognition` con `continuous: true` e `interimResults: true` en un loop
permanente, filtrando el transcript por la palabra "aegis". Cuando detecta la palabra,
activa el pipeline de Siren.

**Pros:** Zero dependencias, implementable en horas, funciona en Chrome/Edge.
**Contras:** Manda audio a Google en Chrome. No funciona offline. Firefox no soportado.

### Opción B — Modelo local en WebAssembly (porcupine-web o onnx)

Correr un modelo de keyword spotting en WASM directamente en el browser.

**Pros:** Offline, privado, multi-browser.
**Contras:** Requiere integrar una librería externa (Porcupine de Picovoice tiene
free tier para "Aegis", o usar un modelo ONNX custom).

**Decisión: Implementar Opción A primero.** Es funcional para desarrollo y demo.
Opción B puede ser CORE-191 como upgrade de privacidad.

---

## Cambios Requeridos

### Backend — ninguno para este ticket

La detección de wake word es 100% frontend. El backend (CORE-185/186) ya maneja
el audio una vez activado.

### Frontend — `shell/ui/src/store/useAegisStore.ts`

#### Agregar al estado:

```typescript
wakeWordEnabled: boolean;
isListeningForWakeWord: boolean;
```

#### Agregar a la interfaz `AegisState`:

```typescript
wakeWordEnabled: boolean;
isListeningForWakeWord: boolean;
setWakeWordEnabled: (enabled: boolean) => void;
startWakeWordListener: () => void;
stopWakeWordListener: () => void;
```

#### Valores iniciales:

```typescript
wakeWordEnabled: false,
isListeningForWakeWord: false,
```

#### Implementación de `startWakeWordListener`:

```typescript
startWakeWordListener: () => {
    if (get().isListeningForWakeWord) return;

    const SpeechRecognitionCtor =
        window.SpeechRecognition || window.webkitSpeechRecognition;

    if (!SpeechRecognitionCtor) {
        console.warn('Wake Word: Web Speech API no disponible');
        return;
    }

    const recognition = new SpeechRecognitionCtor();
    recognition.continuous = true;
    recognition.interimResults = true;
    recognition.lang = 'es-ES'; // Aegis está configurado en español por defecto

    // Palabras de activación aceptadas
    const WAKE_WORDS = ['aegis', 'hey aegis', 'oye aegis', 'ok aegis'];

    recognition.onresult = (event: SpeechRecognitionEvent) => {
        // Buscar en todos los resultados (incluyendo interim)
        for (let i = event.resultIndex; i < event.results.length; i++) {
            const transcript = event.results[i][0].transcript.toLowerCase().trim();
            const detected = WAKE_WORDS.some(w => transcript.includes(w));

            if (detected && !get().isRecording && get().inputMode === 'conversation') {
                // Wake word detectado — activar pipeline de voz
                get().stopWakeWordListener();
                get().startSirenStream().catch(console.error);
                break;
            }
        }
    };

    recognition.onend = () => {
        // Reiniciar automáticamente si sigue habilitado y no estamos en conversación
        if (get().wakeWordEnabled && !get().isRecording) {
            // Pequeño delay para evitar loop instantáneo
            setTimeout(() => {
                if (get().wakeWordEnabled && !get().isRecording) {
                    get().startWakeWordListener();
                }
            }, 500);
        } else {
            set({ isListeningForWakeWord: false });
        }
    };

    recognition.onerror = (event) => {
        // 'no-speech' es normal, no loguear
        if ((event as SpeechRecognitionErrorEvent).error !== 'no-speech') {
            console.error('Wake Word error:', (event as SpeechRecognitionErrorEvent).error);
        }
        set({ isListeningForWakeWord: false });
    };

    set({ isListeningForWakeWord: true });
    (window as unknown as Record<string, unknown>)._aegis_wake_recognition = recognition;
    recognition.start();
},

stopWakeWordListener: () => {
    const rec = (window as unknown as Record<string, unknown>)
        ._aegis_wake_recognition as SpeechRecognition | undefined;
    if (rec) {
        try { rec.stop(); } catch { /* already stopped */ }
        delete (window as unknown as Record<string, unknown>)._aegis_wake_recognition;
    }
    set({ isListeningForWakeWord: false });
},

setWakeWordEnabled: (enabled: boolean) => {
    set({ wakeWordEnabled: enabled });
    if (enabled) {
        get().startWakeWordListener();
    } else {
        get().stopWakeWordListener();
    }
},
```

#### Integración con el ciclo de conversación:

En `STATE_COMPLETED`, después de que termina el TTS en modo conversación, si
`wakeWordEnabled` está activo, NO reactivar el mic automáticamente — esperar el
wake word en cambio. Modificar el bloque de `scheduleReactivation` de CORE-184:

```typescript
// En STATE_COMPLETED, dentro de if (get().inputMode === 'conversation'):
const scheduleReactivation = () => {
    const ctx = ttsPlayer.getAudioContext();
    const delay = ctx
        ? Math.max(300, (ttsPlayer.getNextStartTime() - ctx.currentTime) * 1000 + 200)
        : 500;

    setTimeout(() => {
        if (get().inputMode === 'conversation' && !get().isRecording) {
            if (get().wakeWordEnabled) {
                // Modo wake word: esperar activación por voz
                get().startWakeWordListener();
            } else {
                // Modo automático: reactivar mic directo
                get().restartMicForConversation().catch(console.error);
            }
        }
    }, delay);
};
```

#### Persistir `wakeWordEnabled` en `partialize`:

```typescript
partialize: (state) => ({
    // ...campos existentes...
    wakeWordEnabled: state.wakeWordEnabled,
    inputMode: state.inputMode,
}),
```

### Frontend — `shell/ui/src/components/InputModeSelector.tsx`

Agregar toggle de wake word visible solo cuando `inputMode === 'conversation'`:

```tsx
import { Mic2 } from 'lucide-react'; // o MicVocal

// Dentro del componente InputModeSelector, después de los botones de modo:
{inputMode === 'conversation' && (
    <button
        onClick={() => setWakeWordEnabled(!wakeWordEnabled)}
        title={wakeWordEnabled ? 'Wake word ON — di "Aegis" para activar' : 'Wake word OFF — mic manual'}
        className={cn(
            "p-1.5 rounded-md transition-all ml-1 border border-dashed",
            wakeWordEnabled
                ? "text-yellow-400 border-yellow-400/50 bg-yellow-400/10"
                : "text-white/20 border-white/10 hover:text-white/40"
        )}
    >
        <Mic2 className="w-3.5 h-3.5" />
    </button>
)}
```

Agregar al destructuring del store en el componente:
```typescript
const { ..., wakeWordEnabled, setWakeWordEnabled } = useAegisStore();
```

---

## Palabras de activación configurables (futuro)

Por ahora son hardcodeadas en el store. En el futuro pueden moverse a
`/api/siren/config` y configurarse por tenant desde el panel de admin.

---

## Criterios de Aceptación

- [x] Con `wakeWordEnabled = true` y `inputMode = 'conversation'`, el sistema escucha continuamente
- [x] Al decir "Aegis" (o variantes), `startSirenStream()` se activa automáticamente
- [x] El toggle de wake word es visible en `InputModeSelector` solo en modo conversación
- [x] El estado `wakeWordEnabled` persiste entre recargas
- [x] Al cambiar a modo `text` o `audio`, el wake word listener se detiene
- [x] Logout detiene el wake word listener
- [x] Si Web Speech API no está disponible (Firefox), el toggle no aparece o muestra tooltip de aviso
- [x] `npm run build` sin errores TypeScript

---

## Limitaciones conocidas (documentar en UI)

- Funciona solo en Chrome y Edge (Web Speech API)
- El audio de wake word detection se envía a los servidores de Google en Chrome
- Tooltip en el botón: "Requiere Chrome o Edge. El audio de activación se procesa por Google."

---

## Commit messages

```
feat(shell): CORE-187 wake word detection — activación por voz "Aegis" sin botón
```

---

*Arquitecto IA — 2026-04-25*
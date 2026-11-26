# CORE-183 — feat: Input Mode Selector — Texto / Audio / Conversación en shell web

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / shell/ui  
**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Alta  
**Asignado a:** Shell Engineer  
**Dependencias:** ninguna (TTSPlayer y Siren bridge ya implementados)

---

## Contexto

La shell web tiene el Siren bridge completo (`startSirenStream`, `TTSPlayer`, `ttsPlayer.playChunk`) pero sin una arquitectura de modos clara. El botón de micrófono es un toggle binario que no distingue entre "quiero mandar un audio" y "quiero una conversación continua con voz". El TTS de respuesta tampoco se activa cuando el usuario escribe texto.

Este ticket formaliza 3 modos de interacción seleccionables en cualquier momento:

- **Texto**: comportamiento actual. El usuario escribe, lee la respuesta. Sin TTS, sin mic.
- **Audio**: el usuario presiona el mic, habla, Siren transcribe, el kernel responde en texto. Sin TTS de respuesta. El mic se desactiva al terminar la transcripción.
- **Conversación**: el usuario habla, Siren transcribe, el kernel responde, y la respuesta se vocaliza via `ttsPlayer`. Loop automático — al terminar el TTS se reactiva el mic.

El modo es global y persiste durante la sesión. No hay sesiones separadas — es un toggle siempre visible.

---

## Cambios Requeridos

### 1. Store — `shell/ui/src/store/useAegisStore.ts`

**Agregar al estado:**

```typescript
inputMode: 'text' | 'audio' | 'conversation';
```

**Valor inicial:** `'text'`

**Agregar a la interfaz `AegisState`:**

```typescript
inputMode: 'text' | 'audio' | 'conversation';
setInputMode: (mode: 'text' | 'audio' | 'conversation') => void;
```

**Implementación:**

```typescript
inputMode: 'text',
setInputMode: (mode) => set({ inputMode: mode }),
```

**Agregar `inputMode` al `partialize`** para que persista entre recargas:

```typescript
partialize: (state) => ({
  // ...campos existentes...
  inputMode: state.inputMode,
}),
```

**En `socket.onmessage`, case `kernel_event`**, cuando llega `STATE_COMPLETED`:

```typescript
else if (payload.status_update) {
  const su = payload.status_update as { state: string };
  if (su.state === 'STATE_COMPLETED') {
    set({ status: 'idle', activePid: null });

    // CORE-183: TTS de respuesta en modo conversación
    if (get().inputMode === 'conversation') {
      const msgs = get().messages;
      const lastMsg = msgs[msgs.length - 1];
      if (lastMsg?.role === 'assistant' && lastMsg.type === 'text') {
        ttsPlayer.initialize().then(() => {
          // El audio llega por el sirenWs en chunks — no hay que llamar playChunk aquí.
          // El TTSPlayer ya reproduce los chunks a medida que llegan via sirenWs.onmessage.
          // Lo que sí hay que hacer: reactivar el mic al terminar el último chunk.
          // Ver nota de implementación abajo.
        });
      }
    }
  }
}
```

**Nota de implementación — reactivar mic en modo conversación:**

El `TTSPlayer` no expone un callback `onFinished`. La forma correcta es trackear `nextStartTime` y schedular la reactivación del mic con un `setTimeout` al terminar el último chunk programado:

En `TTSPlayer.ts`, agregar:

```typescript
public getNextStartTime(): number {
  return this.nextStartTime;
}
```

En el store, después de recibir el último chunk TTS (cuando llega `STATE_COMPLETED` y `inputMode === 'conversation'`):

```typescript
// Esperar a que termine el audio programado y reactivar mic
const waitForTTS = () => {
  const ctx = ttsPlayer.getAudioContext(); // agregar getter
  if (!ctx) return;
  const remaining = (ttsPlayer.getNextStartTime() - ctx.currentTime) * 1000;
  const delay = Math.max(0, remaining) + 200; // 200ms de buffer
  setTimeout(() => {
    if (get().inputMode === 'conversation') {
      get().startSirenStream().catch(console.error);
    }
  }, delay);
};
```

Agregar a `TTSPlayer.ts`:

```typescript
public getAudioContext(): AudioContext | null {
  return this.audioContext;
}
```

En `stopSirenStream`, al terminar en modo `audio`, NO reactivar mic — quedarse en `idle`.

---

### 2. Componente InputModeSelector — nuevo archivo

**Crear `shell/ui/src/components/InputModeSelector.tsx`**

Selector de 3 modos compacto, integrado en el input bar.

```tsx
import { Keyboard, Mic, MessageCircle } from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';

type InputMode = 'text' | 'audio' | 'conversation';

const MODES: { mode: InputMode; icon: React.FC<any>; label: string; activeColor: string }[] = [
  { mode: 'text',         icon: Keyboard,       label: 'Text',  activeColor: 'text-white bg-white/10' },
  { mode: 'audio',        icon: Mic,             label: 'Audio', activeColor: 'text-aegis-cyan bg-aegis-cyan/10' },
  { mode: 'conversation', icon: MessageCircle,   label: 'Conv',  activeColor: 'text-aegis-purple bg-aegis-purple/10' },
];

export const InputModeSelector: React.FC = () => {
  const { inputMode, setInputMode, isRecording, stopSirenStream } = useAegisStore();

  const handleSelect = (mode: InputMode) => {
    if (isRecording) stopSirenStream();
    setInputMode(mode);
  };

  return (
    <div className="flex items-center gap-0.5 bg-white/5 rounded-lg p-0.5">
      {MODES.map(({ mode, icon: Icon, label, activeColor }) => (
        <button
          key={mode}
          onClick={() => handleSelect(mode)}
          title={label}
          className={`p-1.5 rounded-md transition-all text-[10px] font-mono flex items-center gap-1 ${
            inputMode === mode
              ? activeColor + ' font-bold'
              : 'text-white/30 hover:text-white/60'
          }`}
        >
          <Icon className="w-3.5 h-3.5" />
          <span className="hidden sm:inline">{label}</span>
        </button>
      ))}
    </div>
  );
};
```

---

### 3. ChatTerminal — integrar selector y lógica de mic

**En `shell/ui/src/components/ChatTerminal.tsx`:**

**Importar el selector:**

```tsx
import { InputModeSelector } from './InputModeSelector';
```

**Agregar `inputMode` y `setInputMode` al destructuring del store:**

```tsx
const { ..., inputMode, setInputMode } = useAegisStore();
```

**Reemplazar el botón de micrófono actual** por el `InputModeSelector` + lógica condicional:

```tsx
{/* ANTES: botón de mic standalone */}
<button onClick={handleToggleMic} ...>

{/* DESPUÉS: selector de modo + botón de mic solo cuando aplica */}
<InputModeSelector />

{(inputMode === 'audio' || inputMode === 'conversation') && (
  <button
    onClick={handleToggleMic}
    disabled={!sttAvailable}
    className={cn(
      "p-2 rounded-lg transition-all",
      status === 'listening'     && "bg-green-500 text-white animate-pulse shadow-[0_0_15px_rgba(34,197,94,0.5)]",
      status === 'transcribing'  && "bg-yellow-500 text-black",
      isRecording                && "bg-red-500/20 text-red-400",
      !isRecording && status !== 'listening' && status !== 'transcribing' && "bg-white/5 text-white/40 hover:text-white hover:bg-white/10",
      !sttAvailable              && "opacity-30 cursor-not-allowed"
    )}
    title={isRecording ? "Stop" : "Voice"}
  >
    {status === 'transcribing'
      ? <Loader2 className="w-5 h-5 animate-spin" />
      : isRecording
        ? <MicOff className="w-5 h-5" />
        : <Mic className="w-5 h-5" />
    }
  </button>
)}
```

**`handleToggleMic`** — actualizar para respetar el modo:

```typescript
const handleToggleMic = async () => {
  if (isRecording) {
    stopSirenStream();
    return;
  }
  try {
    setVoiceError(null);
    await startSirenStream();
    // En modo 'audio': el stream se detiene solo al terminar la transcripción (STT_DONE)
    // En modo 'conversation': el store re-arranca el mic automáticamente después del TTS
  } catch (err: unknown) {
    const domError = err as { name?: string };
    setVoiceError(domError.name === 'NotAllowedError'
      ? 'Microphone access denied'
      : 'Hardware error'
    );
    setTimeout(() => setVoiceError(null), 5000);
  }
};
```

**En modo `text`**, el textarea ocupa todo el ancho y el botón de mic no aparece.

**En modo `audio` o `conversation`**, el textarea se achica levemente para dar lugar al botón de mic.

---

### 4. Lógica STT_DONE por modo — en `useAegisStore.ts`

En el handler `sirenWs.onmessage`, case `STT_DONE`, el comportamiento post-transcripción varía:

```typescript
else if (sirenEvent.event_type === 'STT_DONE') {
  const payload = JSON.parse(sirenEvent.message as string) as { transcript: string; pid: string };
  set((state) => ({
    messages: [...state.messages, {
      id: `voice-${Date.now()}`,
      role: 'user',
      content: payload.transcript,
      type: 'text',
      timestamp: Date.now()
    }],
    activePid: payload.pid,
    status: 'thinking'
  }));

  const chatSocket = get().socket;
  if (chatSocket?.readyState === WebSocket.OPEN) {
    chatSocket.send(JSON.stringify({ action: 'watch', pid: payload.pid }));
  }

  // CORE-183: en modo audio, detener el mic al terminar la transcripción
  if (get().inputMode === 'audio') {
    get().stopSirenStream();
  }
  // En modo conversation: el mic se reactiva después del TTS (ver STATE_COMPLETED)
}
```

---

## Criterios de Aceptación

- [ ] El store tiene `inputMode` con valores `'text' | 'audio' | 'conversation'` y `setInputMode`
- [ ] `inputMode` persiste entre recargas de página
- [ ] El `InputModeSelector` es visible en el input bar en todo momento
- [ ] Al cambiar de modo mientras el mic está activo, `stopSirenStream()` se llama antes de cambiar
- [ ] **Modo texto**: no aparece botón de mic, el textarea ocupa todo el ancho, sin TTS
- [ ] **Modo audio**: aparece botón de mic, al terminar `STT_DONE` el mic se detiene, sin TTS de respuesta
- [ ] **Modo conversación**: aparece botón de mic, al llegar `STATE_COMPLETED` el TTS se reproduce, al terminar el TTS el mic se reactiva automáticamente
- [ ] `TTSPlayer` expone `getNextStartTime()` y `getAudioContext()` para calcular el delay de reactivación
- [ ] En modo conversación, si el usuario cambia a modo texto mientras el TTS está reproduciendo, el mic NO se reactiva
- [ ] `npm run build` sin errores TypeScript

---

## Commit message

```
feat(shell): CORE-183 input mode selector — text / audio / conversation con TTS loop
```

---

*Arquitecto IA — 2026-04-25*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: Input Mode Selector implementado: inputMode 'text'|'audio'|'conversation'|'wakeword' + setInputMode en el store (excede el spec: incluye wakeword).

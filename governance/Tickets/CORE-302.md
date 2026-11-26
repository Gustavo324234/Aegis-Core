# CORE-302 — Shell: mutear micrófono durante reproducción TTS (feedback loop)

## Metadata
- **Epic:** 52 — Voice Quality
- **Tipo:** fix
- **Prioridad:** Crítica
- **Asignado a:** Shell Engineer
- **Estado:✅ Done

## Problema

En modo conversación, cuando el TTS (ElevenLabs) reproduce audio por el speaker,
el micrófono captura ese audio. El STT lo transcribe como si fuera el usuario
hablando y lo manda como prompt nuevo al agente. El agente se responde a sí mismo
en un bucle infinito.

## Root cause

El `ScriptProcessorNode` (o `MediaRecorder`) del WebSocket Siren no pausa durante
la reproducción de audio TTS. No hay gate de mute/unmute coordinado entre el
reproductor de audio y el capturador de micrófono.

## Fix

En el componente o hook que maneja el WebSocket Siren (buscar donde se procesa
`tts_audio_chunk` y donde corre el `ScriptProcessorNode`):

### Opción A — Mute del MediaStreamTrack (recomendada)

```ts
// Cuando llega tts_audio_chunk y empieza la reproducción:
microphoneTrack.enabled = false;  // mute inmediato

// Cuando el AudioBuffer termina (onended del AudioBufferSourceNode):
microphoneTrack.enabled = true;   // unmute
```

### Opción B — Suspender el AudioContext de captura

```ts
// Al iniciar TTS:
captureAudioContext.suspend();

// Al terminar TTS:
captureAudioContext.resume();
```

### Implementación

1. Localizar dónde se reproduce el audio TTS (buscar `tts_audio_chunk`,
   `AudioContext`, `decodeAudioData`, o `AudioBufferSourceNode`)

2. Antes de llamar a `source.start()`:
```ts
if (microphoneTrack) microphoneTrack.enabled = false;
```

3. En el callback `source.onended`:
```ts
source.onended = () => {
    if (microphoneTrack) microphoneTrack.enabled = true;
};
```

4. Guardar referencia al `microphoneTrack` accesible desde el reproductor TTS.
   Si el track no está disponible directamente, usar un flag global:

```ts
// Estado compartido
let isTTSPlaying = false;

// En el ScriptProcessorNode o AudioWorklet:
onaudioprocess = (e) => {
    if (isTTSPlaying) return; // descartar audio capturado durante TTS
    // ... resto del procesado
};
```

## Criterios de aceptación

- [ ] Durante la reproducción TTS el micrófono no captura audio
- [ ] Al terminar el TTS el micrófono se reactiva automáticamente
- [ ] El usuario puede hablar inmediatamente después de que termina la respuesta
- [ ] No hay lag perceptible entre fin de TTS y reactivación del mic
- [ ] `npm run build` pasa

## Dependencias

- Ninguna

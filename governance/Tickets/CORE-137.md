# CORE-137 — Feature: Comandos de control de música por chat y voz

**Epic:** 39 — Aegis Music
**Repo:** Aegis-Core — `kernel/` + `shell/`
**Tipo:** feat
**Prioridad:** Media
**Asignado a:** Kernel Engineer (prompt) + Shell Engineer (store)
**Depende de:** CORE-135, CORE-136

---

## Contexto

El usuario debe poder controlar la música en cualquier momento con comandos naturales
de chat o voz. No solo "poneme una canción" — también "pausá", "subí el volumen",
"cambiá a otra cosa", "pará la música".

Estos controles son del lado del **frontend** — el player ya existe en el browser
(CORE-136). El agente no necesita hablar con el kernel para pausar; solo necesita
enviar un evento al store de Zustand.

---

## Arquitectura de control

```
Usuario: "pausá la música" / "subí el volumen" / "cambiá de canción"
    │
    ▼
LLM detecta intención de control de música
    │  (instrucción en SYSTEM_PROMPT_MASTER: usar tags de control)
    ▼
LLM emite tag: [MUSIC_PAUSE] / [MUSIC_RESUME] / [MUSIC_VOLUME:80] / [MUSIC_STOP]
    │
    ▼
WebSocket handler intercepta el tag (similar a MUSIC_PLAY)
    │
    ▼
Evento WS: {"event": "music_control", "data": {"action": "pause"}}
    │
    ▼
Frontend: useMusicStore recibe acción → controla el player YouTube
```

---

## Cambios requeridos

### 1. Kernel — Tags de control en `ws/chat.rs`

Agregar regex y detección en el streaming, junto a `MUSIC_PLAY_RE` de CORE-135:

```rust
static MUSIC_CTRL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[(MUSIC_PAUSE|MUSIC_RESUME|MUSIC_STOP|MUSIC_VOLUME:(\d{1,3}))\]")
        .expect("FATAL: music control regex is invalid")
});
```

En el loop de streaming:
```rust
if let Some(caps) = MUSIC_CTRL_RE.captures(text) {
    let tag = &caps[1];
    let (action, value) = if tag.starts_with("MUSIC_VOLUME:") {
        ("volume", caps.get(2).map(|m| m.as_str()).unwrap_or("70"))
    } else {
        (match tag {
            "MUSIC_PAUSE"  => "pause",
            "MUSIC_RESUME" => "resume",
            "MUSIC_STOP"   => "stop",
            _              => "unknown",
        }, "")
    };

    let _ = socket.send(Message::Text(
        json!({
            "event": "music_control",
            "data": { "action": action, "value": value }
        }).to_string()
    )).await;
    // Filtrar el tag del output visible
    continue;
}
```

### 2. Kernel — Instrucciones de control en `SYSTEM_PROMPT_MASTER`

En `build_prompt()` de `chal/mod.rs`, expandir la sección de música:

```rust
let music_section = if std::env::var("YOUTUBE_API_KEY").is_ok() {
    "\n\nMÚSICA — INSTRUCCIONES:\
     \n- Para reproducir: [SYS_CALL_PLUGIN(\"music_search\", {\"query\": \"<artista canción>\", \"max_results\": 1})] y luego [MUSIC_PLAY:<video_id>]\
     \n- Para pausar: responde brevemente y termina con [MUSIC_PAUSE]\
     \n- Para continuar: responde brevemente y termina con [MUSIC_RESUME]\
     \n- Para detener: responde brevemente y termina con [MUSIC_STOP]\
     \n- Para cambiar volumen: termina con [MUSIC_VOLUME:<0-100>]\
     \n- Para cambiar canción: haz una nueva búsqueda y usa [MUSIC_PLAY:<nuevo_video_id>]\
     \nNunca expliques estos tags al usuario. Solo úsalos.\n"
} else { "" };
```

### 3. Shell — Manejar `music_control` en `useAegisStore.ts`

En la función de procesamiento de eventos del WebSocket:

```typescript
if (event.event === 'music_control') {
  const { setPlaying, setVolume, closePlayer } = useMusicStore.getState();
  const { action, value } = event.data ?? {};
  switch (action) {
    case 'pause':  setPlaying(false); break;
    case 'resume': setPlaying(true);  break;
    case 'stop':   closePlayer();     break;
    case 'volume':
      const vol = parseInt(value ?? '70', 10);
      if (!isNaN(vol)) setVolume(Math.min(100, Math.max(0, vol)));
      break;
  }
  return; // No agregar al chat como mensaje
}
```

### 4. Shell — Soporte de voz para música

El flujo de Siren (voz) ya convierte audio a texto y lo envía al scheduler como
un PCB normal. No hay cambio necesario en el pipeline de voz. El transcript del
usuario llega al LLM con el contexto de música activa, y el LLM emite los tags
correspondientes.

**Única adición:** Pasar el `video_id` actual como contexto al LLM si hay música
reproduciéndose, para que sepa qué está sonando:

```typescript
// En el payload del WebSocket al enviar un prompt:
const musicState = useMusicStore.getState();
const musicContext = musicState.isPlayerVisible && musicState.currentTrack
  ? `[CONTEXT: Currently playing "${musicState.currentTrack.title}"]`
  : '';

// Agregar al prompt antes de enviarlo:
const finalPrompt = musicContext ? `${musicContext}\n${userPrompt}` : userPrompt;
```

---

## Comandos soportados (ejemplos)

| Input del usuario | Tag generado | Resultado |
|---|---|---|
| "poneme algo de jazz" | `[SYS_CALL_PLUGIN("music_search",...)]` + `[MUSIC_PLAY:xxx]` | Reproduce jazz |
| "pausá" / "pause" | `[MUSIC_PAUSE]` | Pausa el player |
| "seguí" / "continuá" | `[MUSIC_RESUME]` | Reanuda |
| "subí el volumen" | `[MUSIC_VOLUME:90]` | Sube a 90 |
| "bajá el volumen" | `[MUSIC_VOLUME:30]` | Baja a 30 |
| "pará la música" | `[MUSIC_STOP]` | Cierra el player |
| "cambiá de canción" | nueva búsqueda + `[MUSIC_PLAY:yyy]` | Cambia el track |

---

## Criterios de aceptación

- [ ] `cargo build --workspace` + `npm run build` sin errores
- [ ] "pausá" mientras suena música → el player se pausa
- [ ] "subí el volumen a 80" → el slider se mueve a 80 y el volumen cambia
- [ ] "pará" → el player desaparece
- [ ] "poneme otra" → nueva búsqueda → nuevo track reemplaza al anterior
- [ ] Los tags no aparecen en el texto del chat
- [ ] Comandos de voz (Siren) funcionan igual que los de chat

---

## Dependencias

- CORE-135 (backend música)
- CORE-136 (store + player UI)

---

## Commit message

```
feat(ank-http,shell): CORE-137 music controls — pause/resume/stop/volume via chat and voice
```

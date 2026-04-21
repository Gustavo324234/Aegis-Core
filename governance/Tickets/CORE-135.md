# CORE-135 — Feature: Music Module — Plugin `music_search` en Kernel

**Epic:** 39 — Aegis Music
**Repo:** Aegis-Core — `kernel/`
**Crates:** `ank-core`, `ank-http`
**Tipo:** feat
**Prioridad:** Media
**Asignado a:** Kernel Engineer

---

## Contexto y arquitectura

El módulo de música funciona así:

```
Usuario: "poneme Bohemian Rhapsody"
    │
    ▼
LLM emite syscall: [SYS_CALL_PLUGIN("music_search", {"query": "Bohemian Rhapsody Queen"})]
    │
    ▼
Kernel → Plugin music_search → YouTube Data API v3 → retorna lista de resultados
    │  { "results": [{ "video_id": "fJ9rUzIMcZQ", "title": "...", "channel": "...", "duration": "5:55" }] }
    ▼
LLM recibe resultado y responde al usuario:
    "Encontré 'Bohemian Rhapsody' de Queen. Reproduciendo ahora."
    + emite evento especial: [MUSIC_PLAY:fJ9rUzIMcZQ]
    │
    ▼
WebSocket handler intercepta [MUSIC_PLAY:...] y lo envía como evento separado al frontend
    │
    ▼
Frontend recibe evento tipo "music_play" → renderiza MusicPlayer con YouTube IFrame API
```

**Sin streaming de audio propio.** El player de YouTube maneja todo: buffering,
DRM, calidad. Aegis solo orquesta la búsqueda y controla el iframe via postMessage.

**YouTube Data API v3:** Gratuita con API key, 10.000 unidades/día en el free tier
(una búsqueda cuesta 100 unidades → 100 búsquedas/día gratis). La key se configura
como variable de entorno o a través del panel de Proveedores del tenant.

---

## Cambios requeridos

### 1. Nueva Syscall `MusicSearch` en `ank-core/src/syscalls/mod.rs`

```rust
// Agregar al enum Syscall:
MusicSearch {
    query: String,
    max_results: u8, // 1-5, default 1
},
```

Agregar regex en los `LazyLock`:

```rust
#[allow(clippy::expect_used)]
static MUSIC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\[SYS_CALL_PLUGIN\("music_search",\s*(\{.*?\})\)\]"#)
        .expect("FATAL: music syscall regex is invalid")
});
```

En `parse_syscall()`, agregar antes del return final:

```rust
if let Some(caps) = MUSIC_RE.captures(text) {
    if let Ok(args) = serde_json::from_str::<serde_json::Value>(&caps[1]) {
        let query = args["query"].as_str().unwrap_or("").to_string();
        let max = args.get("max_results").and_then(|v| v.as_u64()).unwrap_or(1) as u8;
        return Some(Syscall::MusicSearch { query, max_results: max.min(5) });
    }
}
```

### 2. Ejecutor de `MusicSearch` en `SyscallExecutor::execute()`

```rust
Syscall::MusicSearch { query, max_results } => {
    // Leer YOUTUBE_API_KEY del entorno
    // Si no hay key: retornar instrucción para el LLM de que informe al usuario
    let api_key = match std::env::var("YOUTUBE_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => return Ok(
            "[SYSTEM_RESULT: No YouTube API key configured. \
             Tell the user to add YOUTUBE_API_KEY to configure music search.]".to_string()
        ),
    };

    let url = format!(
        "https://www.googleapis.com/youtube/v3/search\
         ?part=snippet&type=video&videoCategoryId=10\
         &q={}&maxResults={}&key={}",
        urlencoding::encode(&query),
        max_results,
        api_key
    );

    // Usar self.http_client (reqwest Arc) para la petición
    let resp = self.http_client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| SyscallError::IOError(format!("YouTube API request failed: {}", e)))?;

    if !resp.status().is_success() {
        return Err(SyscallError::IOError(
            format!("YouTube API error: {}", resp.status())
        ));
    }

    let data: serde_json::Value = resp.json().await
        .map_err(|e| SyscallError::IOError(e.to_string()))?;

    // Extraer resultados relevantes
    let results: Vec<serde_json::Value> = data["items"]
        .as_array()
        .map(|items| items.iter().map(|item| {
            serde_json::json!({
                "video_id": item["id"]["videoId"].as_str().unwrap_or(""),
                "title": item["snippet"]["title"].as_str().unwrap_or(""),
                "channel": item["snippet"]["channelTitle"].as_str().unwrap_or(""),
                "thumbnail": item["snippet"]["thumbnails"]["default"]["url"].as_str().unwrap_or("")
            })
        }).collect())
        .unwrap_or_default();

    Ok(format!("[SYSTEM_RESULT: {}]",
        serde_json::to_string(&serde_json::json!({ "results": results }))
            .unwrap_or_default()
    ))
}
```

**Nota:** Agregar `urlencoding = "2"` a `ank-core/Cargo.toml` si no existe ya.
Si el workspace ya tiene `percent-encoding`, usar ese en su lugar.

El `SyscallExecutor` necesita acceso a un `Arc<reqwest::Client>`. Agregar campo:
```rust
pub struct SyscallExecutor {
    // ... campos existentes ...
    http_client: Arc<reqwest::Client>,
}
```

### 3. Interceptar `[MUSIC_PLAY:video_id]` en `ws/chat.rs`

El LLM, después de recibir el resultado de búsqueda, genera en su respuesta el tag:
`[MUSIC_PLAY:fJ9rUzIMcZQ]`

Interceptar este tag en el streaming de tokens (similar al `StreamInterceptor` de syscalls),
extrayendo el `video_id` antes de enviar el token al WebSocket, y emitiendo en su lugar
un evento especial:

```rust
// En stream_with_receiver(), antes de enviar cada token al socket:
if let ank_proto::v1::task_event::Payload::Output(ref text) = payload {
    // Detectar [MUSIC_PLAY:xxxxx]
    if let Some(caps) = MUSIC_PLAY_RE.captures(text) {
        let video_id = caps[1].to_string();
        // Enviar evento especial al frontend
        let _ = socket.send(Message::Text(
            json!({
                "event": "music_play",
                "data": { "video_id": video_id }
            }).to_string()
        )).await;
        // No enviar el tag raw al chat (filtrar del output)
        continue;
    }
}
```

Agregar regex:
```rust
static MUSIC_PLAY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[MUSIC_PLAY:([A-Za-z0-9_-]{11})\]")
        .expect("FATAL: music play regex is invalid")
});
```

### 4. Endpoint `GET /api/music/config` — estado de la integración

```rust
// En ank-http, nuevo archivo routes/music_api.rs
// GET /api/music/config → { "configured": bool, "provider": "youtube" }
// Solo verifica si YOUTUBE_API_KEY está seteada. No requiere auth (info pública).
```

Registrar en `routes/mod.rs`:
```rust
pub mod music_api;
// En build_router():
.nest("/api/music", music_api::router())
```

### 5. Prompt del sistema — instrucciones de música

En `SYSTEM_PROMPT_MASTER` (modificado en CORE-128), agregar sección condicional
que se inyecta cuando la música está configurada. En `build_prompt()`:

```rust
let music_section = if std::env::var("YOUTUBE_API_KEY").is_ok() {
    "\n\nMÚSICA: Cuando el usuario pida reproducir música, usa exactamente:\
     [SYS_CALL_PLUGIN(\"music_search\", {\"query\": \"<artista y canción>\", \"max_results\": 1})]\
     Cuando recibas el resultado, responde brevemente confirmando qué vas a reproducir\
     y termina con [MUSIC_PLAY:<video_id>] en una línea separada. Sin explicaciones adicionales.\n"
} else {
    ""
};
```

---

## Variables de entorno

| Variable | Descripción | Requerida |
|---|---|---|
| `YOUTUBE_API_KEY` | API key de YouTube Data API v3 | Sí (para música) |

El usuario puede configurarla en `/etc/aegis/aegis.env` o a través de la UI
(CORE-136 agrega el campo en Settings).

---

## Criterios de aceptación

- [ ] `cargo build --workspace` sin errores ni warnings Clippy
- [ ] Sin `YOUTUBE_API_KEY`: el agente le informa al usuario que configure la key
- [ ] Con `YOUTUBE_API_KEY`: "poneme Bohemian Rhapsody" → syscall → resultado → evento `music_play`
- [ ] Evento `music_play` llega al WebSocket como `{"event": "music_play", "data": {"video_id": "..."}}`
- [ ] El tag `[MUSIC_PLAY:xxx]` no aparece en el texto visible del chat
- [ ] `GET /api/music/config` retorna `{"configured": false}` sin key, `{"configured": true, "provider": "youtube"}` con key

---

## Dependencias

- CORE-128 (modificaciones a `build_prompt`)

---

## Commit message

```
feat(ank-core,ank-http): CORE-135 music module — YouTube search syscall + music_play event
```

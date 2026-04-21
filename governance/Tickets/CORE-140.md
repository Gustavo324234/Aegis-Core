# CORE-140 — Feature: Spotify Music — reemplaza YouTube en Epic 39

**Epic:** 40 — Connected Accounts (OAuth)
**Repo:** Aegis-Core — `kernel/` + `shell/`
**Tipo:** feat
**Prioridad:** Media
**Asignado a:** Kernel Engineer + Shell Engineer
**Depende de:** CORE-138 (OAuth tokens), CORE-135 (música base)

---

## Contexto

Si el tenant tiene Spotify conectado (CORE-138), el módulo de música usa
Spotify en lugar de YouTube. Si no tiene Spotify pero tiene Google conectado,
usa YouTube Music. Si no tiene ninguno conectado, le pide que conecte una cuenta.

**Prioridad de providers:**
```
Spotify conectado  → usar Spotify Web API
Google conectado   → usar YouTube Data API (sin key manual — usa token OAuth)
Ninguno            → informar al usuario que conecte una cuenta en Settings
```

---

## Cambios requeridos

### 1. Kernel — Syscall `MusicSearch` actualizada

Modificar `SyscallExecutor::execute()` para el caso `MusicSearch` (CORE-135):

```rust
Syscall::MusicSearch { query, max_results } => {
    // Abrir enclave del tenant para verificar conexiones OAuth
    let db = crate::enclave::TenantDB::open(tenant_id, session_key_hash)?;

    // Prioridad 1: Spotify
    if db.is_oauth_connected("spotify")? {
        return search_spotify(
            &self.http_client, &db, &query, max_results, tenant_id
        ).await;
    }

    // Prioridad 2: YouTube via Google OAuth (sin key manual)
    if db.is_oauth_connected("google")? {
        return search_youtube_oauth(
            &self.http_client, &db, &query, max_results
        ).await;
    }

    // Sin conexión
    Ok("[SYSTEM_RESULT: No music provider connected. \
        Tell the user to connect Spotify or Google in Settings \
        (the gear icon → Cuentas tab).]".to_string())
}
```

#### Función `search_spotify()`

```rust
async fn search_spotify(
    http_client: &Arc<reqwest::Client>,
    db: &TenantDB,
    query: &str,
    max_results: u8,
    tenant_id: &str,
) -> Result<String, SyscallError> {
    let token = crate::oauth::get_or_refresh_token(http_client, db, "spotify")
        .await
        .map_err(|e| SyscallError::IOError(e.to_string()))?;

    let url = format!(
        "https://api.spotify.com/v1/search?q={}&type=track&limit={}",
        urlencoding::encode(query), max_results
    );

    let resp: serde_json::Value = http_client
        .get(&url)
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| SyscallError::IOError(e.to_string()))?
        .json()
        .await
        .map_err(|e| SyscallError::IOError(e.to_string()))?;

    let results: Vec<serde_json::Value> = resp["tracks"]["items"]
        .as_array()
        .map(|items| items.iter().map(|t| serde_json::json!({
            "provider": "spotify",
            "track_id": t["id"].as_str().unwrap_or(""),
            "track_uri": t["uri"].as_str().unwrap_or(""), // spotify:track:xxx
            "title": t["name"].as_str().unwrap_or(""),
            "artist": t["artists"][0]["name"].as_str().unwrap_or(""),
            "album": t["album"]["name"].as_str().unwrap_or(""),
            "duration_ms": t["duration_ms"].as_u64().unwrap_or(0),
            "thumbnail": t["album"]["images"][0]["url"].as_str().unwrap_or(""),
            "preview_url": t["preview_url"].as_str().unwrap_or(""),
        })).collect())
        .unwrap_or_default();

    Ok(format!("[SYSTEM_RESULT: {}]",
        serde_json::to_string(&serde_json::json!({ "results": results, "provider": "spotify" }))
            .unwrap_or_default()
    ))
}
```

#### Función `search_youtube_oauth()`

Idéntica a la búsqueda de CORE-135 pero sin `YOUTUBE_API_KEY` — usa el access token
de Google en el header `Authorization: Bearer <token>`.

### 2. Evento `music_play` extendido — soporte Spotify

En `ws/chat.rs`, el tag del LLM cambia según el provider:

```
[MUSIC_PLAY:spotify:spotify:track:4iV5W9uYEdYUVa79Axb7Rh]  ← Spotify URI
[MUSIC_PLAY:youtube:fJ9rUzIMcZQ]                            ← YouTube video ID
```

El regex en CORE-135 se actualiza:
```rust
static MUSIC_PLAY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[MUSIC_PLAY:(spotify|youtube):([A-Za-z0-9_:%-]{5,50})\]")
        .expect("FATAL: music play regex is invalid")
});
```

El evento enviado al frontend incluye el provider:
```json
{
  "event": "music_play",
  "data": {
    "provider": "spotify",
    "track_uri": "spotify:track:4iV5W9uYEdYUVa79Axb7Rh",
    "title": "Bohemian Rhapsody",
    "artist": "Queen",
    "thumbnail": "https://..."
  }
}
```

### 3. Shell — `MusicPlayer` con soporte Spotify Web Playback SDK

Modificar `MusicPlayer.tsx` (CORE-136) para soportar ambos providers:

```typescript
// Si provider === 'spotify':
// Usar Spotify Web Playback SDK (requiere Premium)
// O usar el preview_url (30s MP3 — funciona sin Premium)

// Estrategia:
// 1. Si tiene preview_url disponible → usar <audio> nativo (funciona siempre)
// 2. Si no → mostrar enlace a Spotify con deep link (spotify:track:xxx)

// Para Spotify Free (sin Premium): solo preview 30s + link a Spotify
// Para Spotify Premium: usar Web Playback SDK (requiere token en el frontend)
```

**Para MVP:** Usar `preview_url` (30s MP3 gratuito) + botón "Abrir en Spotify".
El Spotify Web Playback SDK (reproducción completa) es post-MVP — requiere Premium
y manejo de token en el frontend.

```tsx
// En MusicPlayer, branch por provider:
{currentTrack.provider === 'spotify' && currentTrack.previewUrl && (
  <audio
    ref={audioRef}
    src={currentTrack.previewUrl}
    onPlay={() => setPlaying(true)}
    onPause={() => setPlaying(false)}
    onEnded={() => setPlaying(false)}
  />
)}
{currentTrack.provider === 'spotify' && (
  <a
    href={currentTrack.trackUri}
    target="_blank"
    rel="noopener noreferrer"
    className="text-[9px] font-mono text-green-400/60 hover:text-green-400 flex items-center gap-1"
  >
    <Music2 className="w-3 h-3" /> Abrir en Spotify
  </a>
)}
```

---

## Criterios de aceptación

- [ ] Sin OAuth conectado: el agente informa al usuario que conecte una cuenta
- [ ] Con Spotify: "poneme Bohemian Rhapsody" → busca en Spotify → player muestra thumbnail + título + artista
- [ ] Con Spotify Free: preview de 30s se reproduce + link "Abrir en Spotify"
- [ ] Con Google OAuth: búsqueda funciona sin `YOUTUBE_API_KEY` en el env
- [ ] Prioridad Spotify > YouTube se respeta si ambos están conectados
- [ ] Los tags `[MUSIC_PLAY:spotify:...]` / `[MUSIC_PLAY:youtube:...]` no aparecen en el chat

---

## Dependencias

- CORE-138 (OAuth tokens en enclave)
- CORE-135 (syscall MusicSearch base)
- CORE-136 (MusicPlayer UI)

---

## Commit message

```
feat(ank-core,shell): CORE-140 Spotify music — OAuth-based search and playback replacing YouTube key
```

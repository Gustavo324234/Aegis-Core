# CORE-136 — Feature: MusicPlayer UI — Player flotante con controles YouTube

**Epic:** 39 — Aegis Music
**Repo:** Aegis-Core — `shell/`
**Path:** `shell/ui/src/`
**Tipo:** feat
**Prioridad:** Media
**Asignado a:** Shell Engineer (Antigravity)
**Depende de:** CORE-135 (evento `music_play` del WebSocket)

---

## Contexto

Cuando el WebSocket recibe un evento `{"event": "music_play", "data": {"video_id": "xxx"}}`,
el frontend debe renderizar un player flotante estilo Spotify — discreto, en la esquina
inferior, con controles de play/pause y volumen — sin interrumpir el chat.

El player usa la **YouTube IFrame API** (no descarga audio — reproduce el video de YouTube
embebido con el iframe oculto). El video está oculto; solo se muestra el player control UI.

---

## Componentes a crear

### 1. `shell/ui/src/store/musicStore.ts` — Store Zustand de música

```typescript
import { create } from 'zustand';

export interface MusicTrack {
  videoId: string;
  title: string;
  channel: string;
  thumbnail: string;
}

interface MusicState {
  currentTrack: MusicTrack | null;
  isPlaying: boolean;
  volume: number;           // 0-100
  isPlayerVisible: boolean;
  playerReady: boolean;
  // Acciones
  playTrack: (track: MusicTrack) => void;
  setPlaying: (playing: boolean) => void;
  setVolume: (vol: number) => void;
  setPlayerReady: (ready: boolean) => void;
  closePlayer: () => void;
}

export const useMusicStore = create<MusicState>((set) => ({
  currentTrack: null,
  isPlaying: false,
  volume: 70,
  isPlayerVisible: false,
  playerReady: false,

  playTrack: (track) => set({
    currentTrack: track,
    isPlaying: true,
    isPlayerVisible: true,
    playerReady: false,
  }),

  setPlaying: (isPlaying) => set({ isPlaying }),
  setVolume: (volume) => set({ volume }),
  setPlayerReady: (playerReady) => set({ playerReady }),
  closePlayer: () => set({
    isPlayerVisible: false,
    isPlaying: false,
    currentTrack: null,
  }),
}));
```

### 2. `shell/ui/src/components/MusicPlayer.tsx` — Player flotante

El player tiene dos partes:
- **YouTube IFrame oculto:** reproduce el audio real via `window.YT.Player`
- **UI visible:** controles flotantes en la esquina inferior derecha

```tsx
import React, { useEffect, useRef, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Play, Pause, Volume2, VolumeX, X, Music2, ChevronDown } from 'lucide-react';
import { useMusicStore } from '../store/musicStore';

// Declaración de tipos para YouTube IFrame API
declare global {
  interface Window {
    YT: {
      Player: new (elementId: string, config: object) => YouTubePlayer;
      PlayerState: { PLAYING: number; PAUSED: number; ENDED: number };
    };
    onYouTubeIframeAPIReady: () => void;
  }
}

interface YouTubePlayer {
  playVideo: () => void;
  pauseVideo: () => void;
  setVolume: (vol: number) => void;
  getVolume: () => number;
  getVideoData: () => { title: string; author: string };
  destroy: () => void;
}

const MusicPlayer: React.FC = () => {
  const {
    currentTrack, isPlaying, volume, isPlayerVisible, playerReady,
    setPlaying, setVolume, setPlayerReady, closePlayer,
  } = useMusicStore();

  const playerRef = useRef<YouTubePlayer | null>(null);
  const [isMinimized, setIsMinimized] = useState(false);
  const [showVolume, setShowVolume] = useState(false);
  const [resolvedTitle, setResolvedTitle] = useState('');

  // Cargar YouTube IFrame API una vez
  useEffect(() => {
    if (window.YT) return;
    const tag = document.createElement('script');
    tag.src = 'https://www.youtube.com/iframe_api';
    document.head.appendChild(tag);
  }, []);

  // Inicializar o actualizar el player cuando cambia el track
  useEffect(() => {
    if (!currentTrack) return;

    const initPlayer = () => {
      if (playerRef.current) {
        playerRef.current.destroy();
      }

      playerRef.current = new window.YT.Player('aegis-yt-player', {
        height: '1',
        width: '1',
        videoId: currentTrack.videoId,
        playerVars: {
          autoplay: 1,
          controls: 0,
          disablekb: 1,
          iv_load_policy: 3,
          modestbranding: 1,
          rel: 0,
        },
        events: {
          onReady: (e: { target: YouTubePlayer }) => {
            e.target.setVolume(volume);
            const data = e.target.getVideoData();
            setResolvedTitle(data.title || currentTrack.title);
            setPlayerReady(true);
          },
          onStateChange: (e: { data: number }) => {
            if (e.data === window.YT.PlayerState.PLAYING) setPlaying(true);
            if (e.data === window.YT.PlayerState.PAUSED) setPlaying(false);
            if (e.data === window.YT.PlayerState.ENDED) setPlaying(false);
          },
        },
      });
    };

    if (window.YT?.Player) {
      initPlayer();
    } else {
      window.onYouTubeIframeAPIReady = initPlayer;
    }

    return () => {
      playerRef.current?.destroy();
      playerRef.current = null;
    };
  }, [currentTrack?.videoId]);

  // Sincronizar play/pause
  useEffect(() => {
    if (!playerReady || !playerRef.current) return;
    if (isPlaying) playerRef.current.playVideo();
    else playerRef.current.pauseVideo();
  }, [isPlaying, playerReady]);

  // Sincronizar volumen
  useEffect(() => {
    if (playerRef.current && playerReady) {
      playerRef.current.setVolume(volume);
    }
  }, [volume, playerReady]);

  const togglePlay = () => setPlaying(!isPlaying);

  const handleVolumeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setVolume(Number(e.target.value));
  };

  return (
    <>
      {/* YouTube IFrame oculto — DEBE existir en el DOM */}
      <div id="aegis-yt-player" style={{ position: 'absolute', opacity: 0, pointerEvents: 'none', width: 1, height: 1 }} />

      {/* Player UI flotante */}
      <AnimatePresence>
        {isPlayerVisible && currentTrack && (
          <motion.div
            initial={{ y: 100, opacity: 0 }}
            animate={{ y: 0, opacity: 1 }}
            exit={{ y: 100, opacity: 0 }}
            transition={{ type: 'spring', damping: 20, stiffness: 300 }}
            className="fixed bottom-6 right-6 z-[200]"
          >
            <div className={`
              glass border border-aegis-cyan/20 rounded-2xl shadow-2xl
              bg-black/80 backdrop-blur-xl overflow-hidden
              transition-all duration-300
              ${isMinimized ? 'w-14 h-14' : 'w-80'}
            `}>
              {isMinimized ? (
                /* Estado minimizado: solo el ícono de nota */
                <button
                  onClick={() => setIsMinimized(false)}
                  className="w-full h-full flex items-center justify-center text-aegis-cyan"
                >
                  <Music2 className="w-5 h-5" />
                  {isPlaying && (
                    <span className="absolute top-1 right-1 w-2 h-2 bg-green-500 rounded-full animate-pulse" />
                  )}
                </button>
              ) : (
                /* Estado expandido */
                <div className="p-4">
                  {/* Header */}
                  <div className="flex items-center justify-between mb-3">
                    <div className="flex items-center gap-2">
                      <Music2 className="w-3.5 h-3.5 text-aegis-cyan" />
                      <span className="text-[9px] font-mono text-aegis-cyan/60 uppercase tracking-widest">
                        Aegis Music
                      </span>
                    </div>
                    <div className="flex items-center gap-1">
                      <button
                        onClick={() => setIsMinimized(true)}
                        className="p-1 text-white/30 hover:text-white/70 transition-colors"
                      >
                        <ChevronDown className="w-3.5 h-3.5" />
                      </button>
                      <button
                        onClick={closePlayer}
                        className="p-1 text-white/30 hover:text-red-400 transition-colors"
                      >
                        <X className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>

                  {/* Thumbnail + Info */}
                  <div className="flex items-center gap-3 mb-4">
                    {currentTrack.thumbnail && (
                      <img
                        src={currentTrack.thumbnail}
                        alt="thumbnail"
                        className="w-12 h-12 rounded-lg object-cover"
                      />
                    )}
                    <div className="flex-1 min-w-0">
                      <p className="text-xs font-mono text-white font-bold truncate leading-tight">
                        {resolvedTitle || currentTrack.title || 'Cargando...'}
                      </p>
                      <p className="text-[10px] font-mono text-white/40 truncate mt-0.5">
                        {currentTrack.channel}
                      </p>
                    </div>
                  </div>

                  {/* Controles */}
                  <div className="flex items-center justify-between">
                    {/* Play/Pause */}
                    <button
                      onClick={togglePlay}
                      disabled={!playerReady}
                      className={`
                        w-10 h-10 rounded-full flex items-center justify-center
                        transition-all duration-200
                        ${playerReady
                          ? 'bg-aegis-cyan text-black hover:scale-105 active:scale-95'
                          : 'bg-white/10 text-white/30 cursor-wait'
                        }
                      `}
                    >
                      {!playerReady ? (
                        <span className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                      ) : isPlaying ? (
                        <Pause className="w-4 h-4" />
                      ) : (
                        <Play className="w-4 h-4 ml-0.5" />
                      )}
                    </button>

                    {/* Volumen */}
                    <div className="flex items-center gap-2 flex-1 ml-4">
                      <button
                        onClick={() => setShowVolume(!showVolume)}
                        className="text-white/40 hover:text-white transition-colors"
                      >
                        {volume === 0
                          ? <VolumeX className="w-4 h-4" />
                          : <Volume2 className="w-4 h-4" />
                        }
                      </button>
                      <input
                        type="range"
                        min="0"
                        max="100"
                        value={volume}
                        onChange={handleVolumeChange}
                        className="flex-1 h-1 accent-aegis-cyan cursor-pointer"
                      />
                      <span className="text-[9px] font-mono text-white/30 w-6 text-right">
                        {volume}
                      </span>
                    </div>
                  </div>

                  {/* Barra de progreso visual (decorativa — YT no expone progress sin polling) */}
                  {isPlaying && (
                    <div className="mt-3 h-0.5 bg-white/10 rounded-full overflow-hidden">
                      <motion.div
                        className="h-full bg-aegis-cyan rounded-full"
                        animate={{ width: ['0%', '100%'] }}
                        transition={{ duration: 300, repeat: Infinity, ease: 'linear' }}
                      />
                    </div>
                  )}
                </div>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </>
  );
};

export default MusicPlayer;
```

### 3. Integrar `MusicPlayer` en `ChatTerminal.tsx`

```tsx
// Import:
import MusicPlayer from './MusicPlayer';
import { useMusicStore } from '../store/musicStore';

// En ChatTerminal, agregar handler de evento music_play en useAegisStore
// (ver CORE-137 — el store ya maneja el evento del WS)

// En el JSX, antes del cierre del div principal:
<MusicPlayer />
```

### 4. Integrar evento `music_play` en `useAegisStore.ts`

En la función que procesa eventos del WebSocket (`handleKernelEvent` o equivalente),
agregar el caso:

```typescript
if (event.event === 'music_play' && event.data?.video_id) {
  const { playTrack } = useMusicStore.getState();
  playTrack({
    videoId: event.data.video_id,
    title: event.data.title || '',
    channel: event.data.channel || '',
    thumbnail: event.data.thumbnail || '',
  });
  return; // No agregar al chat como mensaje
}
```

---

## Criterios de aceptación

- [ ] `npm run build` sin errores TypeScript ni ESLint
- [ ] Al recibir evento `music_play` via WebSocket: el player emerge desde abajo a la derecha
- [ ] El player reproduce el audio del video de YouTube
- [ ] Botón Play/Pause funciona y refleja el estado real del player
- [ ] Slider de volumen ajusta el volumen en tiempo real
- [ ] Botón minimizar colapsa el player a un ícono pequeño con dot verde si está reproduciendo
- [ ] Botón cerrar detiene la reproducción y oculta el player
- [ ] El player no interrumpe el chat ni el scroll de mensajes
- [ ] La thumbnail y el título del track son visibles

---

## Dependencias

- CORE-135 (evento `music_play` del WebSocket)

---

## Commit message

```
feat(shell): CORE-136 music player UI — floating YouTube player with play/pause/volume controls
```

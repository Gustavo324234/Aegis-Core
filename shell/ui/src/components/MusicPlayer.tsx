import React, { useEffect, useRef, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Play, Pause, Volume2, VolumeX, X, Music2, ChevronDown } from 'lucide-react';
import { useMusicStore } from '../store/musicStore';

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

    useEffect(() => {
        if (window.YT) return;
        const tag = document.createElement('script');
        tag.src = 'https://www.youtube.com/iframe_api';
        document.head.appendChild(tag);
    }, []);

    useEffect(() => {
        if (!currentTrack) return;

        const videoId = currentTrack.videoId;

        const initPlayer = () => {
            if (playerRef.current) {
                playerRef.current.destroy();
            }

            playerRef.current = new window.YT.Player('aegis-yt-player', {
                height: '1',
                width: '1',
                videoId,
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
    }, [currentTrack, setPlayerReady, setPlaying, volume]);

    useEffect(() => {
        if (!playerReady || !playerRef.current) return;
        if (isPlaying) playerRef.current.playVideo();
        else playerRef.current.pauseVideo();
    }, [isPlaying, playerReady]);

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
            <div id="aegis-yt-player" style={{ position: 'absolute', opacity: 0, pointerEvents: 'none', width: 1, height: 1 }} />

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
                                <button
                                    onClick={() => setIsMinimized(false)}
                                    className="w-full h-full flex items-center justify-center text-aegis-cyan relative"
                                >
                                    <Music2 className="w-5 h-5" />
                                    {isPlaying && (
                                        <span className="absolute top-1 right-1 w-2 h-2 bg-green-500 rounded-full animate-pulse" />
                                    )}
                                </button>
                            ) : (
                                <div className="p-4">
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

                                    <div className="flex items-center justify-between">
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

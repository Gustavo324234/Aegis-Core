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
    volume: number;
    isPlayerVisible: boolean;
    playerReady: boolean;
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

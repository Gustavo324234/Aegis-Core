import React, { useState } from 'react';
import { Keyboard, Mic, MessageCircle, AlertTriangle, Wand2 } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useAegisStore } from '../store/useAegisStore';

type InputMode = 'text' | 'audio' | 'conversation' | 'wakeword';

const MODES: { mode: InputMode; icon: React.FC<React.SVGProps<SVGSVGElement>>; label: string; activeColor: string; title: string }[] = [
    { mode: 'text',         icon: Keyboard,      label: 'Text',   activeColor: 'text-white bg-white/10',           title: 'Texto' },
    { mode: 'audio',        icon: Mic,            label: 'Audio',  activeColor: 'text-aegis-cyan bg-aegis-cyan/10', title: 'Audio (push-to-talk)' },
    { mode: 'conversation', icon: MessageCircle,  label: 'Conv',   activeColor: 'text-aegis-purple bg-aegis-purple/10', title: 'Conversación continua' },
    { mode: 'wakeword',     icon: Wand2,          label: 'Wake',   activeColor: 'text-amber-400 bg-amber-400/10',   title: 'Activación por nombre' },
];

export const InputModeSelector: React.FC = () => {
    const {
        inputMode,
        setInputMode,
        isRecording,
        stopSirenStream,
        sttProvider,
        sttReady,
        agentName,
        isWakewordListening,
        lastWakeSpeaker,
    } = useAegisStore(
        useShallow((s) => ({
            inputMode: s.inputMode,
            setInputMode: s.setInputMode,
            isRecording: s.isRecording,
            stopSirenStream: s.stopSirenStream,
            sttProvider: s.sttProvider,
            sttReady: s.sttReady,
            agentName: s.agentName,
            isWakewordListening: s.isWakewordListening,
            lastWakeSpeaker: s.lastWakeSpeaker,
        })),
    );
    const [micError, setMicError] = useState<string | null>(null);

    const handleSelect = (mode: InputMode) => {
        if (isRecording) stopSirenStream();

        const useServerStt = sttReady || sttProvider !== 'browser';
        const SpeechRecognitionCtor = window.SpeechRecognition || window.webkitSpeechRecognition;

        if (mode === 'audio' || mode === 'conversation') {
            const isInsecure =
                window.location.protocol === 'http:' &&
                !['localhost', '127.0.0.1'].includes(window.location.hostname);
            if (isInsecure) {
                setMicError('El micrófono en navegador requiere http://localhost:8000 o HTTPS.');
                setInputMode(mode);
                return;
            }
            if (!useServerStt && !SpeechRecognitionCtor) {
                setMicError('Configurá Groq en Motores o usá Chrome/Edge para dictado.');
                setInputMode(mode);
                return;
            }
        }

        if (mode === 'wakeword') {
            if (!useServerStt && !SpeechRecognitionCtor) {
                setMicError('Activá Groq/Whisper local o usá Chrome/Edge para Wake Word.');
                return;
            }
        }

        setMicError(null);
        setInputMode(mode);
    };

    const effectiveName = agentName || 'Aegis';
    const wakewordTooltip = inputMode === 'wakeword'
        ? `Escuchando: "${effectiveName} escuchame"`
        : 'Activación por voz con reconocimiento de identidad';

    return (
        <div className="flex flex-col gap-1">
            <div className="flex items-center gap-0.5 bg-white/5 rounded-lg p-0.5">
                {MODES.map(({ mode, icon: Icon, label, activeColor, title }) => (
                    <button
                        key={mode}
                        onClick={() => handleSelect(mode)}
                        title={mode === 'wakeword' ? wakewordTooltip : title}
                        className={`p-1.5 rounded-md transition-all text-[10px] font-mono flex items-center gap-1 relative ${
                            inputMode === mode
                                ? activeColor + ' font-bold'
                                : 'text-white/30 hover:text-white/60'
                        }`}
                    >
                        <Icon className="w-3.5 h-3.5" />
                        <span className="hidden sm:inline">{label}</span>
                        {mode === 'wakeword' && isWakewordListening && (
                            <span className="absolute -top-0.5 -right-0.5 w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse" />
                        )}
                    </button>
                ))}
            </div>
            {micError && (
                <div className="flex items-start gap-1.5 px-1 py-1 bg-amber-500/10 border border-amber-500/20 rounded-md max-w-[220px]">
                    <AlertTriangle className="w-3 h-3 text-amber-400 flex-shrink-0 mt-0.5" />
                    <span className="text-[9px] font-mono text-amber-300 leading-tight">{micError}</span>
                </div>
            )}
            {inputMode === 'wakeword' && (
                <div className="px-1 text-[8px] font-mono text-amber-400/70 tracking-wider">
                    {isWakewordListening
                        ? `● escuchando "${effectiveName} escuchame"${lastWakeSpeaker ? ` · ${lastWakeSpeaker}` : ''}`
                        : `○ iniciando escucha…`}
                </div>
            )}
        </div>
    );
};

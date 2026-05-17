import React, { useState } from 'react';
import { Keyboard, Mic, MessageCircle, AlertTriangle, Wand2 } from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';

type InputMode = 'text' | 'audio' | 'conversation' | 'wakeword';

const MODES: { mode: InputMode; icon: React.FC<React.SVGProps<SVGSVGElement>>; label: string; activeColor: string; title: string }[] = [
    { mode: 'text',         icon: Keyboard,      label: 'Text',   activeColor: 'text-white bg-white/10',           title: 'Texto' },
    { mode: 'audio',        icon: Mic,            label: 'Audio',  activeColor: 'text-aegis-cyan bg-aegis-cyan/10', title: 'Audio (push-to-talk)' },
    { mode: 'conversation', icon: MessageCircle,  label: 'Conv',   activeColor: 'text-aegis-purple bg-aegis-purple/10', title: 'Conversación continua' },
    { mode: 'wakeword',     icon: Wand2,          label: 'Wake',   activeColor: 'text-amber-400 bg-amber-400/10',   title: 'Activación por nombre' },
];

export const InputModeSelector: React.FC = () => {
    const { inputMode, setInputMode, isRecording, stopSirenStream, sttProvider, agentName, isWakewordListening } = useAegisStore();
    const [micError, setMicError] = useState<string | null>(null);

    const handleSelect = (mode: InputMode) => {
        if (isRecording) stopSirenStream();

        if (mode === 'audio' || mode === 'conversation') {
            const isInsecure =
                window.location.protocol === 'http:' &&
                !['localhost', '127.0.0.1'].includes(window.location.hostname);
            if (isInsecure) {
                setMicError('El micrófono requiere HTTPS. Accedé via el link de Cloudflare.');
                setInputMode(mode);
                return;
            }
            const SpeechRecognitionCtor = window.SpeechRecognition || window.webkitSpeechRecognition;
            if (sttProvider === 'browser' && !SpeechRecognitionCtor) {
                setMicError('Tu navegador no soporta voz. Usá Chrome o Edge.');
                setInputMode(mode);
                return;
            }
        }

        if (mode === 'wakeword') {
            const SpeechRecognitionCtor = window.SpeechRecognition || window.webkitSpeechRecognition;
            if (!SpeechRecognitionCtor) {
                setMicError('Wake word requiere Chrome o Edge.');
                return;
            }
        }

        setMicError(null);
        setInputMode(mode);
    };

    const wakewordTooltip = inputMode === 'wakeword' && agentName
        ? `Escuchando: "${agentName}"`
        : 'Activación por nombre del agente';

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
            {inputMode === 'wakeword' && agentName && (
                <div className="px-1 text-[8px] font-mono text-amber-400/60 tracking-wider">
                    {isWakewordListening ? `● escuchando "${agentName}"` : `○ iniciando…`}
                </div>
            )}
        </div>
    );
};

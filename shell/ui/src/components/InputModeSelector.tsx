import React from 'react';
import { Keyboard, Mic, MessageCircle } from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';

type InputMode = 'text' | 'audio' | 'conversation';

const MODES: { mode: InputMode; icon: React.FC<React.SVGProps<SVGSVGElement>>; label: string; activeColor: string }[] = [
    { mode: 'text',         icon: Keyboard,      label: 'Text',  activeColor: 'text-white bg-white/10' },
    { mode: 'audio',        icon: Mic,            label: 'Audio', activeColor: 'text-aegis-cyan bg-aegis-cyan/10' },
    { mode: 'conversation', icon: MessageCircle,  label: 'Conv',  activeColor: 'text-aegis-purple bg-aegis-purple/10' },
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

import React from 'react';
import { Sun, Sparkles, Cpu, Shield, Flame, Bot } from 'lucide-react';
import { loadVoicePrefs } from '../audio/naturalVoice';
import { AVATAR_PRESETS } from '../constants/avatarPresets';

interface AssistantAvatarProps {
    avatarId?: string;
    customAvatarUrl?: string;
    size?: 'xs' | 'sm' | 'md' | 'lg';
    animated?: boolean;
    className?: string;
}

export const AssistantAvatar: React.FC<AssistantAvatarProps> = ({
    avatarId,
    customAvatarUrl,
    size = 'sm',
    animated = false,
    className = '',
}) => {
    const prefs = loadVoicePrefs();
    const resolvedId = avatarId ?? prefs.avatarId ?? 'orb-cyan';
    const resolvedCustomUrl = customAvatarUrl ?? prefs.customAvatarUrl ?? '';

    const sizeClasses = {
        xs: 'w-6 h-6 rounded-full',
        sm: 'w-8 h-8 rounded-full',
        md: 'w-11 h-11 rounded-2xl',
        lg: 'w-14 h-14 rounded-2xl',
    }[size];

    const iconSizeClasses = {
        xs: 'w-3.5 h-3.5',
        sm: 'w-4 h-4',
        md: 'w-5 h-5',
        lg: 'w-7 h-7',
    }[size];

    if (resolvedId === 'custom' && resolvedCustomUrl) {
        return (
            <div
                className={`${sizeClasses} overflow-hidden border border-aegis-cyan/40 shrink-0 bg-black/50 shadow-[0_0_12px_rgba(0,242,254,0.25)] ${
                    animated ? 'animate-pulse' : ''
                } ${className}`}
            >
                <img
                    src={resolvedCustomUrl}
                    alt="Avatar del Asistente"
                    className="w-full h-full object-cover"
                />
            </div>
        );
    }

    const preset = AVATAR_PRESETS.find((p) => p.id === resolvedId) ?? AVATAR_PRESETS[0];

    const renderIcon = () => {
        switch (preset.id) {
            case 'sol-amber':
                return <Sun className={`${iconSizeClasses} ${preset.textClass}`} />;
            case 'orb-cyan':
                return <Cpu className={`${iconSizeClasses} ${preset.textClass}`} />;
            case 'nova-purple':
                return <Sparkles className={`${iconSizeClasses} ${preset.textClass}`} />;
            case 'atlas-emerald':
                return <Shield className={`${iconSizeClasses} ${preset.textClass}`} />;
            case 'crimson-flame':
                return <Flame className={`${iconSizeClasses} ${preset.textClass}`} />;
            default:
                return <Bot className={`${iconSizeClasses} ${preset.textClass}`} />;
        }
    };

    return (
        <div
            className={`${sizeClasses} ${preset.bgClass} ${preset.borderClass} ${preset.glowClass} border flex items-center justify-center shrink-0 transition-all ${
                animated ? 'animate-pulse' : ''
            } ${className}`}
        >
            {renderIcon()}
        </div>
    );
};

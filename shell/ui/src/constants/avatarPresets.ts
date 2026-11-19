export interface AvatarPreset {
    id: string;
    name: string;
    desc: string;
    bgClass: string;
    borderClass: string;
    textClass: string;
    glowClass: string;
}

export const AVATAR_PRESETS: AvatarPreset[] = [
    {
        id: 'sol-amber',
        name: 'Ámbar Radiante',
        desc: 'Cálido, cercano y luminoso',
        bgClass: 'bg-gradient-to-br from-amber-400/30 via-orange-500/20 to-yellow-500/30',
        borderClass: 'border-amber-400/50',
        textClass: 'text-amber-300',
        glowClass: 'shadow-[0_0_15px_rgba(251,191,36,0.3)]',
    },
    {
        id: 'orb-cyan',
        name: 'Núcleo Cuántico',
        desc: 'Futurista, preciso y sereno',
        bgClass: 'bg-gradient-to-br from-cyan-400/30 via-sky-500/20 to-blue-600/30',
        borderClass: 'border-aegis-cyan/50',
        textClass: 'text-aegis-cyan',
        glowClass: 'shadow-[0_0_15px_rgba(0,242,254,0.3)]',
    },
    {
        id: 'nova-purple',
        name: 'Nova Neural',
        desc: 'Creativa, expresiva y moderna',
        bgClass: 'bg-gradient-to-br from-purple-500/30 via-fuchsia-500/20 to-pink-500/30',
        borderClass: 'border-purple-400/50',
        textClass: 'text-purple-300',
        glowClass: 'shadow-[0_0_15px_rgba(192,132,252,0.3)]',
    },
    {
        id: 'atlas-emerald',
        name: 'Atlas Ejecutivo',
        desc: 'Analítico, ejecutivo y equilibrado',
        bgClass: 'bg-gradient-to-br from-emerald-400/30 via-teal-500/20 to-green-600/30',
        borderClass: 'border-emerald-400/50',
        textClass: 'text-emerald-300',
        glowClass: 'shadow-[0_0_15px_rgba(52,211,153,0.3)]',
    },
    {
        id: 'crimson-flame',
        name: 'Fénix Carmesí',
        desc: 'Audaz, sarcástico y con carácter',
        bgClass: 'bg-gradient-to-br from-rose-500/30 via-red-500/20 to-orange-600/30',
        borderClass: 'border-rose-400/50',
        textClass: 'text-rose-300',
        glowClass: 'shadow-[0_0_15px_rgba(251,113,133,0.3)]',
    },
    {
        id: 'minimal-bot',
        name: 'Androide Clásico',
        desc: 'Minimalista y directo',
        bgClass: 'bg-gradient-to-br from-slate-400/25 via-zinc-500/20 to-neutral-700/30',
        borderClass: 'border-white/30',
        textClass: 'text-white/90',
        glowClass: 'shadow-[0_0_12px_rgba(255,255,255,0.15)]',
    },
];

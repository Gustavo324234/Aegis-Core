/**
 * naturalVoice.ts
 * Motor unificado y simplificado de Voz Natural (TTS + STT) y Personalización Visual (Avatar/Voz)
 * para Aegis Personal Assistant.
 * - Soporta selección de género de voz (Femenina / Masculina) y estilos de entonación.
 * - Selecciona automáticamente voces Neurales / Online (Edge Natural: Elena/Tomás/Dalia/Álvaro,
 *   Google Español, Apple Enhanced) evitando las voces robóticas SAPI5 de escritorio.
 * - Limpia Markdown, bloques de código, URLs y símbolos antes de hablar.
 * - Divide respuestas largas en oraciones naturales para evitar el corte de 15s de Chrome.
 */

export type VoiceGender = 'female' | 'male';

export interface VoicePrefs {
    voiceURI: string;          // '' = selección automática inteligente según género e idioma
    voiceGender: VoiceGender;  // 'female' | 'male'
    voicePreset: string;       // 'female-warm' | 'female-dynamic' | 'male-natural' | 'male-deep' | 'custom'
    rate: number;              // default 1.02
    pitch: number;             // default 1.0
    lang: string;              // default 'es-AR'
    autoContinueMic: boolean;  // Modo conversación manos libres
    avatarId: string;          // 'sol-amber' | 'orb-cyan' | 'nova-purple' | 'atlas-emerald' | 'crimson-flame' | 'minimal-bot' | 'custom'
    customAvatarUrl: string;   // Data URL si el usuario subió su propia imagen
}

export interface VoiceStylePreset {
    id: string;
    label: string;
    desc: string;
    gender: VoiceGender;
    rate: number;
    pitch: number;
}

export const VOICE_STYLE_PRESETS: VoiceStylePreset[] = [
    {
        id: 'female-warm',
        label: 'Femenina Cálida',
        desc: 'Tono cercano, fluido y natural',
        gender: 'female',
        rate: 1.02,
        pitch: 1.0,
    },
    {
        id: 'female-dynamic',
        label: 'Femenina Dinámica',
        desc: 'Ágil, expresiva y enérgica',
        gender: 'female',
        rate: 1.12,
        pitch: 1.05,
    },
    {
        id: 'male-natural',
        label: 'Masculina Serena',
        desc: 'Voz masculina clara y equilibrada',
        gender: 'male',
        rate: 1.0,
        pitch: 0.95,
    },
    {
        id: 'male-deep',
        label: 'Masculina Profunda',
        desc: 'Tono grave, calmo y ejecutivo',
        gender: 'male',
        rate: 0.96,
        pitch: 0.85,
    },
];

const PREFS_STORAGE_KEY = 'aegis_natural_voice_prefs';

const DEFAULT_PREFS: VoicePrefs = {
    voiceURI: '',
    voiceGender: 'female',
    voicePreset: 'female-warm',
    rate: 1.02,
    pitch: 1.0,
    lang: 'es-AR',
    autoContinueMic: false,
    avatarId: 'orb-cyan',
    customAvatarUrl: '',
};

export function loadVoicePrefs(): VoicePrefs {
    try {
        const raw = localStorage.getItem(PREFS_STORAGE_KEY);
        if (!raw) return { ...DEFAULT_PREFS };
        return { ...DEFAULT_PREFS, ...JSON.parse(raw) };
    } catch {
        return { ...DEFAULT_PREFS };
    }
}

export function saveVoicePrefs(prefs: Partial<VoicePrefs>): VoicePrefs {
    const merged = { ...loadVoicePrefs(), ...prefs };
    try {
        localStorage.setItem(PREFS_STORAGE_KEY, JSON.stringify(merged));
        if (typeof window !== 'undefined') {
            window.dispatchEvent(new CustomEvent('aegis-voice-prefs-changed', { detail: merged }));
        }
    } catch {
        /* ignore */
    }
    return merged;
}

const MALE_VOICE_PATTERNS = /\b(tomas|tomás|alvaro|álvaro|jorge|alonso|gonzalo|lorenzo|emilio|dario|darío|pablo|raul|raúl|carlos|diego|juan|andres|andrés|mateo|marcelo|federico|nicolas|nicolás|alejandro|rodrigo|manuel|javier|miguel|fernando|male|hombre|masculin|guy|christopher|eric|brian|andrew|david|mark|daniel|george|james|richard|roger|steffan|rishi|aaron|reed|rocko|eddy)\b/i;

const FEMALE_VOICE_PATTERNS = /\b(elena|dalia|elvira|paloma|salome|salomé|catalina|sabina|helena|laura|paulina|monica|mónica|luciana|sofia|sofía|camila|valeria|andrea|ximena|beatriz|renata|carmen|paola|gabriela|clara|belen|belén|female|mujer|femenin|aria|jenny|michelle|sonia|zira|hazel|samantha|victoria|karen|ava|emma|flo|sandy|shelley)\b/i;

export function detectVoiceGender(v: SpeechSynthesisVoice): 'female' | 'male' | 'neutral' {
    const name = v.name.toLowerCase();
    if (MALE_VOICE_PATTERNS.test(name)) return 'male';
    if (FEMALE_VOICE_PATTERNS.test(name)) return 'female';
    return 'neutral';
}

/**
 * Limpia Markdown, código, URLs y símbolos para que la síntesis de voz
 * lea el texto con fluidez humana en lugar de deletrear asteriscos o símbolos.
 */
export function cleanMarkdownForSpeech(raw: string): string {
    if (!raw) return '';
    return raw
        .replace(/```[\s\S]*?```/g, ' ')
        .replace(/`([^`]+)`/g, '$1')
        .replace(/!\[[^\]]*\]\([^)]+\)/g, '')
        .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
        .replace(/https?:\/\/\S+/g, '')
        .replace(/^#{1,6}\s+/gm, '')
        .replace(/\*\*([^*]+)\*\*/g, '$1')
        .replace(/\*([^*]+)\*/g, '$1')
        .replace(/__([^_]+)__/g, '$1')
        .replace(/_([^_]+)_/g, '$1')
        .replace(/~~([^~]+)~~/g, '$1')
        .replace(/^\s*[-*•·]\s+/gm, ', ')
        .replace(/---+|\*\*\*+/g, ' ')
        .replace(/[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/gu, '')
        .replace(/\n+/g, '. ')
        .replace(/\s{2,}/g, ' ')
        .replace(/\.\s*\./g, '.')
        .trim();
}

/**
 * Puntúa qué tan natural es una voz del navegador para priorizar voces Neurales/Cloud
 * sobre las voces robóticas locales (SAPI5 Desktop), respetando el género preferido.
 */
export function scoreVoiceNaturalness(
    v: SpeechSynthesisVoice,
    preferredLang: string = 'es',
    preferredGender?: VoiceGender,
): number {
    const langLower = v.lang.toLowerCase();
    const nameLower = v.name.toLowerCase();
    const targetPrefix = preferredLang.slice(0, 2).toLowerCase();

    if (!langLower.startsWith(targetPrefix)) {
        return -100;
    }

    let score = 10;

    // 1. Voces Neurales / Natural Online de Microsoft Edge (las más humanas del mercado gratis)
    if (nameLower.includes('natural') || nameLower.includes('online')) {
        score += 100;
    }

    // 2. Voces Cloud de Google Chrome ("Google español", "Google español de Estados Unidos")
    if (nameLower.includes('google')) {
        score += 80;
    }

    // 3. Voces Premium/Enhanced de macOS/iOS
    if (nameLower.includes('premium') || nameLower.includes('enhanced') || nameLower.includes('siri')) {
        score += 75;
    }
    if (nameLower.includes('paulina') || nameLower.includes('mónica') || nameLower.includes('monica') || nameLower.includes('luciana') || nameLower.includes('jorge') || nameLower.includes('juan') || nameLower.includes('diego')) {
        score += 35;
    }

    // 4. Preferencia regional rioplatense / latinoamericana
    if (langLower === 'es-ar' || nameLower.includes('elena') || nameLower.includes('tomas') || nameLower.includes('tomás')) {
        score += 25;
    } else if (langLower === 'es-mx' || langLower === 'es-us' || langLower === 'es-419' || nameLower.includes('dalia') || nameLower.includes('jorge') || nameLower.includes('alonso') || nameLower.includes('paloma')) {
        score += 15;
    } else if (langLower.startsWith('es')) {
        score += 10;
    }

    // 5. Ajuste por género de voz elegido (Femenina vs Masculina)
    if (preferredGender) {
        const detected = detectVoiceGender(v);
        if (detected === preferredGender) {
            score += 90;
        } else if (detected !== 'neutral') {
            score -= 80;
        }
    }

    // 6. Penalizar fuertemente voces SAPI5 Desktop viejas ("Microsoft Sabina Desktop", "Helena Desktop")
    if (nameLower.includes('desktop')) {
        score -= 50;
    }
    if (v.localService && !nameLower.includes('natural') && !nameLower.includes('google')) {
        score -= 20;
    }

    return score;
}

export interface ScoredVoice {
    voice: SpeechSynthesisVoice;
    score: number;
    isNatural: boolean;
    gender: 'female' | 'male' | 'neutral';
    label: string;
}

export function getSortedVoices(langPrefix: string = 'es', preferredGender?: VoiceGender): SpeechSynthesisVoice[] {
    if (typeof window === 'undefined' || !window.speechSynthesis) return [];
    const all = window.speechSynthesis.getVoices();
    const matching = all.filter(v => v.lang.toLowerCase().startsWith(langPrefix.slice(0, 2).toLowerCase()));
    const pool = matching.length > 0 ? matching : all;
    return [...pool].sort(
        (a, b) => scoreVoiceNaturalness(b, langPrefix, preferredGender) - scoreVoiceNaturalness(a, langPrefix, preferredGender)
    );
}

export function getScoredVoices(langPrefix: string = 'es', preferredGender?: VoiceGender): ScoredVoice[] {
    return getSortedVoices(langPrefix, preferredGender).map((voice) => {
        const baseScore = scoreVoiceNaturalness(voice, langPrefix);
        const score = scoreVoiceNaturalness(voice, langPrefix, preferredGender);
        const isNatural = baseScore >= 50;
        const gender = detectVoiceGender(voice);
        const cleanName = voice.name
            .replace(/^Microsoft\s+/i, '')
            .replace(/\s+Online\s+\(Natural\)\s*-?\s*/i, ' (Neural) - ');
        return {
            voice,
            score,
            isNatural,
            gender,
            label: cleanName,
        };
    });
}

export function selectBestVoice(langPrefix: string = 'es'): SpeechSynthesisVoice | null {
    if (typeof window === 'undefined' || !window.speechSynthesis) return null;
    const prefs = loadVoicePrefs();
    const all = window.speechSynthesis.getVoices();
    if (all.length === 0) return null;

    if (prefs.voiceURI) {
        const explicit = all.find(v => v.voiceURI === prefs.voiceURI || v.name === prefs.voiceURI);
        if (explicit) return explicit;
    }

    const sorted = getSortedVoices(langPrefix, prefs.voiceGender);
    return sorted[0] ?? null;
}

/**
 * Divide un texto largo en fragmentos por oración (máx ~180 caracteres)
 * para evitar el bug de Chrome donde speechSynthesis se corta a los ~15 segundos.
 */
function splitIntoNaturalChunks(text: string): string[] {
    const rawSentences = text
        .split(/(?<=[.!?¡¿…])\s+/)
        .map(s => s.trim())
        .filter(Boolean);

    const chunks: string[] = [];
    for (const sentence of rawSentences) {
        if (sentence.length <= 180) {
            chunks.push(sentence);
        } else {
            const subParts = sentence.split(/(?<=[,;:])\s+/);
            let current = '';
            for (const part of subParts) {
                if ((current + ' ' + part).trim().length <= 180) {
                    current = (current + ' ' + part).trim();
                } else {
                    if (current) chunks.push(current);
                    current = part.trim();
                }
            }
            if (current) chunks.push(current);
        }
    }
    return chunks.length > 0 ? chunks : [text];
}

export function detectLanguage(text: string): string {
    const spanishChars = /[áéíóúñÁÉÍÓÚÑ¿¡]/;
    const commonSpanish = /\b(el|la|los|las|un|una|en|es|y|o|pero|por|para|con|que|como|cómo|hola|soy|puedo|ayudar)\b/i;
    return spanishChars.test(text) || commonSpanish.test(text) ? 'es-AR' : 'en-US';
}

let activePlaybackToken = 0;

export function stopSpeaking(): void {
    activePlaybackToken++;
    if (typeof window !== 'undefined' && window.speechSynthesis) {
        try {
            window.speechSynthesis.cancel();
        } catch {
            /* ignore */
        }
    }
}

/**
 * Habla un texto usando la mejor voz neural disponible, aplicando género,
 * velocidad, tono y limpieza de Markdown.
 */
export function speakNaturalText(
    rawText: string,
    onDone?: () => void,
): void {
    if (typeof window === 'undefined' || !window.speechSynthesis) {
        onDone?.();
        return;
    }

    const clean = cleanMarkdownForSpeech(rawText);
    if (!clean) {
        onDone?.();
        return;
    }

    const token = ++activePlaybackToken;
    window.speechSynthesis.cancel();

    const prefs = loadVoicePrefs();
    const detectedLang = detectLanguage(clean);
    const langPrefix = detectedLang.slice(0, 2);
    const bestVoice = selectBestVoice(langPrefix);

    // Si el usuario pidió voz masculina pero el navegador solo tiene una voz neutra/femenina
    // (como "Google español" en Chrome sin voces adicionales instaladas), bajamos sutilmente el pitch
    // para darle un timbre más masculino.
    let effectivePitch = prefs.pitch;
    if (prefs.voiceGender === 'male' && bestVoice && detectVoiceGender(bestVoice) !== 'male') {
        effectivePitch = Math.min(prefs.pitch, 0.80);
    }

    const chunks = splitIntoNaturalChunks(clean);
    let currentIdx = 0;
    let finished = false;

    const finishOnce = () => {
        if (finished) return;
        finished = true;
        if (token === activePlaybackToken) {
            onDone?.();
        }
    };

    const wordCount = clean.split(/\s+/).length;
    const safetyMs = Math.min(90_000, Math.max(8_000, (wordCount / 2.2) * 1000 + 6_000));
    const safetyTimer = setTimeout(finishOnce, safetyMs);

    const speakNextChunk = () => {
        if (token !== activePlaybackToken) {
            clearTimeout(safetyTimer);
            return;
        }
        if (currentIdx >= chunks.length) {
            clearTimeout(safetyTimer);
            finishOnce();
            return;
        }

        const chunkText = chunks[currentIdx++];
        const utterance = new SpeechSynthesisUtterance(chunkText);

        if (bestVoice) {
            utterance.voice = bestVoice;
            utterance.lang = bestVoice.lang;
        } else {
            utterance.lang = prefs.lang || detectedLang;
        }

        utterance.rate = prefs.rate;
        utterance.pitch = effectivePitch;

        utterance.onend = () => {
            speakNextChunk();
        };
        utterance.onerror = () => {
            speakNextChunk();
        };

        window.speechSynthesis.speak(utterance);
    };

    if (window.speechSynthesis.getVoices().length === 0) {
        const handleVoicesLoaded = () => {
            window.speechSynthesis.removeEventListener('voiceschanged', handleVoicesLoaded);
            speakNextChunk();
        };
        window.speechSynthesis.addEventListener('voiceschanged', handleVoicesLoaded);
        setTimeout(() => {
            window.speechSynthesis.removeEventListener('voiceschanged', handleVoicesLoaded);
            if (currentIdx === 0 && token === activePlaybackToken) {
                speakNextChunk();
            }
        }, 400);
    } else {
        speakNextChunk();
    }
}

import React, { useState, useRef, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Mic, MicOff, Check, X, AlertTriangle, Loader } from 'lucide-react';

interface Props {
    tenantId: string;
    sessionKey: string;
    onSuccess: () => void;
    onClose: () => void;
}

type Phase = 'idle' | 'recording' | 'processing' | 'success' | 'error';

const RECORD_SECONDS = 5;

const SpeakerEnrollModal: React.FC<Props> = ({ tenantId, sessionKey, onSuccess, onClose }) => {
    const [phase, setPhase] = useState<Phase>('idle');
    const [countdown, setCountdown] = useState(RECORD_SECONDS);
    const [errorMsg, setErrorMsg] = useState('');
    const [level, setLevel] = useState(0);

    const mediaRef = useRef<MediaRecorder | null>(null);
    const chunksRef = useRef<Blob[]>([]);
    const analyserRef = useRef<AnalyserNode | null>(null);
    const animFrameRef = useRef<number>(0);
    const countdownRef = useRef<ReturnType<typeof setInterval> | null>(null);

    useEffect(() => {
        return () => {
            if (animFrameRef.current) cancelAnimationFrame(animFrameRef.current);
            if (countdownRef.current) clearInterval(countdownRef.current);
            if (mediaRef.current?.state === 'recording') mediaRef.current.stop();
        };
    }, []);

    const animateLevel = () => {
        if (!analyserRef.current) return;
        const data = new Uint8Array(analyserRef.current.frequencyBinCount);
        analyserRef.current.getByteFrequencyData(data);
        const avg = data.reduce((s, v) => s + v, 0) / data.length;
        setLevel(avg / 128);
        animFrameRef.current = requestAnimationFrame(animateLevel);
    };

    const startRecording = async () => {
        setErrorMsg('');
        setPhase('recording');
        setCountdown(RECORD_SECONDS);
        chunksRef.current = [];

        try {
            const stream = await navigator.mediaDevices.getUserMedia({
                audio: { sampleRate: 16000, channelCount: 1, echoCancellation: true }
            });

            const ctx = new AudioContext({ sampleRate: 16000 });
            const source = ctx.createMediaStreamSource(stream);
            const analyser = ctx.createAnalyser();
            analyser.fftSize = 256;
            source.connect(analyser);
            analyserRef.current = analyser;
            animFrameRef.current = requestAnimationFrame(animateLevel);

            const recorder = new MediaRecorder(stream);
            mediaRef.current = recorder;
            recorder.ondataavailable = (e) => { if (e.data.size > 0) chunksRef.current.push(e.data); };
            recorder.onstop = async () => {
                cancelAnimationFrame(animFrameRef.current);
                stream.getTracks().forEach(t => t.stop());
                ctx.close();
                setLevel(0);
                await submitEnrollment();
            };
            recorder.start();

            let remaining = RECORD_SECONDS;
            countdownRef.current = setInterval(() => {
                remaining -= 1;
                setCountdown(remaining);
                if (remaining <= 0) {
                    clearInterval(countdownRef.current!);
                    recorder.stop();
                }
            }, 1000);
        } catch {
            setPhase('error');
            setErrorMsg('No se pudo acceder al micrófono. Verificá los permisos del navegador.');
        }
    };

    const submitEnrollment = async () => {
        setPhase('processing');
        try {
            const blob = new Blob(chunksRef.current, { type: 'audio/webm' });
            const arrayBuffer = await blob.arrayBuffer();
            const bytes = new Uint8Array(arrayBuffer);
            const b64 = btoa(String.fromCharCode(...bytes));

            const res = await fetch('/api/siren/enroll', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({ pcm_b64: b64 }),
            });

            if (res.ok) {
                setPhase('success');
                setTimeout(() => { onSuccess(); onClose(); }, 1500);
            } else {
                const err = await res.json().catch(() => ({}));
                setPhase('error');
                setErrorMsg(err.detail || 'Error al guardar el perfil de voz.');
            }
        } catch {
            setPhase('error');
            setErrorMsg('Error de red al enviar el audio.');
        }
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm" onClick={onClose}>
            <motion.div
                initial={{ scale: 0.9, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                exit={{ scale: 0.9, opacity: 0 }}
                className="glass border border-white/10 rounded-2xl p-8 w-full max-w-sm shadow-2xl space-y-6"
                onClick={(e) => e.stopPropagation()}
            >
                <div className="flex items-center justify-between">
                    <h3 className="text-sm font-mono font-bold uppercase tracking-widest text-white">
                        Enrollment de Voz
                    </h3>
                    <button onClick={onClose} className="text-white/30 hover:text-white/60 transition-colors">
                        <X className="w-4 h-4" />
                    </button>
                </div>

                <p className="text-[10px] font-mono text-white/40 leading-relaxed">
                    Habla normalmente durante {RECORD_SECONDS} segundos para registrar tu perfil de voz.
                    Aegis usará esta huella para verificar tu identidad antes de procesar audio.
                </p>

                {/* Visualizador */}
                <div className="flex justify-center items-center h-24">
                    <AnimatePresence mode="wait">
                        {phase === 'recording' && (
                            <motion.div key="recording" className="relative flex items-center justify-center">
                                <motion.div
                                    className="absolute rounded-full bg-red-500/20"
                                    animate={{ scale: [1, 1 + level * 1.5, 1] }}
                                    transition={{ duration: 0.15, repeat: Infinity }}
                                    style={{ width: 80, height: 80 }}
                                />
                                <div className="relative z-10 p-4 rounded-full bg-red-500/30 border border-red-500/50">
                                    <Mic className="w-8 h-8 text-red-400" />
                                </div>
                                <span className="absolute -bottom-6 text-2xl font-mono font-bold text-red-400">
                                    {countdown}s
                                </span>
                            </motion.div>
                        )}
                        {phase === 'idle' && (
                            <motion.div key="idle" className="p-4 rounded-full bg-aegis-cyan/10 border border-aegis-cyan/30">
                                <MicOff className="w-8 h-8 text-aegis-cyan/60" />
                            </motion.div>
                        )}
                        {phase === 'processing' && (
                            <motion.div key="processing" className="p-4 rounded-full bg-white/5 border border-white/10">
                                <Loader className="w-8 h-8 text-aegis-cyan animate-spin" />
                            </motion.div>
                        )}
                        {phase === 'success' && (
                            <motion.div key="success" initial={{ scale: 0 }} animate={{ scale: 1 }}
                                className="p-4 rounded-full bg-green-500/20 border border-green-500/40">
                                <Check className="w-8 h-8 text-green-400" />
                            </motion.div>
                        )}
                        {phase === 'error' && (
                            <motion.div key="error" className="p-4 rounded-full bg-red-500/10 border border-red-500/30">
                                <AlertTriangle className="w-8 h-8 text-red-400" />
                            </motion.div>
                        )}
                    </AnimatePresence>
                </div>

                {phase === 'error' && (
                    <p className="text-[10px] font-mono text-red-400 text-center leading-tight">{errorMsg}</p>
                )}

                {phase === 'success' && (
                    <p className="text-[10px] font-mono text-green-400 text-center uppercase tracking-widest">
                        Perfil de voz guardado
                    </p>
                )}

                {(phase === 'idle' || phase === 'error') && (
                    <button
                        onClick={startRecording}
                        className="w-full py-4 rounded-xl bg-aegis-cyan/10 hover:bg-aegis-cyan/20 border border-aegis-cyan/40 transition-all font-mono text-xs font-bold uppercase tracking-widest text-aegis-cyan"
                    >
                        {phase === 'error' ? 'Reintentar' : 'Iniciar Grabación'}
                    </button>
                )}
            </motion.div>
        </div>
    );
};

export default SpeakerEnrollModal;

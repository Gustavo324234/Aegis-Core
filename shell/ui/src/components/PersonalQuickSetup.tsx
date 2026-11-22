import React, { useState } from 'react';
import { motion } from 'framer-motion';
import { Shield, Loader2, Sparkles, User, Lock, Key, CheckCircle2, AlertCircle } from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';

interface PersonalQuickSetupProps {
    setupToken?: string | null;
}

const PROVIDERS = [
    { id: 'gemini', name: 'Google Gemini (Recomendado)', defaultModel: 'gemini-2.5-flash', requiresKey: true, placeholder: 'AIzaSy...' },
    { id: 'openrouter', name: 'OpenRouter', defaultModel: 'meta-llama/llama-3.3-70b-instruct', requiresKey: true, placeholder: 'sk-or-v1-...' },
    { id: 'groq', name: 'Groq (Ultra rápido)', defaultModel: 'groq/llama-3.3-70b-versatile', requiresKey: true, placeholder: 'gsk_...' },
    { id: 'ollama', name: 'Ollama Local (Gratis sin API Key)', defaultModel: 'llama3.2', requiresKey: false, placeholder: '' },
    { id: 'openai', name: 'OpenAI', defaultModel: 'openai/gpt-4o-mini', requiresKey: true, placeholder: 'sk-proj-...' },
    { id: 'anthropic', name: 'Anthropic Claude (vía OpenRouter)', defaultModel: 'anthropic/claude-sonnet-4-6', requiresKey: true, placeholder: 'sk-or-v1-...' },
];

export const PersonalQuickSetup: React.FC<PersonalQuickSetupProps> = ({ setupToken }) => {
    const { setAuth, setEngineConfigured, fetchSystemState } = useAegisStore();
    const [token, setToken] = useState(setupToken ?? '');
    const [username, setUsername] = useState('');
    const [passphrase, setPassphrase] = useState('');
    const [provider, setProvider] = useState('gemini');
    const [apiKey, setApiKey] = useState('');
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [success, setSuccess] = useState(false);

    const activeProvider = PROVIDERS.find(p => p.id === provider) || PROVIDERS[0];

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setError(null);

        const cleanToken = token.trim();
        if (!cleanToken) {
            setError('Ingresá tu Setup Token (consola de inicio o "aegis setup-url").');
            return;
        }

        const cleanUsername = username.trim().toLowerCase();
        if (!cleanUsername) {
            setError('Ingresá tu nombre o usuario.');
            return;
        }

        if (passphrase.length < 8) {
            setError('La contraseña debe tener al menos 8 caracteres.');
            return;
        }

        if (activeProvider.requiresKey && !apiKey.trim()) {
            setError(`Ingresá tu API Key para ${activeProvider.name}.`);
            return;
        }

        setIsSubmitting(true);

        try {
            const payload: Record<string, unknown> = {
                username: cleanUsername,
                passphrase: passphrase,
                provider: provider,
                model_name: activeProvider.defaultModel,
                setup_token: cleanToken,
            };

            if (activeProvider.requiresKey) {
                payload.api_key = apiKey.trim();
            }

            const res = await fetch('/api/auth/quick-personal-setup', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(payload),
            });

            if (res.ok) {
                const data = (await res.json().catch(() => ({}))) as { session_token?: string };
                setSuccess(true);
                // Iniciar sesión y marcar motor configurado
                setAuth(cleanUsername, data.session_token || passphrase);
                setEngineConfigured(true);
                await fetchSystemState();
            } else {
                const data = await res.json().catch(() => ({}));
                const detail = (data as { detail?: string; error?: string; message?: string }).detail ||
                               (data as { error?: string }).error ||
                               (data as { message?: string }).message ||
                               'Error durante la configuración inicial.';
                setError(detail);
            }
        } catch {
            setError('Error de comunicación con el servidor de Aegis.');
        } finally {
            setIsSubmitting(false);
        }
    };

    if (success) {
        return (
            <div className="flex flex-col items-center justify-center min-h-screen bg-black p-6">
                <motion.div 
                    initial={{ scale: 0.9, opacity: 0 }}
                    animate={{ scale: 1, opacity: 1 }}
                    className="glass p-10 rounded-3xl border border-emerald-500/30 flex flex-col items-center text-center max-w-md space-y-4"
                >
                    <div className="p-4 rounded-full bg-emerald-500/10">
                        <CheckCircle2 className="w-12 h-12 text-emerald-400" />
                    </div>
                    <h1 className="text-xl font-bold tracking-wider uppercase text-white">¡Bienvenido a Aegis!</h1>
                    <p className="text-xs font-mono text-white/50 leading-relaxed">
                        Tu asistente personal está listo. Entrando al chat...
                    </p>
                </motion.div>
            </div>
        );
    }

    return (
        <div className="flex flex-col items-center justify-center min-h-screen bg-black p-6 relative overflow-hidden">
            {/* Ambient Background Glow */}
            <div className="absolute top-0 left-0 w-full h-full pointer-events-none">
                <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[700px] h-[700px] bg-aegis-cyan/10 rounded-full blur-[140px]" />
                <div className="absolute top-0 left-0 w-full h-full bg-[radial-gradient(circle_at_50%_50%,rgba(255,255,255,0.03)_0%,transparent_100%)]" />
            </div>

            <motion.div 
                initial={{ y: 20, opacity: 0 }}
                animate={{ y: 0, opacity: 1 }}
                className="w-full max-w-md relative z-10"
            >
                {/* Header Card */}
                <div className="flex flex-col items-center mb-8 text-center">
                    <div className="p-4 rounded-2xl bg-white/5 border border-white/10 mb-4 shadow-xl">
                        <Shield className="w-10 h-10 text-aegis-cyan" />
                    </div>
                    <h1 className="text-2xl font-bold tracking-wider text-white uppercase flex items-center gap-2">
                        <span>Aegis</span>
                        <span className="text-aegis-cyan">Personal</span>
                    </h1>
                    <p className="text-xs font-mono text-white/50 mt-1 max-w-xs">
                        Configurá tu asistente en 1 solo paso y empezá a chatear.
                    </p>
                </div>

                {/* Form Card */}
                <div className="glass p-8 rounded-3xl border border-white/10 shadow-2xl space-y-6">
                    {error && (
                        <div className="p-3.5 rounded-xl bg-red-500/10 border border-red-500/20 flex items-center gap-2 text-xs font-mono text-red-400">
                            <AlertCircle className="w-4 h-4 shrink-0" />
                            <span>{error}</span>
                        </div>
                    )}

                    <form onSubmit={handleSubmit} className="space-y-4">
                        {/* Setup Token */}
                        <div className="space-y-1.5">
                            <label className="text-[11px] font-mono uppercase tracking-wider text-white/60 flex items-center gap-1.5">
                                <Shield className="w-3.5 h-3.5 text-aegis-cyan" />
                                <span>Setup Token</span>
                            </label>
                            <input
                                type="text"
                                value={token}
                                onChange={(e) => setToken(e.target.value)}
                                placeholder="SETUP-XXXX-XXXX"
                                required
                                autoFocus={!setupToken}
                                className="w-full px-4 py-2.5 rounded-xl bg-white/5 border border-white/10 text-white placeholder-white/20 text-sm focus:outline-none focus:border-aegis-cyan transition-colors font-mono"
                            />
                            <p className="text-[10px] font-mono text-white/40">
                                Obtené tu token en la consola de inicio o ejecutando <code className="text-white/60">aegis setup-url</code>.
                            </p>
                        </div>

                        {/* Username */}
                        <div className="space-y-1.5">
                            <label className="text-[11px] font-mono uppercase tracking-wider text-white/60 flex items-center gap-1.5">
                                <User className="w-3.5 h-3.5 text-aegis-cyan" />
                                <span>Tu Nombre o Usuario</span>
                            </label>
                            <input
                                type="text"
                                value={username}
                                onChange={(e) => setUsername(e.target.value)}
                                placeholder="ej. alex"
                                required
                                autoFocus={Boolean(setupToken)}
                                className="w-full px-4 py-2.5 rounded-xl bg-white/5 border border-white/10 text-white placeholder-white/20 text-sm focus:outline-none focus:border-aegis-cyan transition-colors"
                            />
                        </div>

                        {/* Password */}
                        <div className="space-y-1.5">
                            <label className="text-[11px] font-mono uppercase tracking-wider text-white/60 flex items-center gap-1.5">
                                <Lock className="w-3.5 h-3.5 text-aegis-cyan" />
                                <span>Tu Contraseña (mínimo 8 caracteres)</span>
                            </label>
                            <input
                                type="password"
                                value={passphrase}
                                onChange={(e) => setPassphrase(e.target.value)}
                                placeholder="••••••••"
                                required
                                minLength={8}
                                className="w-full px-4 py-2.5 rounded-xl bg-white/5 border border-white/10 text-white placeholder-white/20 text-sm focus:outline-none focus:border-aegis-cyan transition-colors"
                            />
                        </div>

                        {/* AI Provider */}
                        <div className="space-y-1.5">
                            <label className="text-[11px] font-mono uppercase tracking-wider text-white/60 flex items-center gap-1.5">
                                <Sparkles className="w-3.5 h-3.5 text-aegis-purple" />
                                <span>Proveedor de IA</span>
                            </label>
                            <select
                                value={provider}
                                onChange={(e) => setProvider(e.target.value)}
                                className="w-full px-4 py-2.5 rounded-xl bg-white/5 border border-white/10 text-white text-sm focus:outline-none focus:border-aegis-cyan transition-colors"
                            >
                                {PROVIDERS.map((p) => (
                                    <option key={p.id} value={p.id} className="bg-neutral-900 text-white">
                                        {p.name}
                                    </option>
                                ))}
                            </select>
                        </div>

                        {/* API Key (if required) */}
                        {activeProvider.requiresKey && (
                            <div className="space-y-1.5">
                                <label className="text-[11px] font-mono uppercase tracking-wider text-white/60 flex items-center gap-1.5">
                                    <Key className="w-3.5 h-3.5 text-emerald-400" />
                                    <span>API Key ({activeProvider.name})</span>
                                </label>
                                <input
                                    type="password"
                                    value={apiKey}
                                    onChange={(e) => setApiKey(e.target.value)}
                                    placeholder={activeProvider.placeholder}
                                    required
                                    className="w-full px-4 py-2.5 rounded-xl bg-white/5 border border-white/10 text-white placeholder-white/20 text-sm focus:outline-none focus:border-aegis-cyan transition-colors font-mono"
                                />
                            </div>
                        )}

                        {/* Submit Button */}
                        <button
                            type="submit"
                            disabled={isSubmitting}
                            className="w-full mt-4 py-3 rounded-xl bg-aegis-cyan hover:bg-aegis-cyan/90 text-black font-bold uppercase tracking-wider text-xs flex items-center justify-center gap-2 shadow-lg shadow-aegis-cyan/20 transition-all disabled:opacity-50"
                        >
                            {isSubmitting ? (
                                <>
                                    <Loader2 className="w-4 h-4 animate-spin" />
                                    <span>Iniciando Asistente...</span>
                                </>
                            ) : (
                                <span>Comenzar Ahora</span>
                            )}
                        </button>
                    </form>
                </div>
            </motion.div>
        </div>
    );
};

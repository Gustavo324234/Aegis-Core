import React, { useState, useEffect, useCallback } from 'react';
import { Plus, Trash2, RefreshCw, Key, Download, Upload } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { useTranslation } from '../../i18n';

interface KeyInfo {
    key_id: string;
    provider: string;
    api_key: string;
    api_url?: string;
    label?: string;
    is_active: boolean;
    rate_limited_until?: string;
}

interface EncryptedKeysBackup {
    salt: string;
    nonce: string;
    ciphertext: string;
}

const PROVIDERS = ['anthropic', 'openai', 'gemini', 'groq', 'deepseek', 'mistral', 'openrouter', 'qwen', 'ollama'];

const GlobalKeyManager: React.FC<{ tenantId: string; sessionKey: string }> = ({ tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const [keys, setKeys] = useState<KeyInfo[]>([]);
    const [isLoading, setIsLoading] = useState(false);
    const [showModal, setShowModal] = useState(false);
    const [provider, setProvider] = useState('anthropic');
    const [apiKey, setApiKey] = useState('');
    const [apiUrl, setApiUrl] = useState('');
    const [label, setLabel] = useState('');
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const fetchKeys = useCallback(async () => {
        setIsLoading(true);
        try {
            const res = await fetch(
                `/api/router/keys/global?tenant_id=${encodeURIComponent(tenantId)}`,
                { headers: { 'x-citadel-key': sessionKey } }
            );
            if (res.ok) {
                const data = await res.json();
                setKeys(data.keys || []);
            }
        } catch (err) {
            console.error('Failed to fetch global keys:', err);
        } finally {
            setIsLoading(false);
        }
    }, [tenantId, sessionKey]);

    useEffect(() => { fetchKeys(); }, [fetchKeys]);

    const [passwordPromptType, setPasswordPromptType] = useState<'export' | 'import' | null>(null);
    const [promptPassword, setPromptPassword] = useState('');
    const [importFileContent, setImportFileContent] = useState<EncryptedKeysBackup | null>(null);
    const [modalError, setModalError] = useState<string | null>(null);
    const [isProcessingBackup, setIsProcessingBackup] = useState(false);

    const initiateExport = () => {
        setPromptPassword('');
        setModalError(null);
        setPasswordPromptType('export');
    };

    const initiateImport = () => {
        const fileInput = document.getElementById('import-keys-file');
        if (fileInput) {
            fileInput.click();
        }
    };

    const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0];
        if (!file) return;

        const reader = new FileReader();
        reader.onload = (event) => {
            try {
                const json = JSON.parse(event.target?.result as string);
                if (!json.salt || !json.nonce || !json.ciphertext) {
                    alert('Archivo de backup inválido. Debe contener salt, nonce y ciphertext.');
                    return;
                }
                setImportFileContent(json);
                setPromptPassword('');
                setModalError(null);
                setPasswordPromptType('import');
            } catch (err) {
                alert('Error al leer el archivo: no es un JSON válido.');
            }
        };
        reader.readAsText(file);
        e.target.value = '';
    };

    const handleExportConfirm = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!promptPassword.trim()) {
            setModalError('Se requiere una contraseña para encriptar el backup.');
            return;
        }
        setIsProcessingBackup(true);
        setModalError(null);
        try {
            const res = await fetch('/api/router/keys/export', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey
                },
                body: JSON.stringify({ password: promptPassword })
            });

            if (res.ok) {
                const data = await res.json();
                const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = `aegis_keys_global_${Date.now()}.aegiskey`;
                a.click();
                URL.revokeObjectURL(url);
                setPasswordPromptType(null);
            } else {
                const d = await res.json();
                setModalError(d.detail || 'Falló la exportación de llaves.');
            }
        } catch (err) {
            setModalError(err instanceof Error ? err.message : 'Error desconocido.');
        } finally {
            setIsProcessingBackup(false);
        }
    };

    const handleImportConfirm = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!promptPassword.trim()) {
            setModalError('Ingrese la contraseña de descifrado.');
            return;
        }
        if (!importFileContent) {
            setModalError('No se encontró el contenido del archivo de importación.');
            return;
        }
        setIsProcessingBackup(true);
        setModalError(null);
        try {
            const res = await fetch('/api/router/keys/import', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-tenant': tenantId,
                    'x-citadel-key': sessionKey
                },
                body: JSON.stringify({
                    password: promptPassword,
                    salt: importFileContent.salt,
                    nonce: importFileContent.nonce,
                    ciphertext: importFileContent.ciphertext
                })
            });

            if (res.ok) {
                const data = await res.json();
                alert(`Importación exitosa: se restauraron ${data.count} llaves.`);
                setPasswordPromptType(null);
                await fetchKeys();
            } else {
                const d = await res.json();
                setModalError(d.detail || 'Contraseña incorrecta o backup corrupto.');
            }
        } catch (err) {
            setModalError(err instanceof Error ? err.message : 'Error desconocido.');
        } finally {
            setIsProcessingBackup(false);
        }
    };

    const handleToggle = async (keyId: string, newActive: boolean) => {
        setKeys(prev => prev.map(k =>
            k.key_id === keyId ? { ...k, is_active: newActive } : k
        ));
        try {
            const res = await fetch(`/api/router/keys/global/${encodeURIComponent(keyId)}?tenant_id=${encodeURIComponent(tenantId)}`, {
                method: 'PUT',
                headers: {
                    'Content-Type': 'application/json',
                    'x-citadel-key': sessionKey,
                },
                body: JSON.stringify({ is_active: newActive }),
            });
            if (!res.ok) {
                setKeys(prev => prev.map(k =>
                    k.key_id === keyId ? { ...k, is_active: !newActive } : k
                ));
            }
        } catch {
            setKeys(prev => prev.map(k =>
                k.key_id === keyId ? { ...k, is_active: !newActive } : k
            ));
        }
    };

    const handleAdd = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!apiKey.trim()) {
            setError(t('api_key_required_error'));
            return;
        }
        setIsSubmitting(true);
        setError(null);
        try {
            const res = await fetch('/api/router/keys/global', {
                method: 'POST',
                headers: { 
                    'Content-Type': 'application/json',
                    'x-citadel-key': sessionKey
                },
                body: JSON.stringify({
                    tenant_id: tenantId,
                    provider,
                    api_key: apiKey,
                    api_url: apiUrl || null,
                    label: label || null,
                }),
            });
            if (res.ok) {
                setShowModal(false);
                setApiKey('');
                setLabel('');
                setApiUrl('');
                await fetchKeys();
            } else {
                const d = await res.json();
                setError(d.detail || t('error_updating_password'));
            }
        } catch (err) {
            setError(err instanceof Error ? err.message : t('unknown_error'));
        } finally {
            setIsSubmitting(false);
        }
    };

    const handleDelete = async (keyId: string) => {
        try {
            await fetch(
                `/api/router/keys/global/${encodeURIComponent(keyId)}?tenant_id=${encodeURIComponent(tenantId)}`,
                { 
                    method: 'DELETE',
                    headers: { 'x-citadel-key': sessionKey }
                }
            );
            await fetchKeys();
        } catch (err) {
            console.error('Failed to delete key:', err);
        }
    };

    const getRateLimitText = (until: string | undefined): string => {
        if (!until) return '';
        const ms = new Date(until).getTime() - Date.now();
        if (ms <= 0) return '';
        const minutes = Math.ceil(ms / 60000);
        return t('rate_limited_until', { minutes: minutes.toString() });
    };

    return (
        <div className="glass p-6 rounded-2xl border border-white/10">
            <div className="flex items-center justify-between mb-6">
                <div className="flex items-center gap-3">
                    <Key className="w-5 h-5 text-aegis-cyan" />
                    <h3 className="text-sm font-mono font-bold tracking-widest uppercase text-white">{t('global_keys')}</h3>
                </div>
                <div className="flex gap-2">
                    <button
                        onClick={fetchKeys}
                        className="p-2 border border-white/10 rounded-lg hover:bg-white/5 transition-colors"
                        title="Actualizar llaves"
                    >
                        <RefreshCw className="w-4 h-4 text-white/40" />
                    </button>
                    <button
                        onClick={initiateExport}
                        className="flex items-center gap-2 px-3 py-2 border border-white/10 rounded-lg hover:bg-white/5 transition-colors text-xs font-mono text-white/60"
                        title="Exportar llaves (encriptadas)"
                    >
                        <Download className="w-4 h-4" /> Exportar
                    </button>
                    <button
                        onClick={initiateImport}
                        className="flex items-center gap-2 px-3 py-2 border border-white/10 rounded-lg hover:bg-white/5 transition-colors text-xs font-mono text-white/60"
                        title="Importar llaves (encriptadas)"
                    >
                        <Upload className="w-4 h-4" /> Importar
                    </button>
                    <button
                        onClick={() => setShowModal(true)}
                        className="flex items-center gap-2 px-3 py-2 bg-aegis-cyan/10 border border-aegis-cyan/30 rounded-lg hover:bg-aegis-cyan/20 transition-colors text-xs font-mono text-aegis-cyan"
                    >
                        <Plus className="w-4 h-4" /> {t('add_key')}
                    </button>
                    <input
                        type="file"
                        id="import-keys-file"
                        className="hidden"
                        accept=".aegiskey"
                        onChange={handleFileSelect}
                    />
                </div>
            </div>

            {isLoading ? (
                <div className="text-center py-8 text-white/30 text-xs font-mono">{t('syncing')}</div>
            ) : keys.length === 0 ? (
                <div className="text-center py-8 text-white/30 text-xs font-mono">{t('no_keys_configured')}</div>
            ) : (
                <div className="overflow-x-auto">
                    <table className="w-full text-xs font-mono">
                        <thead>
                            <tr className="text-white/30 uppercase tracking-widest border-b border-white/5">
                                <th className="text-left py-2 pr-4">{t('model').split(' ')[0]}</th>
                                <th className="text-left py-2 pr-4">{t('provider_selection').split(' ')[0]}</th>
                                <th className="text-left py-2 pr-4">{t('status')}</th>
                                <th className="text-right py-2">{t('actions')}</th>
                            </tr>
                        </thead>
                        <tbody>
                            {keys.map((k) => {
                                const rateLimitText = getRateLimitText(k.rate_limited_until);
                                return (
                                    <tr key={k.key_id} className="border-b border-white/5 hover:bg-white/2">
                                        <td className="py-3 pr-4 text-white/70">{k.label || '—'}</td>
                                        <td className="py-3 pr-4 text-aegis-cyan">{k.provider}</td>
                                        <td className="py-3 pr-4">
                                            {rateLimitText ? (
                                                <span className="text-yellow-400">{rateLimitText}</span>
                                            ) : (
                                                <button
                                                    onClick={() => handleToggle(k.key_id, !k.is_active)}
                                                    className={`relative w-10 h-5 rounded-full transition-colors duration-300
                                                        ${k.is_active ? 'bg-green-500/40 border-green-500/50' : 'bg-white/10 border-white/20'}
                                                        border hover:opacity-80`}
                                                    title={k.is_active ? t('status_active') : t('status_inactive')}
                                                >
                                                    <div className={`absolute top-0.5 w-4 h-4 rounded-full transition-all duration-300
                                                        ${k.is_active ? 'left-5 bg-green-400' : 'left-0.5 bg-white/30'}`}
                                                    />
                                                </button>
                                            )}
                                        </td>
                                        <td className="py-3 text-right">
                                            <button
                                                onClick={() => handleDelete(k.key_id)}
                                                className="p-1.5 border border-red-500/20 rounded hover:bg-red-500/10 transition-colors"
                                            >
                                                <Trash2 className="w-3.5 h-3.5 text-red-400" />
                                            </button>
                                        </td>
                                    </tr>
                                );
                            })}
                        </tbody>
                    </table>
                </div>
            )}

            <AnimatePresence>
                {showModal && (
                    <motion.div
                        initial={{ opacity: 0, height: 0, marginTop: 0 }}
                        animate={{ opacity: 1, height: 'auto', marginTop: 24 }}
                        exit={{ opacity: 0, height: 0, marginTop: 0 }}
                        className="overflow-hidden"
                    >
                        <div className="w-full bg-white/5 border border-white/10 rounded-2xl p-6 shadow-2xl">
                            <h4 className="text-sm font-mono font-bold tracking-widest uppercase text-white mb-6">Agregar API Key Global</h4>
                            <form onSubmit={handleAdd} className="space-y-4">
                                <div>
                                    <label className="block text-xs font-mono text-white/40 uppercase tracking-widest mb-1">Provider</label>
                                    <select
                                        value={provider}
                                        onChange={(e) => setProvider(e.target.value)}
                                        className="w-full bg-black/40 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono text-white focus:border-aegis-cyan/50 outline-none"
                                    >
                                        {PROVIDERS.map((p) => (
                                            <option key={p} value={p}>{p}</option>
                                        ))}
                                    </select>
                                </div>
                                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                    <div>
                                        <label className="block text-xs font-mono text-white/40 uppercase tracking-widest mb-1">API Key *</label>
                                        <input
                                            type="password"
                                            value={apiKey}
                                            onChange={(e) => setApiKey(e.target.value)}
                                            placeholder="sk-..."
                                            className="w-full bg-black/40 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono text-white focus:border-aegis-cyan/50 outline-none"
                                            required
                                        />
                                    </div>
                                    <div>
                                        <label className="block text-xs font-mono text-white/40 uppercase tracking-widest mb-1">{t('override_api_url')} ({t('optional')})</label>
                                        <input
                                            type="url"
                                            value={apiUrl}
                                            onChange={(e) => setApiUrl(e.target.value)}
                                            placeholder="https://..."
                                            className="w-full bg-black/40 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono text-white focus:border-aegis-cyan/50 outline-none"
                                        />
                                    </div>
                                </div>
                                <div>
                                    <label className="block text-xs font-mono text-white/40 uppercase tracking-widest mb-1">{t('model').split(' ')[0]} ({t('optional')})</label>
                                    <input
                                        type="text"
                                        value={label}
                                        onChange={(e) => setLabel(e.target.value)}
                                        placeholder="Mi key de produccion"
                                        className="w-full bg-black/40 border border-white/10 rounded-lg px-3 py-2 text-sm font-mono text-white focus:border-aegis-cyan/50 outline-none"
                                    />
                                </div>
                                {error && <p className="text-red-400 text-xs font-mono">{error}</p>}
                                <div className="flex gap-3 pt-2">
                                    <button
                                        type="button"
                                        onClick={() => setShowModal(false)}
                                        className="flex-1 px-4 py-2 border border-white/10 rounded-lg text-xs font-mono text-white/40 hover:bg-white/5 transition-colors uppercase tracking-widest"
                                    >
                                        {t('cancel')}
                                    </button>
                                    <button
                                        type="submit"
                                        disabled={isSubmitting}
                                        className="flex-1 px-4 py-2 bg-aegis-cyan/20 border border-aegis-cyan/30 rounded-lg text-xs font-mono text-aegis-cyan hover:bg-aegis-cyan/30 transition-colors disabled:opacity-50 uppercase tracking-widest font-bold"
                                    >
                                        {isSubmitting ? t('saving') : t('save')}
                                    </button>
                                </div>
                            </form>
                        </div>
                    </motion.div>
                )}
            </AnimatePresence>

            <AnimatePresence>
                {passwordPromptType && (
                    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 p-4">
                        <motion.div
                            initial={{ scale: 0.95, opacity: 0 }}
                            animate={{ scale: 1, opacity: 1 }}
                            exit={{ scale: 0.95, opacity: 0 }}
                            className="bg-zinc-900 border border-white/10 rounded-2xl p-6 max-w-md w-full shadow-2xl font-mono text-xs"
                        >
                            <h4 className="text-sm font-bold tracking-widest uppercase text-white mb-4">
                                {passwordPromptType === 'export' ? 'Exportar Llaves' : 'Importar Llaves'}
                            </h4>
                            <p className="text-white/60 mb-4">
                                {passwordPromptType === 'export'
                                    ? 'Establezca una contraseña para proteger y cifrar el archivo de backup (.aegiskey).'
                                    : 'Ingrese la contraseña para descifrar y restaurar las llaves de este backup.'}
                            </p>
                            <form onSubmit={passwordPromptType === 'export' ? handleExportConfirm : handleImportConfirm} className="space-y-4">
                                <div>
                                    <label className="block text-white/40 uppercase tracking-widest mb-1 font-bold">Contraseña</label>
                                    <input
                                        type="password"
                                        value={promptPassword}
                                        onChange={(e) => setPromptPassword(e.target.value)}
                                        placeholder="••••••••••••"
                                        className="w-full bg-black/40 border border-white/10 rounded-lg px-3 py-2 text-white focus:border-aegis-cyan/50 outline-none"
                                        required
                                        autoFocus
                                    />
                                </div>
                                {modalError && <p className="text-red-400 font-bold">{modalError}</p>}
                                <div className="flex gap-3 pt-2">
                                    <button
                                        type="button"
                                        onClick={() => setPasswordPromptType(null)}
                                        className="flex-1 px-4 py-2 border border-white/10 rounded-lg text-white/40 hover:bg-white/5 transition-colors uppercase tracking-widest"
                                    >
                                        {t('cancel')}
                                    </button>
                                    <button
                                        type="submit"
                                        disabled={isProcessingBackup}
                                        className="flex-1 px-4 py-2 bg-aegis-cyan/20 border border-aegis-cyan/30 rounded-lg text-aegis-cyan hover:bg-aegis-cyan/30 transition-colors disabled:opacity-50 uppercase tracking-widest font-bold"
                                    >
                                        {isProcessingBackup ? 'Procesando...' : 'Confirmar'}
                                    </button>
                                </div>
                            </form>
                        </motion.div>
                    </div>
                )}
            </AnimatePresence>
        </div>
    );
};

export default GlobalKeyManager;

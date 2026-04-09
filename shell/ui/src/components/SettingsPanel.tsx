import React, { useState } from 'react';
import { motion } from 'framer-motion';
import { Settings, X, LogOut, Save } from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';
import { useTranslation } from '../i18n';

interface SettingsPanelProps {
    onClose: () => void;
}

const SettingsPanel: React.FC<SettingsPanelProps> = ({ onClose }) => {
    const { t } = useTranslation();
    const { configureEngine, logout, tenantId } = useAegisStore();
    const [apiUrl, setApiUrl] = useState('');
    const [model, setModel] = useState('');
    const [apiKey, setApiKey] = useState('');
    const [isSaving, setIsSaving] = useState(false);
    const [message, setMessage] = useState<{type: 'success'|'error', text: string} | null>(null);

    const handleSave = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!apiUrl || !model || !apiKey) {
            setMessage({ type: 'error', text: t('all_fields_required') });
            return;
        }
        setIsSaving(true);
        setMessage(null);
        const success = await configureEngine(apiUrl, model, apiKey);
        setIsSaving(false);
        if (success) {
            setMessage({ type: 'success', text: t('engine_reconfigured_success') });
            setTimeout(() => onClose(), 1500);
        } else {
            setMessage({ type: 'error', text: t('failed_configure_engine') });
        }
    };

    return (
        <motion.div 
            initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
            className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm"
        >
            <motion.div 
                initial={{ scale: 0.95, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} exit={{ scale: 0.95, opacity: 0 }}
                className="w-full max-w-md bg-black border border-white/20 rounded-2xl p-6 shadow-[0_0_50px_rgba(0,0,0,0.8)] relative"
            >
                <button onClick={onClose} className="absolute top-4 right-4 p-2 text-white/50 hover:text-white rounded-lg hover:bg-white/10 transition-colors">
                    <X className="w-5 h-5" />
                </button>

                <div className="flex items-center gap-3 mb-6">
                    <Settings className="w-6 h-6 text-aegis-cyan" />
                    <h2 className="text-xl font-bold tracking-widest uppercase text-white">{t('settings_panel')}</h2>
                </div>

                <div className="mb-6 p-4 rounded-xl bg-white/5 border border-white/10">
                    <p className="text-[10px] uppercase tracking-widest text-white/40 mb-1">{t('active_tenant_id')}</p>
                    <p className="font-mono text-sm text-aegis-cyan">{tenantId}</p>
                </div>

                <form onSubmit={handleSave} className="space-y-4">
                    <div>
                        <label className="text-[10px] font-mono uppercase tracking-widest text-aegis-cyan mb-1 flex justify-between">
                            <span>{t('override_api_url')}</span>
                        </label>
                        <input
                            type="url"
                            value={apiUrl}
                            onChange={(e) => setApiUrl(e.target.value)}
                            className="w-full bg-black border border-white/10 rounded-lg py-2 px-3 text-sm focus:outline-none focus:border-aegis-cyan font-mono"
                            placeholder="New API URL..."
                        />
                    </div>
                    <div>
                        <label className="text-[10px] font-mono uppercase tracking-widest text-aegis-cyan mb-1 flex justify-between">
                            <span>{t('override_model_name')}</span>
                        </label>
                        <input
                            type="text"
                            value={model}
                            onChange={(e) => setModel(e.target.value)}
                            className="w-full bg-black border border-white/10 rounded-lg py-2 px-3 text-sm focus:outline-none focus:border-aegis-cyan font-mono"
                            placeholder="New Model..."
                        />
                    </div>
                    <div>
                        <label className="text-[10px] font-mono uppercase tracking-widest text-aegis-cyan mb-1 flex justify-between">
                            <span>{t('override_api_key')}</span>
                            <span className="text-white/30 lowercase">{t('will_overwrite')}</span>
                        </label>
                        <input
                            type="password"
                            value={apiKey}
                            onChange={(e) => setApiKey(e.target.value)}
                            className="w-full bg-black border border-white/10 rounded-lg py-2 px-3 text-sm focus:outline-none focus:border-aegis-cyan font-mono"
                            placeholder="New API Key..."
                        />
                    </div>

                    {message && (
                        <motion.p initial={{ opacity: 0 }} animate={{ opacity: 1 }} className={`text-xs font-mono mt-2 ${message.type === 'success' ? 'text-green-500' : 'text-red-500'}`}>
                            {message.text}
                        </motion.p>
                    )}

                    <div className="flex gap-3 mt-6 pt-4 border-t border-white/10">
                        <button type="button" onClick={logout} className="flex-1 flex items-center justify-center gap-2 bg-red-500/10 text-red-500 hover:bg-red-500/20 border border-red-500/30 py-3 rounded-lg text-xs font-bold uppercase tracking-widest transition-colors">
                            <LogOut className="w-4 h-4" /> {t('disconnect')}
                        </button>
                        <button type="submit" disabled={isSaving} className="flex-1 flex items-center justify-center gap-2 bg-aegis-cyan text-black hover:bg-aegis-cyan/80 py-3 rounded-lg text-xs font-bold uppercase tracking-widest transition-colors disabled:opacity-50">
                            {isSaving ? t('syncing') : t('save_config')} <Save className="w-4 h-4" />
                        </button>
                    </div>
                </form>
            </motion.div>
        </motion.div>
    );
};

export default SettingsPanel;

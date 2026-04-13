import React, { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Shield, Plus, Key, Terminal, LogOut, Check, Users, Activity, Trash2, RefreshCw, Cpu, HardDrive, Clock, Server, Mic } from 'lucide-react';
import { useAegisStore } from '../store/useAegisStore';
import { useTranslation } from '../i18n';

// Aegis Shell UI Core Components
import ProvidersTab from './ProvidersTab';
import SirenConfigTab from './SirenConfigTab';
import UserPasswordChange from './UserPasswordChange';

interface NewTenant {
    tenant_id: string;
    temporary_passphrase: string;
    network_port: number;
}

type TabId = 'users' | 'system' | 'providers' | 'siren';

const UsersTab: React.FC<{ tenantId: string | null; sessionKey: string | null }> = ({ tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const { tenants, fetchTenants, createTenant, deleteTenant, resetPassword, isFetchingTenants, lastTenantsUpdate, tenantsError } = useAegisStore();
    const [newUsername, setNewUsername] = useState('');
    const [isCreating, setIsCreating] = useState(false);
    const [createdTenant, setCreatedTenant] = useState<NewTenant | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null);
    const [isDeleting, setIsDeleting] = useState(false);
    const [resetResult, setResetResult] = useState<{ tenant_id: string, key: string } | null>(null);
    const [copied, setCopied] = useState(false);

    useEffect(() => {
        if (tenantId && sessionKey) fetchTenants();
    }, [tenantId, sessionKey, fetchTenants]);

    const handleCreateTenant = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!newUsername.trim()) return;
        setIsCreating(true);
        setError(null);
        setCreatedTenant(null);
        const result = await createTenant(newUsername);
        if (result.success) {
            setCreatedTenant({ tenant_id: newUsername, temporary_passphrase: result.temporary_passphrase || 'Check Logs', network_port: 0 });
            setNewUsername('');
        } else {
            setError(result.message || t('initialization_error'));
        }
        setIsCreating(false);
    };

    const handleDeleteTenant = async (targetTenantId: string) => {
        if (!tenantId || !sessionKey) return;
        setIsDeleting(true);
        const success = await deleteTenant(targetTenantId);
        if (success) setDeleteConfirm(null);
        else setError(t('initialization_error'));
        setIsDeleting(false);
    };

    const handleResetPassword = async (targetTenantId: string) => {
        if (!tenantId || !sessionKey) return;
        const newPass = Math.random().toString(36).slice(-12);
        const success = await resetPassword(targetTenantId, newPass);
        if (success) {
            setResetResult({ tenant_id: targetTenantId, key: newPass });
            setCopied(false);
        } else {
            setError(t('initialization_error'));
        }
    };

    return (
        <div className="space-y-8">
            <div className="glass p-6 rounded-2xl border border-white/10 shadow-2xl">
                <div className="flex items-center justify-between mb-6">
                    <div className="flex items-center gap-3">
                        <Users className="w-5 h-5 text-aegis-cyan" />
                        <h2 className="text-lg font-bold tracking-widest uppercase">{t('registered_enclaves')}</h2>
                    </div>
                    <div className="flex items-center gap-3">
                        {lastTenantsUpdate && (
                            <span className="text-[9px] font-mono text-white/20 uppercase tracking-widest hidden sm:inline">
                                Last Sync: {lastTenantsUpdate}
                            </span>
                        )}
                        <button 
                            onClick={fetchTenants} 
                            disabled={isFetchingTenants}
                            className="flex items-center gap-2 px-3 py-1.5 border border-white/10 rounded-lg hover:bg-white/5 disabled:opacity-50 transition-colors text-[10px] font-mono uppercase"
                        >
                            <RefreshCw className={`w-3 h-3 ${isFetchingTenants ? 'animate-spin' : ''}`} />
                            {isFetchingTenants ? t('syncing') : t('sync_now')}
                        </button>
                    </div>
                </div>

                {tenantsError && (
                    <div className="mb-6 p-4 bg-red-500/10 border border-red-500/20 rounded-xl flex items-center gap-3">
                        <Terminal className="w-4 h-4 text-red-500" />
                        <span className="text-[10px] font-mono text-red-400 uppercase tracking-widest">{tenantsError}</span>
                    </div>
                )}

                {tenants.length === 0 ? (
                    <div className="text-center py-8">
                        <p className="text-xs font-mono text-white/30 uppercase tracking-widest">{t('no_enclaves_registered')}</p>
                    </div>
                ) : (
                    <div className="overflow-x-auto">
                        <table className="w-full text-sm font-mono">
                            <thead>
                                <tr className="border-b border-white/10">
                                    <th className="text-left py-3 px-4 text-[10px] text-white/40 uppercase tracking-widest">Tenant ID</th>
                                    <th className="text-left py-3 px-4 text-[10px] text-white/40 uppercase tracking-widest">{t('status')}</th>
                                    <th className="text-right py-3 px-4 text-[10px] text-white/40 uppercase tracking-widest">{t('actions')}</th>
                                </tr>
                            </thead>
                            <tbody>
                                {tenants.map((tid) => (
                                    <tr key={tid} className="border-b border-white/5 hover:bg-white/5 transition-colors">
                                        <td className="py-3 px-4 text-white font-bold">{tid}</td>
                                        <td className="py-3 px-4">
                                            <span className="inline-flex items-center gap-1.5 px-2 py-0.5 bg-green-500/10 border border-green-500/30 rounded text-[10px] text-green-400 uppercase tracking-wider">
                                                <span className="w-1.5 h-1.5 bg-green-500 rounded-full" /> {t('active')}
                                            </span>
                                        </td>
                                        <td className="py-3 px-4 text-right">
                                            <div className="flex items-center justify-end gap-2">
                                                <button onClick={() => handleResetPassword(tid)} className="p-1.5 rounded hover:bg-yellow-500/10 transition-colors group" title="Reset Password">
                                                    <Key className="w-3.5 h-3.5 text-white/30 group-hover:text-yellow-500 transition-colors" />
                                                </button>
                                                {deleteConfirm === tid ? (
                                                    <div className="flex items-center gap-1">
                                                        <button onClick={() => handleDeleteTenant(tid)} disabled={isDeleting} className="px-2 py-1 bg-red-500/20 border border-red-500/50 rounded text-[9px] text-red-400 font-mono uppercase hover:bg-red-500/30 transition-colors">
                                                            {isDeleting ? '...' : t('confirm')}
                                                        </button>
                                                        <button onClick={() => setDeleteConfirm(null)} className="px-2 py-1 border border-white/10 rounded text-[9px] text-white/40 font-mono uppercase hover:bg-white/5 transition-colors">
                                                            {t('cancel')}
                                                        </button>
                                                    </div>
                                                ) : (
                                                    <button onClick={() => setDeleteConfirm(tid)} className="p-1.5 rounded hover:bg-red-500/10 transition-colors group" title={t('delete')}>
                                                        <Trash2 className="w-3.5 h-3.5 text-white/30 group-hover:text-red-500 transition-colors" />
                                                    </button>
                                                )}
                                            </div>
                                        </td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                )}
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-8">
                <div className="glass p-6 rounded-2xl border border-white/10 shadow-2xl relative overflow-hidden">
                    <div className="flex items-center gap-3 mb-6">
                        <Plus className="w-5 h-5 text-aegis-cyan" />
                        <h2 className="text-lg font-bold tracking-widest uppercase">Forge New Tenant</h2>
                    </div>
                    <form onSubmit={handleCreateTenant} className="space-y-6">
                        <div className="space-y-2">
                            <label className="text-[10px] font-mono text-white/40 uppercase ml-1 tracking-widest">{t('id_name_label')}</label>
                            <input type="text" value={newUsername} onChange={(e) => setNewUsername(e.target.value)} placeholder="Operador_Alfa" className="w-full bg-black/40 border border-white/10 rounded-lg py-3 px-4 text-sm font-mono focus:border-aegis-cyan/50 focus:ring-0 transition-all placeholder:text-white/10" required />
                        </div>
                        {error && (
                            <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="bg-red-500/10 border border-red-500/30 p-3 rounded-lg flex items-center gap-3">
                                <Terminal className="w-4 h-4 text-red-500 flex-shrink-0" />
                                <span className="text-[10px] font-mono text-red-400 leading-tight">{error}</span>
                            </motion.div>
                        )}
                        <button type="submit" disabled={isCreating} className={`w-full group relative overflow-hidden rounded-lg py-4 transition-all duration-500 ${isCreating ? "bg-aegis-cyan/20 cursor-wait" : "bg-aegis-cyan/10 hover:bg-aegis-cyan/20 border border-aegis-cyan/30"}`}>
                            <div className="relative z-10 flex items-center justify-center gap-3">
                                <span className="text-xs font-mono font-bold tracking-[0.3em] uppercase text-aegis-cyan">{isCreating ? t('deriving') : t('create_enclave')}</span>
                            </div>
                        </button>
                    </form>
                </div>

                <AnimatePresence>
                    {createdTenant ? (
                        <motion.div initial={{ opacity: 0, x: 20 }} animate={{ opacity: 1, x: 0 }} className="glass p-6 rounded-2xl border border-green-500/30 bg-green-500/5 shadow-2xl">
                            <div className="flex items-center gap-3 mb-6">
                                <Check className="w-5 h-5 text-green-500" />
                                <h2 className="text-lg font-bold tracking-widest uppercase text-green-400">{t('enclave_operational')}</h2>
                            </div>
                            <div className="space-y-4 font-mono text-sm">
                                <div className="bg-black/50 p-4 rounded-lg border border-white/5">
                                    <p className="text-white/40 text-[10px] uppercase mb-1">Host</p>
                                    <p className="text-white font-bold">{window.location.hostname}</p>
                                </div>
                                <div className="bg-black/50 p-4 rounded-lg border border-aegis-cyan/20">
                                    <p className="text-aegis-cyan/60 text-[10px] uppercase mb-1">{t('access_url')}</p>
                                    <p className="text-aegis-cyan font-bold tracking-widest truncate">{window.location.origin}</p>
                                </div>
                                <div className="bg-black/50 p-4 rounded-lg border border-white/5">
                                    <p className="text-white/40 text-[10px] uppercase mb-1">Tenant</p>
                                    <p className="text-white font-bold">{createdTenant.tenant_id}</p>
                                </div>
                                <div className="bg-black/50 p-4 rounded-lg border border-green-500/20">
                                    <div className="flex justify-between items-center mb-1">
                                        <p className="text-green-500/60 text-[10px] uppercase">{t('temp_password')}</p>
                                        <Key className="w-3 h-3 text-green-500/60" />
                                    </div>
                                    <p className="text-green-400 font-bold tracking-widest">{createdTenant.temporary_passphrase}</p>
                                    <p className="text-[9px] text-green-500/40 mt-2">{t('temp_password_warning')}</p>
                                </div>
                            </div>
                            <p className="text-[10px] font-mono text-white/30 text-center mt-6">{t('credential_safety_notice')}</p>
                        </motion.div>
                    ) : (
                        <div className="flex items-center justify-center p-6 border border-dashed border-white/10 rounded-2xl bg-white/5">
                            <p className="text-xs font-mono text-white/30 uppercase tracking-widest text-center">{t('waiting_forge_commands')}</p>
                        </div>
                    )}
                </AnimatePresence>
            </div>

            <AnimatePresence>
                {resetResult && (
                    <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm">
                        <motion.div initial={{ scale: 0.95 }} animate={{ scale: 1 }} exit={{ scale: 0.95 }} className="glass p-8 rounded-2xl border border-yellow-500/30 bg-black max-w-md w-full shadow-2xl relative">
                            <div className="flex items-center gap-3 mb-6">
                                <Key className="w-6 h-6 text-yellow-500" />
                                <h2 className="text-xl font-bold tracking-widest uppercase text-yellow-400">{t('change_password')}</h2>
                            </div>
                            <p className="text-sm font-mono text-white/70 mb-4 uppercase tracking-wider">
                                {t('status')}: <span className="font-bold text-white">{resetResult.tenant_id}</span>
                            </p>
                            <div className="bg-black/50 p-4 rounded-lg border border-yellow-500/20 mb-6 relative">
                                <p className="text-white/40 text-[10px] uppercase mb-2">{t('temp_password')}</p>
                                <div className="flex items-center justify-between gap-4">
                                    <p className="text-yellow-400 font-bold tracking-widest text-xl truncate selection:bg-yellow-500/30">{resetResult.key}</p>
                                    <button 
                                        onClick={() => { 
                                            navigator.clipboard.writeText(resetResult.key); 
                                            setCopied(true);
                                            setTimeout(() => setCopied(false), 2000);
                                        }} 
                                        className="px-3 py-2 bg-yellow-500/10 hover:bg-yellow-500/20 border border-yellow-500/30 rounded text-yellow-500 text-[10px] uppercase font-bold transition-colors shadow-none outline-none"
                                    >
                                        {copied ? 'COPIED' : 'COPY'}
                                    </button>
                                </div>
                            </div>
                            <button onClick={() => setResetResult(null)} className="w-full py-3 bg-white/5 hover:bg-white/10 text-white font-mono text-sm uppercase tracking-widest rounded-lg transition-colors">
                                {t('confirm')}
                            </button>
                        </motion.div>
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
};

interface SystemMetrics {
    cpu_load: number;
    vram_allocated_mb: number;
    vram_total_mb: number;
    total_processes: number;
    active_workers: number;
    uptime?: string;
    hw_profile?: string;
    loaded_models?: string[];
}

const SystemTab: React.FC<{ tenantId: string | null; sessionKey: string | null }> = ({ tenantId, sessionKey }) => {
    const { t } = useTranslation();
    const [metrics, setMetrics] = useState<SystemMetrics | null>(null);
    const [isLoading, setIsLoading] = useState(true);

    useEffect(() => {
        if (!tenantId || !sessionKey) return;
        const poll = async () => {
            try {
                const response = await fetch(`/api/status?tenant_id=${encodeURIComponent(tenantId)}`, {
                    headers: {
                        'x-citadel-key': sessionKey
                    }
                });
                if (response.ok) setMetrics(await response.json());
            } catch (err) {
                console.error('Telemetry poll error:', err);
            } finally {
                setIsLoading(false);
            }
        };
        poll();
        const interval = setInterval(poll, 5000);
        return () => clearInterval(interval);
    }, [tenantId, sessionKey]);

    const cpuPercent = metrics?.cpu_load ?? 0;
    const vramUsed = metrics?.vram_allocated_mb ?? 0;
    const vramTotal = metrics?.vram_total_mb ?? 1;
    const vramPercent = vramTotal > 0 ? (vramUsed / vramTotal) * 100 : 0;

    return (
        <div className="space-y-6">
            {isLoading ? (
                <div className="flex items-center justify-center py-16">
                    <Activity className="w-6 h-6 text-aegis-cyan animate-pulse" />
                    <span className="text-xs font-mono text-white/40 uppercase ml-3 tracking-widest">{t('connecting_telemetry')}</span>
                </div>
            ) : (
                <>
                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                        <div className="glass p-6 rounded-2xl border border-white/10">
                            <div className="flex items-center gap-3 mb-4">
                                <Cpu className="w-5 h-5 text-aegis-cyan" />
                                <span className="text-[10px] font-mono text-white/40 uppercase tracking-widest">CPU Load</span>
                            </div>
                            <p className={`text-3xl font-bold font-mono ${cpuPercent > 80 ? 'text-red-400' : cpuPercent > 50 ? 'text-yellow-400' : 'text-green-400'}`}>{cpuPercent.toFixed(1)}%</p>
                            <div className="mt-3 h-2 bg-white/5 rounded-full overflow-hidden">
                                <motion.div className={`h-full rounded-full ${cpuPercent > 80 ? 'bg-red-500' : cpuPercent > 50 ? 'bg-yellow-500' : 'bg-green-500'}`} initial={{ width: 0 }} animate={{ width: `${Math.min(cpuPercent, 100)}%` }} transition={{ duration: 0.5 }} />
                            </div>
                        </div>
                        <div className="glass p-6 rounded-2xl border border-white/10">
                            <div className="flex items-center gap-3 mb-4">
                                <HardDrive className="w-5 h-5 text-aegis-purple" />
                                <span className="text-[10px] font-mono text-white/40 uppercase tracking-widest">VRAM</span>
                            </div>
                            <p className="text-3xl font-bold font-mono text-aegis-purple">{vramUsed}<span className="text-lg text-white/30">/{vramTotal} MB</span></p>
                            <div className="mt-3 h-2 bg-white/5 rounded-full overflow-hidden">
                                <motion.div className="h-full rounded-full bg-aegis-purple" initial={{ width: 0 }} animate={{ width: `${Math.min(vramPercent, 100)}%` }} transition={{ duration: 0.5 }} />
                            </div>
                        </div>
                        <div className="glass p-6 rounded-2xl border border-white/10">
                            <div className="flex items-center gap-3 mb-4">
                                <Activity className="w-5 h-5 text-green-400" />
                                <span className="text-[10px] font-mono text-white/40 uppercase tracking-widest">Procesos</span>
                            </div>
                            <p className="text-3xl font-bold font-mono text-white">{metrics?.total_processes ?? 0}</p>
                            <p className="text-[10px] font-mono text-white/30 mt-2">Workers: {metrics?.active_workers ?? 0}</p>
                        </div>
                    </div>
                    
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                        <div className="glass p-6 rounded-2xl border border-white/10">
                            <div className="flex items-center gap-3 mb-4">
                                <Clock className="w-5 h-5 text-amber-500" />
                                <span className="text-[10px] font-mono text-white/40 uppercase tracking-widest">{t('system_uptime')}</span>
                            </div>
                            <p className="text-3xl font-bold font-mono text-amber-400">{metrics?.uptime ?? "00:00:00"}</p>
                            <p className="text-[10px] font-mono text-white/30 mt-2">{t('uptime_description')}</p>
                        </div>
                        <div className="glass p-6 rounded-2xl border border-white/10">
                            <div className="flex items-center gap-3 mb-4">
                                <Server className="w-5 h-5 text-blue-500" />
                                <span className="text-[10px] font-mono text-white/40 uppercase tracking-widest">{t('host_environment')}</span>
                            </div>
                            <div className="space-y-2">
                                <div className="flex justify-between">
                                    <span className="text-[10px] font-mono text-white/40 uppercase">Hardware Profile</span>
                                    <span className="text-xs font-mono text-blue-400 font-bold uppercase">{metrics?.hw_profile ?? "Local"}</span>
                                </div>
                                <div className="flex justify-between">
                                    <span className="text-[10px] font-mono text-white/40 uppercase">Aegis OS</span>
                                    <span className="text-xs font-mono text-white/80 font-bold">Native Runtime</span>
                                </div>
                                <div className="flex justify-between">
                                    <span className="text-[10px] font-mono text-white/40 uppercase">Mode</span>
                                    <span className="text-xs font-mono text-green-400 font-bold uppercase">Dev</span>
                                </div>
                            </div>
                        </div>
                    </div>

                    <div className="mt-8 pt-8 border-t border-white/10">
                        <div className="max-w-md mx-auto">
                            <UserPasswordChange onComplete={() => alert(t('admin_password_changed_success'))} />
                        </div>
                    </div>

                    <div className="flex items-center gap-2 justify-center text-[9px] font-mono text-white/20 uppercase tracking-[0.2em] mt-8">
                        <span className="w-1.5 h-1.5 bg-green-500 rounded-full animate-pulse" />
                        Polling each 5s — Real-Time Telemetry Active
                    </div>
                </>
            )}
        </div>
    );
};

const TABS = (t: (key: string) => string): { id: TabId; label: string; icon: React.ReactNode }[] => [
    { id: 'users', label: t('users'), icon: <Users className="w-4 h-4" /> },
    { id: 'system', label: t('system'), icon: <Activity className="w-4 h-4" /> },
    { id: 'providers', label: 'IA Tools', icon: <Cpu className="w-4 h-4" /> },
    { id: 'siren', label: t('voice_audio'), icon: <Mic className="w-4 h-4" /> },
];

const AdminDashboard: React.FC = () => {
    const { t } = useTranslation();
    const { tenantId, sessionKey, logout, adminActiveTab, setAdminActiveTab } = useAegisStore();
    const tabsList = TABS(t);

    return (
        <div className="min-h-screen bg-black text-white p-8 overflow-y-auto relative">
            <div className="absolute inset-0 overflow-hidden pointer-events-none">
                <div className="absolute top-0 right-0 w-[500px] h-[500px] bg-red-500/5 rounded-full blur-[120px]" />
                <div className="absolute bottom-0 left-0 w-[400px] h-[400px] bg-aegis-cyan/5 rounded-full blur-[100px]" />
            </div>
            <div className="max-w-6xl mx-auto z-10 relative">
                <header className="flex justify-between items-center mb-8 border-b border-red-500/20 pb-6">
                    <div className="flex items-center gap-4">
                        <div className="p-3 rounded-xl bg-red-500/10 border border-red-500/30">
                            <Shield className="w-8 h-8 text-red-500" />
                        </div>
                        <div>
                            <h1 className="text-2xl font-bold tracking-[0.2em] uppercase text-white">Master <span className="text-red-500">{t('dashboard')}</span></h1>
                            <p className="text-sm font-mono text-red-500/60 uppercase tracking-widest">Citadel Authorization Level: MAXIMUM</p>
                        </div>
                    </div>
                    <button onClick={logout} className="flex items-center gap-2 px-4 py-2 border border-white/10 rounded-lg hover:bg-white/5 transition-colors font-mono text-xs uppercase text-red-400 hover:text-red-300">
                        <LogOut className="w-4 h-4" /> Disconnect
                    </button>
                </header>
                <div className="flex flex-wrap gap-1 mb-8 bg-white/5 p-1 rounded-xl border border-white/10 w-fit">
                    {tabsList.map((tab) => (
                        <button key={tab.id} onClick={() => setAdminActiveTab(tab.id as TabId)} className={`flex items-center gap-2 px-5 py-2.5 rounded-lg transition-all duration-300 text-xs font-mono uppercase tracking-widest ${adminActiveTab === tab.id ? 'bg-aegis-cyan/20 text-aegis-cyan border border-aegis-cyan/30' : 'text-white/40 hover:text-white/70 hover:bg-white/5 border border-transparent'}`}>
                            {tab.icon}{tab.label}
                        </button>
                    ))}
                </div>
                <AnimatePresence mode="wait">
                    <motion.div key={adminActiveTab} initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -10 }} transition={{ duration: 0.2 }}>
                        {adminActiveTab === 'users' && <UsersTab tenantId={tenantId} sessionKey={sessionKey} />}
                        {adminActiveTab === 'system' && <SystemTab tenantId={tenantId} sessionKey={sessionKey} />}
                        {adminActiveTab === 'providers' && <ProvidersTab tenantId={tenantId} sessionKey={sessionKey} />}
                        {adminActiveTab === 'siren' && <SirenConfigTab />}
                    </motion.div>
                </AnimatePresence>
            </div>
        </div>
    );
};

export default AdminDashboard;

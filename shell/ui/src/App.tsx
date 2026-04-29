import { useEffect, useState, Component, type ReactNode } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Shield } from 'lucide-react';
import { useAegisStore } from './store/useAegisStore';

import ChatTerminal from './components/ChatTerminal';
import LoginScreen from './components/LoginScreen';
import AdminDashboard from './components/AdminDashboard';
import ForcePasswordChangeScreen from './components/ForcePasswordChangeScreen';
import BootstrapSetup from './components/BootstrapSetup';
import EngineSetupWizard from './components/EngineSetupWizard';
import Dashboard from '@/components/Dashboard';


// SRE-FIX: Error boundary para atrapar crashes de React y mostrar pantalla de recuperación
// en lugar de pantalla negra. Limpia el localStorage y redirige al login.
class AegisErrorBoundary extends Component<{ children: ReactNode }, { hasError: boolean }> {
    constructor(props: { children: ReactNode }) {
        super(props);
        this.state = { hasError: false };
    }

    static getDerivedStateFromError() {
        return { hasError: true };
    }

    componentDidCatch(error: Error) {
        console.error('[Aegis Error Boundary]', error);
    }

    handleReload() {
        this.setState({ hasError: false });
    }

    handleReset() {
        try { localStorage.removeItem('aegis-storage'); } catch { /* ignore */ }
        window.location.reload();
    }

    render() {
        if (this.state.hasError) {
            return (
                <div className="min-h-screen bg-black flex items-center justify-center p-4">
                    <div className="glass p-8 rounded-2xl border border-red-500/30 text-center space-y-4 max-w-sm">
                        <Shield className="w-10 h-10 text-red-500 mx-auto" />
                        <h1 className="text-lg font-bold tracking-widest text-red-400 uppercase">View Error</h1>
                        <p className="text-[10px] font-mono text-white/40 uppercase tracking-widest leading-relaxed">
                            An error occurred loading this view. Your session is intact.
                        </p>
                        <div className="flex gap-3 justify-center pt-2">
                            <button
                                onClick={() => this.handleReload()}
                                className="px-4 py-2 border border-white/20 text-white/60 font-bold text-[10px] uppercase tracking-widest rounded-lg hover:bg-white/5 transition-colors"
                            >
                                Reload View
                            </button>
                            <button
                                onClick={() => this.handleReset()}
                                className="px-4 py-2 bg-red-500/20 border border-red-500/30 text-red-400 font-bold text-[10px] uppercase tracking-widest rounded-lg hover:bg-red-500/30 transition-colors"
                            >
                                Reset Session
                            </button>
                        </div>
                    </div>
                </div>
            );
        }
        return this.props.children;
    }
}

function App() {
    const {
        _hydrated, status, isAuthenticated, isAdmin, systemState,
        tenantId, sessionKey, connect, logout, fetchSystemState,
        isEngineConfigured, setEngineConfigured,
        needsPasswordReset, setNeedsPasswordReset,
        currentView, setCurrentView
    } = useAegisStore();
    
    const [setupToken, setSetupToken] = useState<string | null>(null);

    useEffect(() => {
        if (_hydrated) {
            fetchSystemState();
        }
    }, [_hydrated, fetchSystemState]);

    useEffect(() => {
        const params = new URLSearchParams(window.location.search);
        const token = params.get('setup_token');
        if (token) {
            setSetupToken(token);
            window.history.replaceState({}, document.title, window.location.pathname);
        }
    }, []);

    // Security: si el store dice autenticado pero sessionKey es null (tras F5), forzar logout
    useEffect(() => {
        if (_hydrated && isAuthenticated && !sessionKey) {
            logout();
        }
    }, [_hydrated, isAuthenticated, sessionKey, logout]);

    useEffect(() => {
        if (!_hydrated || !isAuthenticated || isAdmin || needsPasswordReset || isEngineConfigured) return;
        const checkGlobalEngine = async () => {
            try {
                const res = await fetch('/api/engine/status');
                if (res.ok) {
                    const data = await res.json() as { configured: boolean };
                    if (data.configured) setEngineConfigured(true);
                }
            } catch (err) {
                console.error('Failed to check global engine:', err);
            }
        };
        checkGlobalEngine();
    }, [_hydrated, isAuthenticated, isAdmin, needsPasswordReset, isEngineConfigured, setEngineConfigured]);

    useEffect(() => {
        if (_hydrated && isAuthenticated && !isAdmin && !needsPasswordReset && isEngineConfigured && tenantId && sessionKey && status === 'disconnected') {
            connect(tenantId, sessionKey);
        }
    }, [_hydrated, isAuthenticated, isAdmin, needsPasswordReset, isEngineConfigured, tenantId, sessionKey, status, connect]);

    // CORE-230: si sessionKey es null pero la vista es dashboard, redirigir a chat antes de montar Dashboard
    useEffect(() => {
        if (_hydrated && currentView === 'dashboard' && !sessionKey) {
            console.warn('[App] sessionKey null with dashboard view — redirecting to chat');
            setCurrentView('chat');
        }
    }, [_hydrated, currentView, sessionKey, setCurrentView]);

    return (
        <div className="bg-black min-h-screen text-white overflow-hidden">
            <AegisErrorBoundary>
                <AnimatePresence mode="wait">
                    {!_hydrated || systemState === 'UNKNOWN' ? (
                        <motion.div key="loading_state" className="flex items-center justify-center h-screen">
                            <Shield className="w-10 h-10 text-white/20 animate-pulse" />
                        </motion.div>
                    ) : setupToken ? (
                        <motion.div key="bootstrap_setup">
                            <BootstrapSetup 
                                token={setupToken} 
                                onComplete={() => {
                                    setSetupToken(null);
                                    fetchSystemState();
                                }} 
                            />
                        </motion.div>
                    ) : systemState === 'STATE_INITIALIZING' ? (
                        <motion.div key="admin_setup_lock" className="min-h-screen bg-black flex items-center justify-center p-4">
                            <div className="glass p-8 rounded-2xl border border-red-500/30 text-center space-y-4">
                                <h1 className="text-2xl font-bold tracking-widest text-red-500 uppercase">Ring 0 Identity Required</h1>
                                <p className="text-[10px] font-mono text-white/40 uppercase tracking-widest max-w-xs mx-auto">
                                    Secure Bootstrap Directive: Administrator identity not found. Verify your Setup Token for system initialization.
                                </p>
                                <div className="pt-4 flex justify-center">
                                    <Shield className="w-8 h-8 text-red-500/20 animate-pulse" />
                                </div>
                            </div>
                        </motion.div>
                    ) : !isAuthenticated ? (
                        <motion.div key="login" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0, scale: 0.95, filter: 'blur(10px)' }} transition={{ duration: 0.5 }} className="w-full">
                            <LoginScreen onNeedsPasswordReset={() => setNeedsPasswordReset(true)} />
                        </motion.div>
                    ) : needsPasswordReset ? (
                        <motion.div key="force_reset">
                            <ForcePasswordChangeScreen onPasswordChanged={() => setNeedsPasswordReset(false)} />
                        </motion.div>
                    ) : isAdmin ? (
                        <motion.div key="admin_dashboard" initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }}>
                            <AdminDashboard />
                        </motion.div>
                    ) : !isEngineConfigured ? (
                        <motion.div key="engine_setup">
                            <EngineSetupWizard />
                        </motion.div>
                    ) : status === 'connecting' ? (
                        <motion.div key="connecting_overlay" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0, scale: 1.1, filter: 'blur(10px)' }} transition={{ duration: 0.8 }} className="flex flex-col items-center justify-center min-h-screen p-4">
                            <div className="relative mb-8">
                                <motion.div animate={{ scale: [1, 1.2, 1], opacity: [0.3, 0.6, 0.3] }} transition={{ duration: 3, repeat: Infinity, ease: "easeInOut" }} className="absolute -inset-8 bg-aegis-cyan rounded-full blur-3xl" />
                                <Shield className="w-20 h-20 text-aegis-cyan relative z-10" />
                            </div>
                            <div className="text-center">
                                <h1 className="text-4xl font-bold tracking-[0.3em] mb-2 uppercase text-white">Aegis <span className="text-aegis-cyan">Shell</span></h1>
                                <p className="text-aegis-steel font-mono tracking-widest text-[10px] animate-pulse">ESTABLISHING RING 0 NEURAL LINK...</p>
                            </div>
                        </motion.div>
                    ) : (
                        <motion.div key="terminal" initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.5, ease: "easeOut" }} className="h-screen w-full">
                            {currentView === 'chat' ? <ChatTerminal /> : <Dashboard />}
                        </motion.div>
                    )}
                </AnimatePresence>
            </AegisErrorBoundary>
        </div>
    );
}

export default App;

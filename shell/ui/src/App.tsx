import { useEffect, useState, Component, Suspense, lazy, type ReactNode } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Shield } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { useAegisStore } from './store/useAegisStore';
import { ApiError, apiGet } from './utils/apiClient';

const ChatTerminal = lazy(() => import('./components/ChatTerminal'));
const LoginScreen = lazy(() => import('./components/LoginScreen'));
const AdminDashboard = lazy(() => import('./components/AdminDashboard'));
const ForcePasswordChangeScreen = lazy(() => import('./components/ForcePasswordChangeScreen'));
const BootstrapSetup = lazy(() => import('./components/BootstrapSetup'));
const EngineSetupWizard = lazy(() => import('./components/EngineSetupWizard'));
const PersonalQuickSetup = lazy(() =>
    import('./components/PersonalQuickSetup').then((m) => ({ default: m.PersonalQuickSetup })),
);
const Dashboard = lazy(() => import('@/components/Dashboard'));
const AgentInboxList = lazy(() =>
    import('./components/AgentInboxList').then((m) => ({ default: m.AgentInboxList })),
);
const AgentThread = lazy(() =>
    import('./components/AgentThread').then((m) => ({ default: m.AgentThread })),
);
const SupervisorQuestionModal = lazy(() =>
    import('./components/SupervisorQuestionModal').then((m) => ({ default: m.SupervisorQuestionModal })),
);

const ViewFallback = () => (
    <div className="flex items-center justify-center h-screen bg-black">
        <Shield className="w-8 h-8 text-aegis-cyan/40 animate-pulse" />
    </div>
);


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
    } = useAegisStore(
        useShallow((s) => ({
            _hydrated: s._hydrated,
            status: s.status,
            isAuthenticated: s.isAuthenticated,
            isAdmin: s.isAdmin,
            systemState: s.systemState,
            tenantId: s.tenantId,
            sessionKey: s.sessionKey,
            connect: s.connect,
            logout: s.logout,
            fetchSystemState: s.fetchSystemState,
            isEngineConfigured: s.isEngineConfigured,
            setEngineConfigured: s.setEngineConfigured,
            needsPasswordReset: s.needsPasswordReset,
            setNeedsPasswordReset: s.setNeedsPasswordReset,
            currentView: s.currentView,
            setCurrentView: s.setCurrentView,
        })),
    );
    
    const [setupToken, setSetupToken] = useState<string | null>(null);
    // CORE-232: pre-rellenar el campo tenant en LoginScreen tras logout forzado por sessionKey null
    const [prefillTenantId, setPrefillTenantId] = useState<string | null>(null);

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
    // CORE-232: guardar tenantId antes del logout para pre-rellenar el campo en LoginScreen
    useEffect(() => {
        if (_hydrated && isAuthenticated && !sessionKey) {
            if (tenantId) setPrefillTenantId(tenantId);
            logout();
        }
    }, [_hydrated, isAuthenticated, sessionKey, tenantId, logout]);

    useEffect(() => {
        if (!_hydrated || !isAuthenticated || isAdmin || needsPasswordReset || isEngineConfigured) return;
        const controller = new AbortController();
        const checkGlobalEngine = async () => {
            try {
                const data = await apiGet<{ configured: boolean }>('/api/engine/status', {
                    auth: tenantId && sessionKey ? { tenantId, sessionKey } : false,
                    signal: controller.signal,
                });
                if (data.configured) setEngineConfigured(true);
            } catch (err) {
                if (err instanceof ApiError && err.code === 'ABORTED') return;
                console.error('Failed to check global engine:', err);
            }
        };
        void checkGlobalEngine();
        return () => controller.abort();
    }, [_hydrated, isAuthenticated, isAdmin, needsPasswordReset, isEngineConfigured, tenantId, sessionKey, setEngineConfigured]);

    useEffect(() => {
        if (_hydrated && isAuthenticated && !isAdmin && !needsPasswordReset && isEngineConfigured && tenantId && sessionKey && status === 'disconnected') {
            connect(tenantId, sessionKey);
        }
    }, [_hydrated, isAuthenticated, isAdmin, needsPasswordReset, isEngineConfigured, tenantId, sessionKey, status, connect]);

    // CORE-230: si sessionKey es null pero la vista no es chat, redirigir a chat
    useEffect(() => {
        if (_hydrated && currentView !== 'chat' && !sessionKey) {
            console.warn('[App] sessionKey null with non-chat view — redirecting to chat');
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
                            <Suspense fallback={<ViewFallback />}>
                                <BootstrapSetup 
                                    token={setupToken} 
                                    onComplete={() => {
                                        setSetupToken(null);
                                        fetchSystemState();
                                    }} 
                                />
                            </Suspense>
                        </motion.div>
                    ) : systemState === 'STATE_INITIALIZING' ? (
                        <motion.div key="admin_setup_lock">
                            <Suspense fallback={<ViewFallback />}>
                                <PersonalQuickSetup setupToken={setupToken} />
                            </Suspense>
                        </motion.div>
                    ) : !isAuthenticated ? (
                        <motion.div key="login" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0, scale: 0.95, filter: 'blur(10px)' }} transition={{ duration: 0.5 }} className="w-full">
                            <Suspense fallback={<ViewFallback />}>
                                <LoginScreen onNeedsPasswordReset={() => setNeedsPasswordReset(true)} prefillTenantId={prefillTenantId} />
                            </Suspense>
                        </motion.div>
                    ) : needsPasswordReset ? (
                        <motion.div key="force_reset">
                            <Suspense fallback={<ViewFallback />}>
                                <ForcePasswordChangeScreen onPasswordChanged={() => setNeedsPasswordReset(false)} />
                            </Suspense>
                        </motion.div>
                    ) : isAdmin ? (
                        <motion.div key="admin_dashboard" initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }}>
                            <Suspense fallback={<ViewFallback />}>
                                <AdminDashboard />
                            </Suspense>
                        </motion.div>
                    ) : !isEngineConfigured ? (
                        <motion.div key="engine_setup">
                            <Suspense fallback={<ViewFallback />}>
                                <EngineSetupWizard />
                            </Suspense>
                        </motion.div>
                    ) : status === 'connecting' ? (
                        <motion.div key="connecting_overlay" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0, scale: 1.1, filter: 'blur(10px)' }} transition={{ duration: 0.8 }} className="flex flex-col items-center justify-center min-h-screen p-4">
                            <div className="relative mb-8">
                                <motion.div animate={{ scale: [1, 1.2, 1], opacity: [0.3, 0.6, 0.3] }} transition={{ duration: 3, repeat: Infinity, ease: "easeInOut" }} className="absolute -inset-8 bg-aegis-cyan rounded-full blur-3xl" />
                                <Shield className="w-20 h-20 text-aegis-cyan relative z-10" />
                            </div>
                            <div className="text-center">
                                <h1 className="text-4xl font-bold tracking-[0.3em] mb-2 uppercase text-white">Aegis <span className="text-aegis-cyan">Shell</span></h1>
                                <p className="text-aegis-steel font-mono tracking-widest text-[10px] animate-pulse">CONECTANDO CON TU ASISTENTE PERSONAL...</p>
                            </div>
                        </motion.div>
                    ) : (
                        <motion.div key="terminal" initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.5, ease: "easeOut" }} className="h-screen w-full">
                            <Suspense fallback={<ViewFallback />}>
                                {currentView === 'chat' ? (
                                    <ChatTerminal />
                                ) : currentView === 'agents' ? (
                                    <AgentInboxList />
                                ) : currentView === 'agent_thread' ? (
                                    <AgentThread />
                                ) : (
                                    <Dashboard />
                                )}
                                {/* CORE-FIX: supervisor questions pop up over ANY view
                                    (chat / agents / dashboard) so the user can answer
                                    inline instead of hunting for the agents thread. */}
                                <SupervisorQuestionModal />
                            </Suspense>
                        </motion.div>
                    )}
                </AnimatePresence>
            </AegisErrorBoundary>
        </div>
    );
}

export default App;

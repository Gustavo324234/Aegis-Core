import { useEffect, useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Shield } from 'lucide-react';
import { useAegisStore } from './store/useAegisStore';

// Aegis Shell UI Core Components
import ChatTerminal from './components/ChatTerminal';
import LoginScreen from './components/LoginScreen';
import AdminDashboard from './components/AdminDashboard';
import ForcePasswordChangeScreen from './components/ForcePasswordChangeScreen';
import BootstrapSetup from './components/BootstrapSetup';
import EngineSetupWizard from './components/EngineSetupWizard';

function App() {
    const { 
        _hydrated, status, isAuthenticated, isAdmin, systemState, 
        tenantId, sessionKey, connect, fetchSystemState, 
        isEngineConfigured, setEngineConfigured, 
        needsPasswordReset, setNeedsPasswordReset 
    } = useAegisStore();
    
    const [setupToken, setSetupToken] = useState<string | null>(null);

    // Initial hydration and system state check
    useEffect(() => {
        if (_hydrated) {
            fetchSystemState();
        }
    }, [_hydrated, fetchSystemState]);

    // Handle setup tokens from URL
    useEffect(() => {
        const params = new URLSearchParams(window.location.search);
        const token = params.get('setup_token');
        if (token) {
            setSetupToken(token);
            // Clean URL without reloading
            window.history.replaceState({}, document.title, window.location.pathname);
        }
    }, []);

    // Check for global cognitive engine if user is authenticated but enclave engine not set
    useEffect(() => {
        if (!_hydrated || !isAuthenticated || isAdmin || needsPasswordReset || isEngineConfigured) return;

        const checkGlobalEngine = async () => {
            try {
                const res = await fetch('/api/engine/status');
                if (res.ok) {
                    const data = await res.json();
                    if (data.configured) {
                        setEngineConfigured(true);
                    }
                }
            } catch (err) {
                console.error('Failed to check global engine:', err);
            }
        };

        checkGlobalEngine();
    }, [_hydrated, isAuthenticated, isAdmin, needsPasswordReset, isEngineConfigured, setEngineConfigured]);

    // Connect WebSocket orchestrator
    useEffect(() => {
        if (_hydrated && isAuthenticated && !isAdmin && !needsPasswordReset && isEngineConfigured && tenantId && sessionKey && status === 'disconnected') {
            connect(tenantId, sessionKey);
        }
    }, [_hydrated, isAuthenticated, isAdmin, needsPasswordReset, isEngineConfigured, tenantId, sessionKey, status, connect]);

    return (
        <div className="bg-black min-h-screen text-white overflow-hidden">
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
                        <ChatTerminal />
                    </motion.div>
                )}
            </AnimatePresence>
        </div>
    );
}

export default App;

// CORE-270: lista de supervisores activos en la sesión
import { ArrowLeft, Zap, CheckCircle2, Clock } from 'lucide-react';
import { useAgentInboxStore, type AgentMessage } from '../store/agentInboxStore';
import { useAegisStore } from '../store/useAegisStore';

function formatRelativeTime(timestamp: string): string {
  const diff = Date.now() - new Date(timestamp).getTime();
  const minutes = Math.floor(diff / 60_000);
  if (minutes < 1) return 'ahora';
  if (minutes === 1) return 'hace 1 minuto';
  if (minutes < 60) return `hace ${minutes} minutos`;
  const hours = Math.floor(minutes / 60);
  if (hours === 1) return 'hace 1 hora';
  return `hace ${hours} horas`;
}

function StatusIcon({ status }: { status: AgentMessage['status'] }) {
  if (status === 'pending')
    return <Zap className="w-3.5 h-3.5 text-aegis-cyan" />;
  if (status === 'answered')
    return <CheckCircle2 className="w-3.5 h-3.5 text-green-400" />;
  return <Clock className="w-3.5 h-3.5 text-white/30" />;
}

function AgentRow({ msg }: { msg: AgentMessage }) {
  const setCurrentView = useAegisStore((s) => s.setCurrentView);
  const setActiveAgentId = useAegisStore((s) => s.setActiveAgentId);

  const handleOpen = () => {
    setActiveAgentId(msg.agentId);
    setCurrentView('agent_thread');
  };

  return (
    <div className="flex items-start gap-3 p-4 border-b border-white/5 hover:bg-white/3 transition-colors">
      <div className="mt-0.5 shrink-0">
        <StatusIcon status={msg.status} />
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 mb-0.5">
          <span className="text-[11px] font-mono font-bold text-white uppercase tracking-wider truncate">
            {msg.projectName}
          </span>
          {msg.status === 'timed_out' && (
            <span className="text-[9px] font-mono text-white/30 uppercase tracking-widest">
              Sin respuesta
            </span>
          )}
        </div>
        <p className="text-xs text-white/50 font-mono truncate">{msg.question}</p>
        <span className="text-[9px] font-mono text-white/20 mt-0.5 block">
          {formatRelativeTime(msg.timestamp)}
        </span>
      </div>
      {msg.status === 'pending' && (
        <button
          onClick={handleOpen}
          className="shrink-0 text-[9px] font-mono uppercase tracking-widest text-aegis-cyan hover:text-white border border-aegis-cyan/30 hover:border-white/20 px-2 py-1 rounded transition-colors"
        >
          Abrir →
        </button>
      )}
    </div>
  );
}

export function AgentInboxList() {
  const messages = useAgentInboxStore((s) => s.messages);
  const setCurrentView = useAegisStore((s) => s.setCurrentView);

  return (
    <div className="aegis-screen bg-black text-white font-sans flex flex-col">
      <header
        className="shrink-0 border-b border-white/5 flex items-center gap-4 px-8 bg-black/40 backdrop-blur-3xl z-50"
        style={{ height: '56px' }}
      >
        <button
          onClick={() => setCurrentView('chat')}
          className="flex items-center gap-2 text-white/40 hover:text-aegis-cyan transition-colors"
        >
          <ArrowLeft className="w-3.5 h-3.5" />
          <span className="text-[10px] font-mono uppercase tracking-[0.2em]">Chat</span>
        </button>
        <div className="h-4 w-px bg-white/10" />
        <h1 className="text-[10px] font-mono tracking-[0.4em] text-white font-bold uppercase">
          Agentes activos
        </h1>
      </header>

      <main className="flex-1 overflow-y-auto scrollbar-hide">
        {messages.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full gap-4 text-center p-8">
            <Zap className="w-8 h-8 text-white/10" />
            <p className="text-[10px] font-mono text-white/20 uppercase tracking-[0.3em]">
              Sin supervisores activos
            </p>
          </div>
        ) : (
          messages.map((msg) => <AgentRow key={msg.agentId} msg={msg} />)
        )}
      </main>
    </div>
  );
}

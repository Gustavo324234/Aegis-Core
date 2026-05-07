// CORE-270: hilo de conversación con un supervisor
import React, { useState, useRef, useEffect } from 'react';
import { ArrowLeft, Send, Zap, CheckCircle2, Clock } from 'lucide-react';
import { useAgentInboxStore, type ThreadMessage } from '../store/agentInboxStore';
import { useAegisStore } from '../store/useAegisStore';

function formatTime(timestamp: string): string {
  return new Date(timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

function MessageBubble({ msg }: { msg: ThreadMessage }) {
  const isUser = msg.role === 'user';
  return (
    <div className={`flex w-full gap-4 px-2 ${isUser ? 'justify-end' : 'justify-start'}`}>
      <div className={`max-w-[85%] flex flex-col gap-1 ${isUser ? 'items-end' : 'items-start'}`}>
        <span className={`text-[9px] font-mono uppercase tracking-widest px-1 ${isUser ? 'text-white/40' : 'text-aegis-cyan/60'}`}>
          {isUser ? 'Tú' : 'Supervisor'}
        </span>
        <div
          className={`rounded-2xl px-4 py-3 text-sm shadow-lg ${
            isUser
              ? 'bg-aegis-cyan/10 border border-aegis-cyan/20 text-white rounded-tr-none'
              : 'bg-white/5 border border-white/10 text-white/90 rounded-tl-none'
          }`}
        >
          <p className="whitespace-pre-wrap font-mono text-xs leading-relaxed">{msg.content}</p>
        </div>
        <span className="text-[9px] font-mono text-white/10 px-1">{formatTime(msg.timestamp)}</span>
      </div>
    </div>
  );
}

export function AgentThread() {
  const activeAgentId = useAegisStore((s) => s.activeAgentId);
  const setCurrentView = useAegisStore((s) => s.setCurrentView);
  const { getByAgentId, addThreadMessage, markAnswered } = useAgentInboxStore();
  const [input, setInput] = useState('');
  const [isSending, setIsSending] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  const msg = activeAgentId ? getByAgentId(activeAgentId) : undefined;

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTo({ top: scrollRef.current.scrollHeight, behavior: 'smooth' });
    }
  }, [msg?.thread.length]);

  if (!msg) {
    return (
      <div className="aegis-screen bg-black text-white font-sans flex flex-col items-center justify-center">
        <p className="text-[10px] font-mono text-white/20 uppercase tracking-widest">Agente no encontrado</p>
        <button
          onClick={() => setCurrentView('agents')}
          className="mt-4 text-[9px] font-mono text-aegis-cyan uppercase tracking-widest"
        >
          ← Volver
        </button>
      </div>
    );
  }

  const isPending = msg.status === 'pending';

  const handleSend = async () => {
    if (!input.trim() || !isPending || isSending) return;
    const answer = input.trim();
    setInput('');
    setIsSending(true);

    const now = new Date().toISOString();

    try {
      const { tenantId, sessionKey } = useAegisStore.getState();
      const res = await fetch(`/api/agents/${encodeURIComponent(msg.agentId)}/reply`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...(tenantId && { 'x-citadel-tenant': tenantId }),
          ...(sessionKey && { 'x-citadel-key': sessionKey }),
        },
        body: JSON.stringify({ reply: answer }),
      });

      if (res.ok) {
        addThreadMessage(msg.agentId, { role: 'user', content: answer, timestamp: now });
        markAnswered(msg.agentId);
      } else {
        console.error('[AgentThread] Reply failed:', res.status);
        // Re-show input so the user can retry
        setInput(answer);
      }
    } catch (err) {
      console.error('[AgentThread] Reply error:', err);
      setInput(answer);
    } finally {
      setIsSending(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const statusLabel =
    msg.status === 'answered'
      ? 'Respondiste'
      : msg.status === 'timed_out'
      ? 'El supervisor continuó sin respuesta'
      : null;

  const StatusIcon =
    msg.status === 'pending' ? Zap :
    msg.status === 'answered' ? CheckCircle2 : Clock;

  const statusColor =
    msg.status === 'pending' ? 'text-aegis-cyan' :
    msg.status === 'answered' ? 'text-green-400' : 'text-white/30';

  return (
    <div className="aegis-screen bg-black text-white font-sans flex flex-col">
      {/* Header */}
      <header
        className="shrink-0 border-b border-white/5 flex items-center justify-between px-8 bg-black/40 backdrop-blur-3xl z-50"
        style={{ height: '56px' }}
      >
        <div className="flex items-center gap-4">
          <button
            onClick={() => setCurrentView('agents')}
            className="flex items-center gap-2 text-white/40 hover:text-aegis-cyan transition-colors"
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            <span className="text-[10px] font-mono uppercase tracking-[0.2em]">Agentes</span>
          </button>
          <div className="h-4 w-px bg-white/10" />
          <h1 className="text-[10px] font-mono tracking-[0.3em] text-white font-bold uppercase truncate">
            {msg.projectName}
          </h1>
        </div>
        <div className="flex items-center gap-2">
          <StatusIcon className={`w-3.5 h-3.5 ${statusColor}`} />
          <span className={`text-[9px] font-mono uppercase tracking-widest ${statusColor}`}>
            {msg.status === 'pending' ? 'Pendiente' : msg.status === 'answered' ? 'Respondido' : 'Expirado'}
          </span>
        </div>
      </header>

      {/* Thread */}
      <main
        ref={scrollRef}
        className="flex-1 overflow-y-auto scrollbar-hide px-6 py-8 space-y-6"
        style={{ minHeight: 0 }}
      >
        {msg.context && (
          <div className="max-w-4xl mx-auto px-4 py-3 bg-white/3 border border-white/10 rounded-xl">
            <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mb-1">Contexto</p>
            <p className="text-xs font-mono text-white/50 leading-relaxed whitespace-pre-wrap">{msg.context}</p>
          </div>
        )}
        {msg.thread.map((m, i) => (
          <MessageBubble key={i} msg={m} />
        ))}
        <div className="h-4" />
      </main>

      {/* Footer */}
      <div className="shrink-0 p-6 bg-gradient-to-t from-black via-black/90 to-transparent border-t border-white/5">
        {statusLabel && (
          <p className="text-center text-[9px] font-mono text-white/30 uppercase tracking-widest mb-3">
            {statusLabel}
          </p>
        )}
        <div className="max-w-4xl mx-auto relative">
          <div
            className={`glass rounded-xl border flex items-end p-2 gap-2 transition-all shadow-2xl ${
              isPending
                ? 'border-white/10 focus-within:border-aegis-cyan/30'
                : 'border-white/5 opacity-50'
            }`}
          >
            <textarea
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              disabled={!isPending || isSending}
              placeholder={isPending ? 'Respondé al supervisor...' : statusLabel ?? ''}
              className="w-full bg-transparent border-none focus:ring-0 text-sm py-2 px-3 resize-none max-h-32 min-h-[40px] font-mono placeholder:text-white/20 disabled:cursor-not-allowed"
              rows={1}
            />
            <button
              onClick={handleSend}
              disabled={!isPending || !input.trim() || isSending}
              className="p-2 rounded-lg bg-aegis-cyan/10 border border-aegis-cyan/20 text-aegis-cyan hover:bg-aegis-cyan/20 transition-all disabled:opacity-30 disabled:cursor-not-allowed shrink-0"
            >
              <Send className="w-4 h-4" />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

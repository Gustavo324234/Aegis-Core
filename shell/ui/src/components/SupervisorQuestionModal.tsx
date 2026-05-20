// CORE-FIX: surface supervisor `ask_user` questions as a popup in the MAIN
// chat instead of the separate "Agentes" view. The user reported that
// answering in the agents view was confusing and that a second question from
// the same supervisor blocked everything. This modal:
//   - reads the first still-pending question from the inbox store,
//   - shows it over whatever view is active (chat, dashboard, …),
//   - lets the user click "Sí, adelante" / "No" or type a free-text answer,
//   - POSTs to /api/agents/:id/reply and advances to the next queued question.
import React, { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { Check, X, Send, MessageCircleQuestion } from 'lucide-react';
import { useAgentInboxStore } from '../store/agentInboxStore';
import { useAegisStore } from '../store/useAegisStore';

export function SupervisorQuestionModal() {
  // Subscribe to messages so the modal re-renders when a question arrives or
  // is answered. firstPending() recomputes from the latest list each render.
  const messages = useAgentInboxStore((s) => s.messages);
  const addThreadMessage = useAgentInboxStore((s) => s.addThreadMessage);
  const markAnswered = useAgentInboxStore((s) => s.markAnswered);

  const pending = messages.find((m) => m.status === 'pending');

  const [input, setInput] = useState('');
  const [isSending, setIsSending] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // Reset the input whenever a different question becomes active.
  useEffect(() => {
    setInput('');
    setErrorMessage(null);
  }, [pending?.agentId, pending?.question]);

  if (!pending) return null;

  const submit = async (answer: string) => {
    if (!answer.trim() || isSending) return;
    setErrorMessage(null);
    setIsSending(true);
    const now = new Date().toISOString();
    try {
      const { tenantId, sessionKey } = useAegisStore.getState();
      const res = await fetch(
        `/api/agents/${encodeURIComponent(pending.agentId)}/reply`,
        {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            ...(tenantId && { 'x-citadel-tenant': tenantId }),
            ...(sessionKey && { 'x-citadel-key': sessionKey }),
          },
          body: JSON.stringify({ answer }),
        },
      );
      if (res.ok) {
        addThreadMessage(pending.agentId, { role: 'user', content: answer, timestamp: now });
        markAnswered(pending.agentId);
        setInput('');
      } else if (res.status === 404) {
        // Supervisor already moved on (timed out) — clear it from the queue.
        markAnswered(pending.agentId);
        setErrorMessage('El supervisor ya no esperaba respuesta.');
      } else {
        setErrorMessage('No se pudo enviar la respuesta. Reintentá.');
      }
    } catch {
      setErrorMessage('Error de conexión.');
    } finally {
      setIsSending(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      void submit(input);
    }
  };

  const pendingCount = messages.filter((m) => m.status === 'pending').length;

  return (
    <AnimatePresence>
      <motion.div
        key="supervisor-question-backdrop"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 z-[120] flex items-center justify-center p-4 bg-black/80 backdrop-blur-sm"
      >
        <motion.div
          key={`${pending.agentId}:${pending.question}`}
          initial={{ opacity: 0, scale: 0.96, y: 12 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.96, y: 12 }}
          transition={{ duration: 0.2, ease: 'easeOut' }}
          className="glass w-full max-w-lg rounded-2xl border border-aegis-cyan/25 shadow-2xl overflow-hidden"
        >
          {/* Header */}
          <div className="flex items-center justify-between px-6 py-4 border-b border-white/5 bg-aegis-cyan/5">
            <div className="flex items-center gap-3">
              <MessageCircleQuestion className="w-4 h-4 text-aegis-cyan" />
              <div className="flex flex-col">
                <span className="text-[9px] font-mono uppercase tracking-[0.2em] text-aegis-cyan/70">
                  Supervisor pregunta
                </span>
                <span className="text-[11px] font-mono uppercase tracking-widest text-white/80 truncate max-w-[20rem]">
                  {pending.projectName}
                </span>
              </div>
            </div>
            {pendingCount > 1 && (
              <span className="text-[9px] font-mono text-white/30 uppercase tracking-widest">
                +{pendingCount - 1} en cola
              </span>
            )}
          </div>

          {/* Body */}
          <div className="px-6 py-5 space-y-4">
            {pending.context && (
              <div className="px-4 py-3 bg-white/3 border border-white/10 rounded-xl">
                <p className="text-[9px] font-mono text-white/30 uppercase tracking-widest mb-1">
                  Contexto
                </p>
                <p className="text-xs font-mono text-white/50 leading-relaxed whitespace-pre-wrap">
                  {pending.context}
                </p>
              </div>
            )}
            <p className="text-sm text-white/90 leading-relaxed whitespace-pre-wrap">
              {pending.question}
            </p>

            {/* Free-text answer */}
            <div className="glass rounded-xl border border-white/10 focus-within:border-aegis-cyan/30 flex items-end p-2 gap-2 transition-all">
              <textarea
                value={input}
                onChange={(e) => setInput(e.target.value)}
                onKeyDown={handleKeyDown}
                disabled={isSending}
                placeholder="Escribí una respuesta… (o usá los botones)"
                className="w-full bg-transparent border-none focus:ring-0 text-sm py-2 px-3 resize-none max-h-32 min-h-[40px] font-mono placeholder:text-white/20 disabled:cursor-not-allowed"
                rows={1}
                autoFocus
              />
              <button
                onClick={() => void submit(input)}
                disabled={!input.trim() || isSending}
                className="p-2 rounded-lg bg-aegis-cyan/10 border border-aegis-cyan/20 text-aegis-cyan hover:bg-aegis-cyan/20 transition-all disabled:opacity-30 disabled:cursor-not-allowed shrink-0"
                title="Enviar respuesta"
              >
                <Send className="w-4 h-4" />
              </button>
            </div>

            {errorMessage && (
              <p className="text-[10px] font-mono text-red-400/80">{errorMessage}</p>
            )}
          </div>

          {/* Quick actions */}
          <div className="flex gap-2 px-6 py-4 border-t border-white/5">
            <button
              onClick={() => void submit('Sí, adelante.')}
              disabled={isSending}
              className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg bg-green-500/10 border border-green-500/25 text-green-300 text-[11px] font-mono uppercase tracking-widest hover:bg-green-500/20 transition-all disabled:opacity-40"
            >
              <Check className="w-3.5 h-3.5" /> Sí, adelante
            </button>
            <button
              onClick={() => void submit('No.')}
              disabled={isSending}
              className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-lg bg-white/5 border border-white/10 text-white/60 text-[11px] font-mono uppercase tracking-widest hover:bg-white/10 transition-all disabled:opacity-40"
            >
              <X className="w-3.5 h-3.5" /> No
            </button>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}

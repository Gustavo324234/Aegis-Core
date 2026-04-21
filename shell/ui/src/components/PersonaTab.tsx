import React, { useState, useEffect, useMemo } from 'react';
import { Bot, Save, RotateCcw, CheckCircle, AlertCircle } from 'lucide-react';

const MAX_PERSONA_CHARS = 4000;

const PersonaTab: React.FC<{ tenantId: string | null; sessionKey: string | null }> = ({
  tenantId,
  sessionKey,
}) => {
  const [persona, setPersona] = useState('');
  const [saved, setSaved] = useState('');
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [status, setStatus] = useState<'idle' | 'saved' | 'error'>('idle');
  const [errorMsg, setErrorMsg] = useState('');

  const citadelHeaders = useMemo(() => ({
    'Content-Type': 'application/json',
    'x-citadel-tenant': tenantId ?? '',
    'x-citadel-key': sessionKey ?? '',
  }), [tenantId, sessionKey]);

  useEffect(() => {
    if (!tenantId || !sessionKey) return;
    setIsLoading(true);
    fetch('/api/persona', { headers: citadelHeaders })
      .then((r) => r.json())
      .then((data) => {
        const value = data.persona ?? '';
        setPersona(value);
        setSaved(value);
      })
      .catch(() => setErrorMsg('Error cargando persona'))
      .finally(() => setIsLoading(false));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tenantId, sessionKey]);

  const handleSave = async () => {
    if (!tenantId || !sessionKey) return;
    setIsSaving(true);
    setStatus('idle');
    try {
      const res = await fetch('/api/persona', {
        method: 'POST',
        headers: citadelHeaders,
        body: JSON.stringify({ persona }),
      });
      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.error ?? 'Error al guardar');
      }
      setSaved(persona);
      setStatus('saved');
      setTimeout(() => setStatus('idle'), 3000);
    } catch (e: unknown) {
      setErrorMsg(e instanceof Error ? e.message : 'Error desconocido');
      setStatus('error');
    } finally {
      setIsSaving(false);
    }
  };

  const handleReset = async () => {
    if (!tenantId || !sessionKey) return;
    await fetch('/api/persona', { method: 'DELETE', headers: citadelHeaders });
    setPersona('');
    setSaved('');
    setStatus('idle');
  };

  const isDirty = persona !== saved;
  const charsLeft = MAX_PERSONA_CHARS - persona.length;
  const isOverLimit = charsLeft < 0;

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-16">
        <Bot className="w-6 h-6 text-aegis-cyan animate-pulse" />
        <span className="text-xs font-mono text-white/40 uppercase ml-3 tracking-widest">
          Cargando persona...
        </span>
      </div>
    );
  }

  return (
    <div className="space-y-6 max-w-3xl">
      <div className="glass p-6 rounded-2xl border border-white/10">
        <div className="flex items-center gap-3 mb-3">
          <Bot className="w-5 h-5 text-aegis-cyan" />
          <h2 className="text-lg font-bold tracking-widest uppercase">Identidad del Agente</h2>
        </div>
        <p className="text-xs font-mono text-white/40 leading-relaxed">
          Define el system prompt que determina la identidad, tono y capacidades del agente
          para este tenant. Si está vacío, el agente se presenta como "Aegis" por defecto.
          Máximo {MAX_PERSONA_CHARS} caracteres.
        </p>
      </div>

      <div className="glass p-6 rounded-2xl border border-white/10 space-y-4">
        <textarea
          value={persona}
          onChange={(e) => setPersona(e.target.value)}
          placeholder={
            'Eres Eva, asistente de ACME Corp.\n' +
            'Tu especialidad es atención al cliente.\n' +
            'Siempre responde en español formal.\n' +
            'No discutas temas fuera de los productos de ACME.'
          }
          rows={10}
          className={`w-full bg-black/40 border rounded-lg py-3 px-4 text-sm font-mono resize-y
            focus:ring-0 transition-all placeholder:text-white/15 leading-relaxed
            ${isOverLimit ? 'border-red-500/50 focus:border-red-500' : 'border-white/10 focus:border-aegis-cyan/50'}`}
        />

        <div className="flex items-center justify-between">
          <span
            className={`text-[10px] font-mono uppercase tracking-widest
              ${isOverLimit ? 'text-red-400' : charsLeft < 200 ? 'text-yellow-400' : 'text-white/30'}`}
          >
            {isOverLimit ? `${Math.abs(charsLeft)} sobre el límite` : `${charsLeft} restantes`}
          </span>

          <div className="flex items-center gap-3">
            {saved && (
              <button
                onClick={handleReset}
                className="flex items-center gap-2 px-3 py-1.5 border border-white/10 rounded-lg
                  hover:bg-white/5 text-[10px] font-mono uppercase text-white/40 hover:text-white/60 transition-colors"
              >
                <RotateCcw className="w-3 h-3" /> Restaurar default
              </button>
            )}
            <button
              onClick={handleSave}
              disabled={isSaving || isOverLimit || !isDirty}
              className={`flex items-center gap-2 px-4 py-1.5 rounded-lg text-[10px] font-mono
                uppercase font-bold tracking-widest transition-colors
                ${
                  isSaving || !isDirty || isOverLimit
                    ? 'bg-aegis-cyan/10 text-aegis-cyan/40 cursor-not-allowed border border-aegis-cyan/10'
                    : 'bg-aegis-cyan/20 hover:bg-aegis-cyan/30 text-aegis-cyan border border-aegis-cyan/30'
                }`}
            >
              <Save className="w-3 h-3" />
              {isSaving ? 'Guardando...' : 'Guardar'}
            </button>
          </div>
        </div>

        {status === 'saved' && (
          <div className="flex items-center gap-2 text-green-400 text-[10px] font-mono uppercase">
            <CheckCircle className="w-3.5 h-3.5" /> Persona guardada — activa en el próximo mensaje
          </div>
        )}
        {status === 'error' && (
          <div className="flex items-center gap-2 text-red-400 text-[10px] font-mono uppercase">
            <AlertCircle className="w-3.5 h-3.5" /> {errorMsg}
          </div>
        )}
      </div>

      {persona.trim() && (
        <div className="glass p-6 rounded-2xl border border-aegis-cyan/20 bg-aegis-cyan/5">
          <p className="text-[10px] font-mono text-aegis-cyan/60 uppercase tracking-widest mb-3">
            Preview — identidad activa
          </p>
          <p className="text-sm font-mono text-white/70 whitespace-pre-wrap leading-relaxed">
            {persona.length > 300 ? persona.slice(0, 300) + '...' : persona}
          </p>
        </div>
      )}
    </div>
  );
};

export default PersonaTab;
// CORE-269: badge de mensajes pendientes de supervisores en la barra de navegación
import { Zap } from 'lucide-react';
import { useAgentInboxStore } from '../store/agentInboxStore';
import { useAegisStore } from '../store/useAegisStore';

export function AgentBadge() {
  const pendingCount = useAgentInboxStore((s) => s.pendingCount);
  const setCurrentView = useAegisStore((s) => s.setCurrentView);

  if (pendingCount === 0) return null;

  return (
    <button
      onClick={() => setCurrentView('agents')}
      className="relative p-1.5 rounded-md hover:bg-white/5 text-white/40 hover:text-aegis-cyan transition-colors"
      title={`${pendingCount} mensaje${pendingCount > 1 ? 's' : ''} pendiente${pendingCount > 1 ? 's' : ''} de supervisores`}
    >
      <Zap className="w-3.5 h-3.5" />
      <span className="absolute -top-1 -right-1 flex h-4 w-4 items-center justify-center rounded-full bg-primary text-[10px] text-primary-foreground font-medium bg-aegis-cyan text-black">
        {pendingCount > 9 ? '9+' : pendingCount}
      </span>
    </button>
  );
}

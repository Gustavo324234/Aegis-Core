# CORE-269 — Shell: `AgentInbox` store + `AgentBadge` en nav

**Tipo:** feat  
**Prioridad:** Crítica  
**Épica:** EPIC 50 — Agent Inbox  
**Asignado a:** Shell Engineer  
**Componentes:**
- `shell/ui/src/stores/agentInboxStore.ts` (nuevo)
- `shell/ui/src/services/websocket.ts` (o el archivo que maneja el WS)
- `shell/ui/src/components/nav/` (agregar AgentBadge)
**Estado:** ✅ Completo

---

## Contexto

Con CORE-268, el kernel emite frames JSON por WebSocket cuando un supervisor tiene
una pregunta pendiente. Este ticket es la contraparte en la shell: recibir esos eventos,
mantener el estado de la bandeja de entrada de agentes, y mostrarlo en la navegación.

---

## Cambios requeridos

### 1. `agentInboxStore.ts` — store Zustand

```typescript
import { create } from 'zustand';

export interface AgentMessage {
  agentId: string;
  projectName: string;
  question: string;
  context?: string;
  timestamp: string;
  status: 'pending' | 'answered' | 'expired';
}

interface AgentInboxState {
  messages: AgentMessage[];
  pendingCount: number;
  addMessage: (msg: AgentMessage) => void;
  markAnswered: (agentId: string) => void;
  markExpired: (agentId: string) => void;
  getByAgentId: (agentId: string) => AgentMessage | undefined;
}

export const useAgentInboxStore = create<AgentInboxState>((set, get) => ({
  messages: [],
  pendingCount: 0,
  addMessage: (msg) =>
    set((state) => {
      // Evitar duplicados por agentId
      const exists = state.messages.some(m => m.agentId === msg.agentId && m.status === 'pending');
      if (exists) return state;
      const updated = [...state.messages, msg];
      return { messages: updated, pendingCount: updated.filter(m => m.status === 'pending').length };
    }),
  markAnswered: (agentId) =>
    set((state) => {
      const updated = state.messages.map(m =>
        m.agentId === agentId ? { ...m, status: 'answered' as const } : m
      );
      return { messages: updated, pendingCount: updated.filter(m => m.status === 'pending').length };
    }),
  markExpired: (agentId) =>
    set((state) => {
      const updated = state.messages.map(m =>
        m.agentId === agentId ? { ...m, status: 'expired' as const } : m
      );
      return { messages: updated, pendingCount: updated.filter(m => m.status === 'pending').length };
    }),
  getByAgentId: (agentId) =>
    get().messages.find(m => m.agentId === agentId),
}));
```

### 2. Parsear eventos de agente en el handler WebSocket

En el archivo que maneja los mensajes entrantes del WebSocket, agregar el parseo de
frames de agente. Los frames de texto que lleguen con `"type"` en el JSON son eventos
de agente — los que no tienen ese campo son tokens de chat:

```typescript
// En el handler de mensajes WS:
const handleWsMessage = (raw: string) => {
  try {
    const parsed = JSON.parse(raw);
    
    if (parsed.type === 'supervisor_question') {
      useAgentInboxStore.getState().addMessage({
        agentId: parsed.agent_id,
        projectName: parsed.project_name,
        question: parsed.question,
        context: parsed.context,
        timestamp: parsed.timestamp,
        status: 'pending',
      });
      return; // No procesar como token de chat
    }
    
    if (parsed.type === 'supervisor_resumed') {
      // El supervisor recibió respuesta — no cambiar estado aquí,
      // markAnswered se llama cuando el usuario responde (CORE-271)
      return;
    }
    
    if (parsed.type === 'supervisor_completed') {
      useAgentInboxStore.getState().markAnswered(parsed.agent_id);
      return;
    }
  } catch {
    // No es JSON — es un token de texto normal del chat, procesar como antes
  }
  
  // ... procesamiento existente de tokens de chat ...
};
```

### 3. Componente `AgentBadge`

```typescript
// shell/ui/src/components/nav/AgentBadge.tsx
import { useAgentInboxStore } from '../../stores/agentInboxStore';

export function AgentBadge() {
  const pendingCount = useAgentInboxStore(s => s.pendingCount);
  
  if (pendingCount === 0) return null;
  
  return (
    <div className="relative">
      {/* Ícono de agente/robot — usar el ícono existente del sistema o un svg simple */}
      <AgentIcon className="w-5 h-5 text-muted-foreground" />
      <span className="absolute -top-1 -right-1 flex h-4 w-4 items-center justify-center 
                       rounded-full bg-primary text-[10px] text-primary-foreground font-medium">
        {pendingCount > 9 ? '9+' : pendingCount}
      </span>
    </div>
  );
}
```

Integrar `AgentBadge` en la barra de navegación principal, junto a los otros controles.
Al hacer click en el badge, navegar a `/chat/agent` (lista de supervisores con pendientes)
— esa ruta se implementa en CORE-270.

---

## Criterios de aceptación

- [x] `agentInboxStore.ts` existe con los métodos descritos
- [x] Cuando el WebSocket recibe un frame `supervisor_question`, el store lo agrega y `pendingCount` sube
- [x] Cuando el WebSocket recibe `supervisor_completed`, el mensaje se marca como answered y `pendingCount` baja
- [x] `AgentBadge` muestra el número correcto de pendientes y desaparece cuando es 0
- [x] Los frames de tipo conocido (`supervisor_*`) no se procesan como tokens de chat
- [x] El parseo JSON fallido en el handler no rompe el procesamiento de tokens normales

## Dependencias

- CORE-268 (kernel emite los eventos)

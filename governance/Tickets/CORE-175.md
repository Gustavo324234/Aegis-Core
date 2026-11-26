# CORE-175 — feat: Eventos WebSocket — terminal_output, pr_update, pr_merged, git_push, ci_fix_attempt

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-core + shell/ui  
**Tipo:** feat  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-168, CORE-173, CORE-172
**Estado:** ✅ Completo

---

## Contexto

La comunicación entre el backend y el frontend ya usa WebSocket con eventos tipados (ver `useAegisStore.ts` — los casos `kernel_event`, `music_play`, `music_control`). Este ticket define y registra los nuevos tipos de eventos de la Epic 44, tanto en el backend (emisión) como en el frontend (recepción en el store).

---

## Cambios Requeridos

### 1. Enum `WsEvent` en `ank-core` (o donde esté definido)

Agregar las nuevas variantes al enum de eventos WebSocket del servidor:

```rust
pub enum WsEvent {
    // ... variantes existentes ...

    /// Output de terminal de un agente en tiempo real
    TerminalOutput {
        agent_id: AgentId,
        command: String,
        line: TerminalLine,
    },

    /// Cambio de estado de un PR
    PrUpdate {
        pr_number: u64,
        status: PrStatus,
        url: String,
    },

    /// Un PR fue mergeado (por Aegis o por el usuario en GitHub)
    PrMerged {
        pr_number: u64,
        title: String,
    },

    /// Un agente pusheó una branch
    GitPush {
        agent_id: AgentId,
        branch: String,
        commit_sha: String,
    },

    /// El auto-fix de CI inició un intento
    CiFixAttempt {
        pr_number: u64,
        attempt: u32,
        max_attempts: u32,
        error_summary: String,
    },

    /// Notificación de texto al chat del tenant (para alertas importantes)
    ChatNotification {
        message: String,
    },
}
```

### 2. Serialización de los eventos al WebSocket

Cada `WsEvent` se serializa como:

```json
{ "event": "<nombre>", "data": { ... } }
```

Mapa de nombres:

| Variante | `"event"` |
|---|---|
| `TerminalOutput` | `"terminal_output"` |
| `PrUpdate` | `"pr_update"` |
| `PrMerged` | `"pr_merged"` |
| `GitPush` | `"git_push"` |
| `CiFixAttempt` | `"ci_fix_attempt"` |
| `ChatNotification` | `"chat_notification"` |

### 3. Recepción en `useAegisStore.ts` — `socket.onmessage`

Agregar los nuevos `case` al switch existente:

```typescript
case 'terminal_output': {
  const d = data as {
    agent_id: string;
    command: string;
    line: { kind: 'Stdout' | 'Stderr' | 'System'; content: string; timestamp: string };
  };
  // Emitir al store de terminal (ver CORE-176)
  useTerminalStore.getState().appendLine(d.agent_id, d.command, d.line);
  break;
}

case 'pr_update': {
  const d = data as { pr_number: number; status: string; url: string };
  usePrStore.getState().updatePrStatus(d.pr_number, d.status);
  break;
}

case 'pr_merged': {
  const d = data as { pr_number: number; title: string };
  usePrStore.getState().updatePrStatus(d.pr_number, 'Merged');
  // Agregar mensaje al chat notificando el merge
  get().addSystemMessage(`✅ PR #${d.pr_number} mergeado: "${d.title}"`);
  break;
}

case 'git_push': {
  const d = data as { agent_id: string; branch: string; commit_sha: string };
  get().addSystemMessage(`📦 Branch \`${d.branch}\` pusheada por agente (${d.commit_sha.slice(0, 7)})`);
  break;
}

case 'ci_fix_attempt': {
  const d = data as { pr_number: number; attempt: number; max_attempts: number; error_summary: string };
  get().addSystemMessage(`🔧 Auto-fix CI — PR #${d.pr_number} — Intento ${d.attempt}/${d.max_attempts}`);
  break;
}

case 'chat_notification': {
  const d = data as { message: string };
  get().addSystemMessage(d.message);
  break;
}
```

### 4. Stores auxiliares necesarios

Este ticket requiere que existan (o se creen vacíos como stub) dos stores de Zustand:
- `useTerminalStore` — para CORE-176 (TerminalPanel)
- `usePrStore` — para CORE-179 (PRManager UI)

Si CORE-176 y CORE-179 aún no están implementados, crear los stores como stubs:

```typescript
// src/store/terminalStore.ts
export const useTerminalStore = create(() => ({
  lines: {} as Record<string, TerminalLineEntry[]>,
  appendLine: (agentId: string, command: string, line: TerminalLineDto) => { /* stub */ },
}));

// src/store/prStore.ts
export const usePrStore = create(() => ({
  prs: [] as PrDto[],
  updatePrStatus: (prNumber: number, status: string) => { /* stub */ },
}));
```

---

## Criterios de Aceptación

- [x] Los 6 nuevos tipos de evento (`terminal_output`, `pr_update`, `pr_merged`, `git_push`, `ci_fix_attempt`, `chat_notification`) están definidos en el enum del backend y se serializan correctamente
- [x] `useAegisStore.ts` maneja los 6 casos nuevos en el `switch` de `socket.onmessage` sin errors TypeScript
- [x] El evento `pr_merged` agrega un mensaje al chat del tenant con el formato `✅ PR #N mergeado: "título"`
- [x] El evento `git_push` agrega un mensaje al chat con la branch y SHA corto
- [x] Los stores stub `useTerminalStore` y `usePrStore` existen y no rompen el build
- [x] `cargo build` pasa sin warnings (backend)
- [x] `npm run build` pasa sin errores TypeScript (frontend)

---

*Arquitecto IA — 2026-04-24*
# CORE-176 — feat: TerminalPanel — UI de Terminal en Dashboard del Tenant

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / shell/ui  
**Tipo:** feat  
**Prioridad:** Alta  
**Asignado a:** Shell Engineer  
**Dependencias:** CORE-175 (eventos WebSocket + useTerminalStore)
**Estado:** ✅ Completo

---

## Contexto

El tenant necesita ver en tiempo real los comandos que ejecutan los agentes y su output. El TerminalPanel es un componente tipo terminal que muestra las líneas de output streameadas por WebSocket, agrupadas por agente y comando.

Se integra en el `Dashboard.tsx` del tenant, debajo del `AgentTreeWidget` (CORE-166).

---

## Cambios Requeridos

### 1. Completar `src/store/terminalStore.ts`

Reemplazar el stub creado en CORE-175 con la implementación real:

```typescript
interface TerminalLineEntry {
  kind: 'Stdout' | 'Stderr' | 'System';
  content: string;
  timestamp: string;
}

interface TerminalSession {
  agentId: string;
  command: string;
  lines: TerminalLineEntry[];
  startedAt: string;
  exitCode: number | null;
}

interface TerminalState {
  sessions: TerminalSession[];          // últimas N sesiones
  appendLine: (agentId: string, command: string, line: TerminalLineEntry) => void;
  clearSessions: () => void;
}

export const useTerminalStore = create<TerminalState>((set) => ({
  sessions: [],
  appendLine: (agentId, command, line) => set((state) => {
    const sessions = [...state.sessions];
    const existing = sessions.find(s => s.agentId === agentId && s.command === command && s.exitCode === null);
    if (existing) {
      existing.lines.push(line);
    } else {
      sessions.push({ agentId, command, lines: [line], startedAt: new Date().toISOString(), exitCode: null });
    }
    // Mantener máximo 20 sesiones en memoria
    return { sessions: sessions.slice(-20) };
  }),
  clearSessions: () => set({ sessions: [] }),
}));
```

### 2. Nuevo archivo: `src/components/TerminalPanel.tsx`

```typescript
// Aspecto visual: fondo oscuro tipo terminal (#0a0a0a), fuente monospace, texto verde/blanco/rojo según kind
// Altura fija con scroll interno (overflow-y-auto)
// Auto-scroll al bottom cuando llegan nuevas líneas (useEffect con ref al bottom div)
```

**Layout:**

```
┌─ TERMINAL ──────────────────────────────────────── [Limpiar] ─┐
│                                                                │
│  [▶ spec-scheduler] cargo build --workspace                   │
│  ────────────────────────────────────────────────             │
│  Compiling ank-core v0.5.0 (/home/...)          [stdout]      │
│  Compiling ank-http v0.5.0 (/home/...)          [stdout]      │
│  error[E0425]: cannot find value `foo`          [stderr] ←rojo│
│  Finished in 12.3s                              [stdout]      │
│                                                                │
│  [▶ spec-auth] cargo build --package ank-core                 │
│  ────────────────────────────────────────────────             │
│  Compiling ank-core v0.5.0 ...                  [stdout]      │
│                                               [En progreso ●] │
└───────────────────────────────────────────────────────────────┘
```

**Colores por `kind`:**
- `Stdout` → `text-green-400`
- `Stderr` → `text-red-400`
- `System` → `text-yellow-400` (mensajes del propio Aegis)

**Estado vacío:**
```
[ > _ ]
Esperando comandos de los agentes...
```

### 3. Integración en `Dashboard.tsx`

Agregar después de `AgentTreeWidget`, dentro del mismo grid:

```tsx
import TerminalPanel from './TerminalPanel';

{/* Terminal — Epic 44 */}
<div className="col-span-12 space-y-6">
  <div className="flex items-center gap-3">
    <Terminal className="w-5 h-5 text-aegis-cyan" />
    <h2 className="text-xl font-bold uppercase tracking-widest">Terminal</h2>
  </div>
  <TerminalPanel />
</div>
```

Agregar `Terminal` al import de `lucide-react`.

---

## Criterios de Aceptación

- [x] El TerminalPanel renderiza sin errores cuando no hay sesiones
- [x] Las líneas de stdout aparecen en verde, stderr en rojo, system en amarillo
- [x] El panel hace auto-scroll al bottom cuando llegan nuevas líneas
- [x] El botón "Limpiar" llama a `useTerminalStore.getState().clearSessions()`
- [x] Se muestran las últimas 20 sesiones como máximo (las más antiguas se descartan)
- [x] El panel está integrado en `Dashboard.tsx` debajo del AgentTreeWidget
- [x] `npm run build` pasa sin errores TypeScript

---

*Arquitecto IA — 2026-04-24*
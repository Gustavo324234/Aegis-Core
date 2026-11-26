# CORE-179 — feat: PRManager UI — Lista de PRs con Controles Auto/Manual

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / shell/ui  
**Tipo:** feat  
**Prioridad:** Alta  
**Asignado a:** Shell Engineer  
**Dependencias:** CORE-173 (endpoints /api/prs), CORE-175 (usePrStore)
**Estado:** ✅ Completo

---

## Contexto

El tenant necesita ver todos sus PRs, cambiar el modo de merge (auto/manual), activar/desactivar auto-fix de CI, y mergear manualmente cuando el CI pasa. El PRManager UI es el panel dedicado a eso.

---

## Cambios Requeridos

### 1. Completar `src/store/prStore.ts`

Reemplazar el stub de CORE-175 con la implementación real:

```typescript
interface PrDto {
  pr_number: number;
  title: string;
  branch: string;
  base_branch: string;
  url: string;
  merge_mode: 'automatic' | 'manual';
  auto_fix_ci: boolean;
  status: string;
  updated_at: string;
}

interface PrState {
  prs: PrDto[];
  setPrs: (prs: PrDto[]) => void;
  updatePrStatus: (prNumber: number, status: string) => void;
  updatePrSettings: (prNumber: number, settings: Partial<Pick<PrDto, 'merge_mode' | 'auto_fix_ci'>>) => void;
}

export const usePrStore = create<PrState>((set) => ({
  prs: [],
  setPrs: (prs) => set({ prs }),
  updatePrStatus: (prNumber, status) => set((state) => ({
    prs: state.prs.map(p => p.pr_number === prNumber ? { ...p, status } : p)
  })),
  updatePrSettings: (prNumber, settings) => set((state) => ({
    prs: state.prs.map(p => p.pr_number === prNumber ? { ...p, ...settings } : p)
  })),
}));
```

### 2. Nuevo archivo: `src/components/PRManagerPanel.tsx`

**Layout:**

```
┌─ PULL REQUESTS ──────────────────────────────────── [+ Nuevo PR] ─┐
│                                                                   │
│  #42  feat(ank-core): CORE-155 AgentNode              CI ✅       │
│       feat/epic-43-agent-node → main                             │
│       [◉ AUTO] [🔧 Auto-fix ON]  [Mergear ahora →]               │
│       Actualizado: hace 5 min  · Ver en GitHub ↗                 │
│  ────────────────────────────────────────────────────            │
│  #41  fix(ank-http): auth SHA-256                     CI 🔄       │
│       fix/auth-sha256 → main                                     │
│       [○ MANUAL] [🔧 Auto-fix ON]                                │
│       Actualizado: hace 20 min  · Ver en GitHub ↗               │
│  ────────────────────────────────────────────────────            │
│  (vacío cuando no hay PRs)                                       │
│  [ Sin PRs activos. Los agentes crearán PRs al completar su      │
│    trabajo. ]                                                    │
└───────────────────────────────────────────────────────────────────┘
```

**Toggle merge_mode:**

```tsx
// Toggle clickeable — llama PATCH /api/prs/{number} con { merge_mode: nuevo_valor }
<button
  onClick={() => toggleMergeMode(pr.pr_number, pr.merge_mode)}
  className={`text-[11px] font-mono px-3 py-1 rounded-full border transition-all ${
    pr.merge_mode === 'automatic'
      ? 'border-aegis-cyan text-aegis-cyan bg-aegis-cyan/10'
      : 'border-white/20 text-white/40 hover:border-white/40'
  }`}
>
  {pr.merge_mode === 'automatic' ? '◉ AUTO' : '○ MANUAL'}
</button>
```

**Toggle auto_fix_ci:**

```tsx
<button
  onClick={() => toggleAutoFix(pr.pr_number, pr.auto_fix_ci)}
  className={`text-[11px] font-mono px-2 py-1 rounded border transition-all ${
    pr.auto_fix_ci
      ? 'border-yellow-400/50 text-yellow-400 bg-yellow-400/10'
      : 'border-white/10 text-white/20'
  }`}
>
  🔧 Auto-fix {pr.auto_fix_ci ? 'ON' : 'OFF'}
</button>
```

**Botón "Mergear ahora":**

Solo visible cuando `status === 'MergeReady'` (CI pasó, modo Manual):

```tsx
{pr.status === 'MergeReady' && (
  <button
    onClick={() => mergePr(pr.pr_number)}
    className="text-[11px] font-mono px-3 py-1 rounded bg-green-500/20 border border-green-500/50 text-green-400 hover:bg-green-500/30"
  >
    Mergear ahora →
  </button>
)}
```

**Fetch inicial:**

Al montar, fetch a `GET /api/prs` y cargar en `usePrStore.setPrs()`. Polling cada 30 segundos.

**Funciones de acción:**

```typescript
const toggleMergeMode = async (prNumber: number, current: string) => {
  const newMode = current === 'automatic' ? 'manual' : 'automatic';
  await fetch(`/api/prs/${prNumber}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json', 'x-citadel-tenant': tenantId!, 'x-citadel-key': sessionKey! },
    body: JSON.stringify({ merge_mode: newMode }),
  });
  usePrStore.getState().updatePrSettings(prNumber, { merge_mode: newMode });
};

const toggleAutoFix = async (prNumber: number, current: boolean) => {
  await fetch(`/api/prs/${prNumber}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json', 'x-citadel-tenant': tenantId!, 'x-citadel-key': sessionKey! },
    body: JSON.stringify({ auto_fix_ci: !current }),
  });
  usePrStore.getState().updatePrSettings(prNumber, { auto_fix_ci: !current });
};

const mergePr = async (prNumber: number) => {
  await fetch(`/api/prs/${prNumber}/merge`, {
    method: 'POST',
    headers: { 'x-citadel-tenant': tenantId!, 'x-citadel-key': sessionKey! },
  });
  // El backend enviará pr_merged via WebSocket — el store se actualiza por ahí
};
```

### 3. Integración en `Dashboard.tsx`

```tsx
import PRManagerPanel from './PRManagerPanel';

{/* PR Manager — Epic 44 */}
<div className="col-span-12 space-y-6">
  <div className="flex items-center gap-3">
    <GitPullRequest className="w-5 h-5 text-aegis-cyan" />
    <h2 className="text-xl font-bold uppercase tracking-widest">Pull Requests</h2>
  </div>
  <PRManagerPanel tenantId={tenantId} sessionKey={sessionKey} />
</div>
```

Agregar `GitPullRequest` al import de `lucide-react`.

---

## Criterios de Aceptación

- [x] La lista de PRs se carga al montar y hace polling cada 30 segundos
- [x] El toggle de `merge_mode` hace el PATCH y actualiza el UI optimistamente
- [x] El toggle de `auto_fix_ci` hace el PATCH y actualiza el UI optimistamente
- [x] El botón "Mergear ahora" solo aparece para PRs con `status === 'MergeReady'`
- [x] Al clickear "Mergear ahora", el botón se deshabilita mientras espera la respuesta
- [x] El PR pasa a `Merged` cuando llega el evento `pr_merged` del WebSocket (via `usePrStore`)
- [x] Cuando no hay PRs, se muestra el estado vacío con mensaje explicativo
- [x] `npm run build` pasa sin errores TypeScript

---

*Arquitecto IA — 2026-04-24*
# CORE-178 — feat: GitTimeline — Branches, Commits y Estado de CI en Dashboard

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / shell/ui  
**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Alta  
**Asignado a:** Shell Engineer  
**Dependencias:** CORE-173 (endpoint /api/prs), CORE-171 (datos de branches/commits via nuevo endpoint)

---

## Contexto

El tenant necesita ver el estado del repositorio: qué branches existen, cuáles son los últimos commits, qué PRs están abiertos y cuál es el estado de su CI. El GitTimeline es el widget que muestra todo eso de forma visual en el Dashboard.

---

## Cambios Requeridos

### Nuevo endpoint backend (parte de este ticket)

El Shell Engineer necesita que el Kernel Engineer agregue:

```
GET /api/git/status
    → Retorna branches recientes + commits + PRs activos
    → Requiere: Citadel auth
    → Retorna: GitStatusDto
```

```typescript
// DTOs para el frontend
interface CommitDto {
  sha: string;
  shortSha: string;   // primeros 7 caracteres
  message: string;
  authorName: string;
  date: string;
  isAegis: boolean;   // true si author_email === "bot@aegis-os.dev"
}

interface BranchDto {
  name: string;
  shortSha: string;
  isRemote: boolean;
  lastCommit?: CommitDto;
}

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

interface GitStatusDto {
  branches: BranchDto[];
  recent_commits: CommitDto[];   // últimos 10 commits de la branch actual
  active_prs: PrDto[];
  current_branch: string;
}
```

**Nota para el Kernel Engineer:** Si este endpoint no existe cuando el Shell Engineer implementa CORE-178, usar mock data (igual que en CORE-166).

### Nuevo archivo: `src/components/GitTimeline.tsx`

**Layout:**

```
┌─ GIT ────────────── main [a1b2c3d] ──── [↻ Actualizar] ───────┐
│                                                                │
│  BRANCHES                    PRs ACTIVOS                      │
│  ─────────                   ──────────                       │
│  ● main          (actual)    #42 feat: Epic 43 [CI ✅] [AUTO] │
│  ○ feat/epic-43  a1b2c3d     #41 fix: auth bug [CI 🔄] [MAN]  │
│  ○ fix/auth-bug  d4e5f6g                                      │
│                                                               │
│  COMMITS RECIENTES (main)                                     │
│  ─────────────────────────                                    │
│  🤖 a1b2c3d  feat(ank-core): CORE-155 AgentNode tipos base    │
│              Aegis OS · hace 5 min                            │
│  👤 b2c3d4e  chore: update dependencies                       │
│              Tavo · hace 2 horas                              │
│  🤖 c3d4e5f  feat(ank-http): CORE-163 routes /api/agents      │
│              Aegis OS · hace 3 horas                          │
└───────────────────────────────────────────────────────────────┘
```

**Indicadores de estado de CI en PRs:**

```typescript
const CI_STATUS: Record<string, { icon: string; color: string; label: string }> = {
  CiRunning:         { icon: '🔄', color: 'text-yellow-400', label: 'CI corriendo' },
  CiPassed:          { icon: '✅', color: 'text-green-400',  label: 'CI pasó' },
  CiFailed:          { icon: '❌', color: 'text-red-400',    label: 'CI falló' },
  MergeReady:        { icon: '🚀', color: 'text-green-400',  label: 'Listo para merge' },
  AutoFixInProgress: { icon: '🔧', color: 'text-yellow-400', label: 'Auto-fix en progreso' },
  Merged:            { icon: '✅', color: 'text-purple-400', label: 'Mergeado' },
  Open:              { icon: '○',  color: 'text-white/50',   label: 'Abierto' },
};
```

**Badge de modo merge:**

```tsx
// En cada PR de la lista
<span className={`text-[10px] font-mono px-2 py-0.5 rounded ${
  pr.merge_mode === 'automatic' ? 'bg-aegis-cyan/20 text-aegis-cyan' : 'bg-white/10 text-white/50'
}`}>
  {pr.merge_mode === 'automatic' ? 'AUTO' : 'MAN'}
</span>
```

**Polling:** El componente refresca los datos cada 30 segundos (alineado con el polling del PR Manager en el backend).

**El botón "↻ Actualizar"** fuerza un refresh inmediato.

### Integración en `Dashboard.tsx`

```tsx
import GitTimeline from './GitTimeline';

{/* Git Timeline — Epic 44 */}
<div className="col-span-12 space-y-6">
  <div className="flex items-center gap-3">
    <GitBranch className="w-5 h-5 text-aegis-cyan" />
    <h2 className="text-xl font-bold uppercase tracking-widest">Repositorio</h2>
  </div>
  <GitTimeline tenantId={tenantId} sessionKey={sessionKey} />
</div>
```

Agregar `GitBranch` al import de `lucide-react`.

---

## Criterios de Aceptación

- [ ] El widget muestra la lista de branches con el nombre y SHA corto
- [ ] La branch actual está marcada con `●` y las demás con `○`
- [ ] Los PRs activos muestran el estado de CI con el icono correcto
- [ ] Los PRs muestran el badge `AUTO` o `MAN` según su `merge_mode`
- [ ] Los commits de Aegis OS se distinguen con el icono 🤖
- [ ] El botón "Actualizar" hace un fetch inmediato sin esperar el polling
- [ ] Si no hay GitHub configurado, mostrar "Configurá el repositorio en Workspace Settings"
- [ ] El polling se limpia al desmontar el componente
- [ ] `npm run build` pasa sin errores TypeScript

---

*Arquitecto IA — 2026-04-24*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: components/workspace/GitTimeline.tsx existe.

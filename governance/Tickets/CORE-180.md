# CORE-180 — feat: WorkspaceSettings — Configuración de Token, Repo y Opciones

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / shell/ui  
**Tipo:** feat  
**Prioridad:** Alta  
**Asignado a:** Shell Engineer  
**Dependencias:** CORE-167 (endpoints /api/workspace/config)
**Estado:** ✅ Completo

---

## Contexto

El tenant necesita un lugar en el Dashboard para configurar su workspace antes de poder usar terminal, Git y code viewer. El WorkspaceSettings es un panel de configuración dentro del Dashboard — no una página separada.

---

## Cambios Requeridos

### Nuevo archivo: `src/components/WorkspaceSettings.tsx`

**Layout:**

```
┌─ WORKSPACE ──────────────────────────────────────────────────────┐
│                                                                  │
│  GitHub                                                          │
│  ───────                                                         │
│  Token        [ ••••••••••••••••••••  ] [Cambiar]  ✅ Configurado│
│  Repositorio  [ Gustavo324234/Aegis-Core          ] [Guardar]   │
│                                                                  │
│  Proyecto                                                        │
│  ─────────                                                       │
│  Directorio   [ /home/tavo/Aegis-Core             ] [Guardar]   │
│                                                                  │
│  Terminal                                                        │
│  ────────                                                        │
│  Comandos     [cargo] [npm] [git] [python] [+] ← tags editables │
│  permitidos                                                      │
│                                                                  │
│  Pull Requests                                                   │
│  ─────────────                                                   │
│  Modo default    ( ) Automático  (●) Manual                     │
│  Auto-fix CI     [✓] Activado                                   │
│                                                                  │
│  [  Guardar configuración de PR  ]                              │
└──────────────────────────────────────────────────────────────────┘
```

**Props:**

```typescript
interface WorkspaceSettingsProps {
  tenantId: string | null;
  sessionKey: string | null;
}
```

**Estado interno:**

```typescript
interface WorkspaceFormState {
  githubToken: string;       // vacío = no cambiar; "••••" = ya configurado
  githubTokenConfigured: boolean;
  githubRepo: string;
  projectRoot: string;
  terminalAllowlist: string[];
  prMergeMode: 'automatic' | 'manual';
  prAutoFixCi: boolean;
}
```

**Fetch inicial:**

Al montar, `GET /api/workspace/config` y poblar el formulario. El token llega como `"configured"` (string) o `null` — nunca el valor real.

**Guardar token:**

El campo de token muestra `"••••••••••••" (configurado)` cuando `githubTokenConfigured === true`. Al clickear "Cambiar", se limpia y muestra un input de texto para ingresar el nuevo token. Al guardar, `POST /api/workspace/config` con `{ key: "github_token", value: newToken }`.

**Tags de terminal allowlist:**

```tsx
// Cada comando en un badge con botón X para eliminar
// + botón para agregar nuevo comando (input inline)
{allowlist.map(cmd => (
  <span key={cmd} className="flex items-center gap-1 text-[11px] font-mono px-2 py-1 bg-white/10 rounded border border-white/20">
    {cmd}
    <button onClick={() => removeCommand(cmd)} className="text-white/40 hover:text-red-400">×</button>
  </span>
))}
```

**Guardar cada campo individualmente:**

Cada sección tiene su propio botón "Guardar" que hace `POST /api/workspace/config` con la key y el valor correspondiente. Muestra feedback visual (✅ / ❌) después del POST.

**Guardado de PR defaults:**

```typescript
const savePrDefaults = async () => {
  await Promise.all([
    fetch('/api/workspace/config', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'x-citadel-tenant': tenantId!, 'x-citadel-key': sessionKey! },
      body: JSON.stringify({ key: 'pr_merge_mode', value: prMergeMode }),
    }),
    fetch('/api/workspace/config', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'x-citadel-tenant': tenantId!, 'x-citadel-key': sessionKey! },
      body: JSON.stringify({ key: 'pr_auto_fix_ci', value: String(prAutoFixCi) }),
    }),
  ]);
};
```

### Integración en `Dashboard.tsx`

Este panel va **al final del Dashboard**, como última sección antes del cierre del grid:

```tsx
import WorkspaceSettings from './WorkspaceSettings';

{/* Workspace Settings — Epic 44 */}
<div className="col-span-12 space-y-6">
  <div className="flex items-center gap-3">
    <Settings className="w-5 h-5 text-aegis-cyan" />
    <h2 className="text-xl font-bold uppercase tracking-widest">Workspace</h2>
  </div>
  <WorkspaceSettings tenantId={tenantId} sessionKey={sessionKey} />
</div>
```

Agregar `Settings` al import de `lucide-react` (probablemente ya está).

---

## Criterios de Aceptación

- [x] Al montar, el componente carga la configuración actual via `GET /api/workspace/config`
- [x] El campo de GitHub token nunca muestra el valor real — solo `"•••• (configurado)"` o vacío
- [x] El campo "Cambiar token" muestra el input solo después de hacer click en el botón "Cambiar"
- [x] Los tags de la allowlist de terminal se pueden agregar y eliminar
- [x] Cada sección guarda de forma independiente con feedback visual (✅ guardado / ❌ error)
- [x] Los radio buttons de `merge_mode` funcionan correctamente
- [x] El checkbox de `auto_fix_ci` funciona correctamente
- [x] `npm run build` pasa sin errores TypeScript

---

*Arquitecto IA — 2026-04-24*
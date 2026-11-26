# CORE-252 — Dashboard: header con nombre real del tenant y estado del sistema real

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** EPIC 48 — Shell Observability  
**Asignado a:** Shell Engineer  
**Componente:** `shell/ui/src/components/Dashboard.tsx` — header y welcome section

**Estado:** ✅ Done
---

## Contexto

El dashboard muestra dos strings hardcodeados que nunca reflejan datos reales:

1. `"Welcome back, Operator"` — siempre dice "Operator" sin importar el tenant
2. `"All enclaves secured"` — texto de marketing sin relación con el estado real

El store ya tiene `tenantId`, `systemState`, y `system_metrics` con datos reales. El nombre del tenant está en `GET /api/persona` (campo `name`).

---

## Cambios requeridos

### 1. Saludo personalizado

Al montar el Dashboard, hacer `GET /api/persona` para obtener el nombre configurado:

```typescript
// En Dashboard.tsx, al montar:
const [tenantName, setTenantName] = useState<string | null>(null);

useEffect(() => {
  if (!tenantId || !sessionKey) return;
  fetch('/api/persona', {
    headers: { 'x-citadel-tenant': tenantId, 'x-citadel-key': sessionKey }
  })
    .then(r => r.ok ? r.json() : null)
    .then(data => setTenantName(data?.name || tenantId))
    .catch(() => setTenantName(tenantId));
}, [tenantId, sessionKey]);
```

Reemplazar:
```tsx
// Antes:
<h2>Welcome back, <span className="text-aegis-cyan">Operator</span></h2>

// Después:
<h2>Welcome back, <span className="text-aegis-cyan">{tenantName ?? tenantId ?? 'Operator'}</span></h2>
```

### 2. Estado del sistema real

Reemplazar el texto hardcodeado con un mapeo desde `systemState` del store:

```typescript
const { systemState } = useAegisStore();

const systemStatusText = {
  'STATE_OPERATIONAL': 'Kernel Operational // All systems nominal',
  'STATE_INITIALIZING': 'Kernel Initializing // Please wait',
  'UNKNOWN': 'System Status Unknown',
}[systemState] ?? 'Checking system status...';

const systemStatusColor = systemState === 'STATE_OPERATIONAL' ? 'text-white/40' 
  : systemState === 'STATE_INITIALIZING' ? 'text-yellow-500/60' 
  : 'text-red-500/60';
```

Reemplazar:
```tsx
// Antes:
<p className="text-white/40 font-mono text-xs uppercase tracking-widest">
  System status: Optimal // All enclaves secured
</p>

// Después:
<p className={`font-mono text-xs uppercase tracking-widest ${systemStatusColor}`}>
  {systemStatusText}
</p>
```

### 3. CPU Load con valor numérico en el header

El header ya tiene una barra de CPU animada. Agregar el porcentaje numérico:

```tsx
// Antes:
<span className="text-[8px] font-mono text-white/20 uppercase tracking-widest">CPU LOAD</span>
<div className="w-24 h-1 bg-white/5 rounded-full overflow-hidden mt-1">
  <motion.div animate={{ width: `${system_metrics.cpu_load}%` }} />
</div>

// Después:
<div className="flex items-center justify-between gap-2">
  <span className="text-[8px] font-mono text-white/20 uppercase tracking-widest">CPU</span>
  <span className="text-[9px] font-mono text-white/40">{system_metrics.cpu_load.toFixed(0)}%</span>
</div>
<div className="w-24 h-1 bg-white/5 rounded-full overflow-hidden mt-1">
  <motion.div animate={{ width: `${system_metrics.cpu_load}%` }} />
</div>
```

### 4. Dot de estado "Kernel Operational" en el header

El dot verde con "Kernel Operational" también es hardcodeado. Hacerlo dinámico:

```tsx
const isOperational = systemState === 'STATE_OPERATIONAL';

<div className="flex items-center gap-3">
  <div className={`w-2 h-2 rounded-full ${isOperational ? 'bg-green-500 animate-pulse' : 'bg-yellow-500'}`} />
  <span className="text-[10px] font-mono text-white/40 uppercase">
    {isOperational ? 'Kernel Operational' : 'Kernel Initializing'}
  </span>
</div>
```

---

## Criterios de aceptación

- [x] El saludo muestra el nombre real del tenant (de `GET /api/persona`) o `tenantId` como fallback.
- [x] El texto de estado del sistema refleja `systemState` del store (no hardcodeado).
- [x] La barra de CPU muestra el porcentaje numérico además del visual.
- [x] El dot de estado del header refleja `systemState` (no siempre verde).
- [x] TypeScript sin errores de compilación.

## Dependencias

- `systemState` en el store (ya disponible via `fetchSystemState`)
- `GET /api/persona` (ya existe)

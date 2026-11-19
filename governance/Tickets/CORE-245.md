# CORE-245 — Admin: toggle habilitar/deshabilitar provider sin eliminarlo

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** UX & Onboarding (EPIC 41)  
**Asignado a:** Shell Engineer  
**Componente:** `shell/ui/src/components/ProvidersTab.tsx`

**Estado:** ✅ Done
---

## Contexto

El admin puede agregar y eliminar providers en el panel IA Tools, pero no puede deshabilitarlos temporalmente sin perder la key configurada. Si un provider tiene problemas (rate limit sostenido, mantenimiento, etc.), la única opción actual es borrarlo y volver a configurarlo después.

Se necesita un toggle ON/OFF por provider que persista `is_active` en el backend sin tocar la key.

## Cambios requeridos

### `shell/ui/src/components/ProvidersTab.tsx`

**En `ProviderCard`:** reemplazar el badge de estado estático por un toggle interactivo.

El toggle debe:
- Mostrar el estado actual (active / inactive) de forma visual clara.
- Al hacer click, llamar `PUT /api/router/keys/global/{key_id}` con `{ is_active: !provider.is_active }` en el body.
- Mostrar un spinner mientras la request está en vuelo.
- Actualizar el estado local optimísticamente (o refrescar la lista tras éxito).
- Si el provider está `rate_limited_until` (fecha futura), mostrar ese estado encima del toggle sin bloquearlo — el admin puede reactivarlo igualmente.

Estructura del toggle dentro de `ProviderCard`:

```tsx
// Reemplaza el badge estático de estado
<button
  onClick={() => onToggle(provider.key_id, !provider.is_active)}
  disabled={isToggling}
  className={`relative w-11 h-6 rounded-full transition-colors duration-300
    ${provider.is_active ? 'bg-green-500/40' : 'bg-white/10'}
    border ${provider.is_active ? 'border-green-500/50' : 'border-white/20'}
    hover:opacity-80 disabled:opacity-50`}
>
  <div className={`absolute top-0.5 w-5 h-5 rounded-full transition-all duration-300
    ${provider.is_active ? 'left-5 bg-green-400' : 'left-0.5 bg-white/30'}`}
  />
</button>
```

**En `ProvidersTab`:** agregar handler `handleToggle`:

```tsx
const handleToggle = async (keyId: string, newActive: boolean) => {
  // Actualizar local optimísticamente
  setProviders(prev => prev.map(p =>
    p.key_id === keyId ? { ...p, is_active: newActive } : p
  ));
  try {
    const res = await fetch(`/api/router/keys/global/${keyId}`, {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
        'x-citadel-tenant': tenantId!,
        'x-citadel-key': sessionKey!,
      },
      body: JSON.stringify({ is_active: newActive }),
    });
    if (!res.ok) {
      // Revertir si falla
      setProviders(prev => prev.map(p =>
        p.key_id === keyId ? { ...p, is_active: !newActive } : p
      ));
    }
  } catch {
    setProviders(prev => prev.map(p =>
      p.key_id === keyId ? { ...p, is_active: !newActive } : p
    ));
  }
};
```

Pasar `onToggle={handleToggle}` a `ProviderCard` y agregar prop `onToggle: (keyId: string, active: boolean) => void` en la interfaz del componente.

**Verificar** que el endpoint `PUT /api/router/keys/global/{key_id}` en el backend ya acepta `is_active` en el body. Si no, registrar como sub-tarea de kernel (no bloquear el ticket Shell).

## Criterios de aceptación

- [x] Cada `ProviderCard` muestra un toggle ON/OFF en lugar del badge de texto estático.
- [x] Hacer click en el toggle actualiza `is_active` via `PUT /api/router/keys/global/{key_id}`.
- [x] El cambio se refleja visualmente de forma inmediata (optimistic update).
- [x] Si la request falla, el toggle revierte al estado anterior.
- [x] Un provider con `rate_limited_until` futuro muestra ese badge adicional, pero el toggle sigue siendo operable.
- [x] `cargo build` / TypeScript sin errores de compilación.

## Dependencias

- Backend: `PUT /api/router/keys/global/{key_id}` debe aceptar `is_active` en el body (verificar que ya está implementado en `ank-http/src/routes/`).

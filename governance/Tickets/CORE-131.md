# CORE-131 — Feature: Soporte de Persona en Aegis App (modo Satélite)

**Epic:** 38 — Agent Persona System
**Repo:** Aegis-Core — `app/`
**Tipo:** feat
**Prioridad:** Media
**Asignado a:** Shell Engineer (Antigravity)
**Depende de:** CORE-129

---

## Contexto

La app mobile en modo Satélite conecta al mismo `ank-http` que la Shell web.
Cuando hay una Persona configurada, el modelo ya la usa — esto ocurre en el
backend y la app no necesita hacer nada.

Este ticket cubre dos cosas de bajo costo y alto valor:

1. **Mostrar en Settings si hay Persona activa** — el usuario sabe que el agente
   tiene una identidad configurada por el operador.
2. **Eliminar el "quién eres" hardcodeado** en `cloudRouter.ts` — en modo Cloud
   directo la app envía una descripción genérica hardcodeada al provider. Debe
   usar el mismo sistema o al menos no mentir sobre capacidades.

---

## Cambios requeridos

### 1. `app/src/services/bffClient.ts` — nuevo método `fetchPersona`

```typescript
export async function fetchPersona(
  serverUrl: string,
  tenantId: string,
  sessionKey: string
): Promise<{ persona: string; is_configured: boolean }> {
  const res = await fetch(`${serverUrl}/api/persona`, {
    headers: {
      'x-citadel-tenant': tenantId,
      'x-citadel-key': sessionKey,
    },
  });
  if (!res.ok) return { persona: '', is_configured: false };
  return res.json();
}
```

### 2. `app/src/stores/settingsStore.ts` — campo `agentPersona`

```typescript
interface SettingsState {
  // ... campos existentes ...
  agentPersona: string;
  isPersonaConfigured: boolean;
  fetchAgentPersona: () => Promise<void>;
}

// En el store Zustand, agregar:
agentPersona: '',
isPersonaConfigured: false,

fetchAgentPersona: async () => {
  const { serverUrl, tenantId, sessionKey } = useAuthStore.getState();
  if (!serverUrl || !tenantId || !sessionKey) return;
  try {
    const result = await fetchPersona(serverUrl, tenantId, sessionKey);
    set({ agentPersona: result.persona, isPersonaConfigured: result.is_configured });
  } catch {
    // best-effort — no crashear si falla
  }
},
```

### 3. `app/app/(main)/settings.tsx` — sección "Identidad del Agente"

En la pantalla de Settings, agregar una sección readonly que muestre el estado
de la Persona cuando el modo es Satélite:

```tsx
// Llamar fetchAgentPersona al montar (solo en modo Satélite)
useEffect(() => {
  if (mode === 'satellite') {
    settingsStore.fetchAgentPersona();
  }
}, [mode]);

// En el render, después de la sección de configuración de servidor:
{mode === 'satellite' && (
  <View style={styles.section}>
    <Text style={styles.sectionTitle}>Identidad del Agente</Text>
    {isPersonaConfigured ? (
      <View style={styles.personaBadge}>
        <Text style={styles.personaBadgeText}>✓ Persona configurada por el operador</Text>
        <Text style={styles.personaPreview} numberOfLines={2}>
          {agentPersona.slice(0, 120)}{agentPersona.length > 120 ? '...' : ''}
        </Text>
      </View>
    ) : (
      <Text style={styles.personaEmpty}>Sin identidad personalizada — usando Aegis por defecto</Text>
    )}
  </View>
)}
```

### 4. `app/src/services/cloudRouter.ts` — eliminar system prompt hardcodeado

Localizar cualquier string hardcodeado del tipo `"Eres un asistente..."` o
`"You are a helpful assistant..."` en las llamadas a providers cloud y reemplazar
por el valor del store (si está disponible) o por una cadena vacía para que el
provider use su default.

---

## Criterios de aceptación

- [ ] En modo Satélite, `fetchPersona` se llama al abrir Settings
- [ ] Si hay persona: badge "✓ Persona configurada" visible con preview truncado
- [ ] Si no hay persona: texto "Sin identidad personalizada — usando Aegis por defecto"
- [ ] En modo Cloud: no se envía un system prompt hardcodeado inventado
- [ ] La app no crashea si `/api/persona` falla (best-effort)
- [ ] `npm run` (expo) sin errores TypeScript

---

## Dependencias

- CORE-129 (endpoint `/api/persona` operativo)

---

## Commit message

```
feat(app): CORE-131 persona display in settings — satellite mode awareness
```

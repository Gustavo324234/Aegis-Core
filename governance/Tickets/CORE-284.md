# CORE-284 — Shell: botón de reply al supervisor — fix envío

**Tipo:** fix  
**Prioridad:** Alta  
**Asignado a:** Shell Engineer  
**Componentes:**
- `shell/ui/src/components/AgentThread.tsx`
- `shell/ui/src/store/agentInboxStore.ts`
**Estado:** ✅ Completo

---

## Contexto

El usuario puede ver los mensajes del supervisor y escribir en el hilo dedicado
(CORE-270 funciona), pero al presionar enviar no pasa nada.

**Causa probable:** el componente `AgentThread` tiene el envío mockeado con
`console.log` como indicaba el briefing de la Sesión 3, esperando que CORE-271
(el endpoint kernel) estuviera mergeado. Si CORE-271 está mergeado, el fix es
conectar el fetch real. Si no está mergeado, el fix es implementar ambos juntos.

Este ticket asume que CORE-271 (endpoint `POST /api/agents/:agent_id/reply`)
está implementado en el kernel. Si no lo está, resolver CORE-271 primero.

---

## Diagnóstico antes de implementar

Antes de escribir código, verificar en `AgentThread.tsx`:

1. ¿El botón de envío tiene un handler `onClick` o `onSubmit`?
2. ¿Ese handler llama a `fetch('/api/agents/...')` o tiene un `console.log`?
3. ¿Las credenciales (`tenantId`, `sessionKey`) están disponibles en el componente?

Si el handler ya llama al fetch pero falla, verificar:
- La URL que construye (¿incluye el `agentId` correcto?)
- Los headers de autenticación (¿usa `x-citadel-tenant` y `x-citadel-key`?)
- Si hay un error de CORS o 404 en la consola del browser

---

## Cambios requeridos

### `AgentThread.tsx` — implementar envío real

```typescript
import { useAegisStore } from '../store/useAegisStore';
import { useAgentInboxStore } from '../store/agentInboxStore';

// En el componente AgentThread:
const { tenantId, sessionKey } = useAegisStore(s => ({
    tenantId: s.tenantId,
    sessionKey: s.sessionKey,
}));

const handleSend = async (answer: string) => {
    if (!answer.trim() || !tenantId || !sessionKey) return;

    setIsSending(true);
    try {
        const res = await fetch(`/api/agents/${agentId}/reply`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
                'x-citadel-tenant': tenantId,
                'x-citadel-key': sessionKey,
            },
            body: JSON.stringify({ answer: answer.trim() }),
        });

        if (res.ok) {
            // Actualizar el store
            useAgentInboxStore.getState().addThreadMessage(agentId, {
                role: 'user',
                content: answer.trim(),
                timestamp: new Date().toISOString(),
            });
            useAgentInboxStore.getState().markAnswered(agentId);
            setInputValue('');
        } else if (res.status === 404) {
            // El supervisor ya no espera — informar al usuario
            // Mostrar un mensaje inline, no un alert
            setErrorMessage('El supervisor ya no está esperando una respuesta.');
        } else {
            setErrorMessage('No se pudo enviar la respuesta. Intentá de nuevo.');
        }
    } catch {
        setErrorMessage('Error de conexión.');
    } finally {
        setIsSending(false);
    }
};
```

El input debe estar deshabilitado mientras `isSending === true`.
El botón de envío muestra un spinner o está deshabilitado durante el envío.

### Estado del input

Agregar estado local al componente:
```typescript
const [inputValue, setInputValue] = useState('');
const [isSending, setIsSending] = useState(false);
const [errorMessage, setErrorMessage] = useState<string | null>(null);
```

El input solo está habilitado cuando `status === 'pending'` Y `!isSending`.

---

## Criterios de aceptación

- [x] Al escribir en el hilo y presionar enviar, el mensaje se entrega al supervisor
- [x] El mensaje del usuario aparece en el hilo inmediatamente (optimistic update)
- [x] El badge del inbox se actualiza a 0 al responder
- [x] Si el supervisor ya no espera (404), aparece un mensaje explicativo en el hilo
- [x] El input se deshabilita durante el envío y se re-habilita si falla
- [x] No hay `console.log` reemplazando el fetch real

## Dependencias

- CORE-271 ✅ debe estar mergeado (endpoint kernel)
- CORE-270 ✅ (componente AgentThread existe)

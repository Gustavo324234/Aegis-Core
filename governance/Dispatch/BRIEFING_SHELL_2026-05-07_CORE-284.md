# BRIEFING — Shell: fix botón reply supervisor

**Fecha:** 2026-05-07  
**Para:** Shell Engineer (Antigravity)  
**Tickets:** CORE-284

---

## Branch

```
fix/core-284-agent-reply-button
```

---

## Prerequisito

Verificar que CORE-271 esté mergeado — el endpoint `POST /api/agents/:agent_id/reply`
debe existir en el kernel. Si no está mergeado, resolver eso primero.

Leer el ticket antes de implementar:
- `governance/Tickets/CORE-284.md`

---

## Objetivo

### CORE-284 — Fix botón de envío en AgentThread

**Diagnóstico previo (hacer antes de escribir código):**

Abrir `shell/ui/src/components/AgentThread.tsx` y verificar:
1. ¿El handler de envío tiene `console.log` o un `fetch` real?
2. ¿Los headers incluyen `x-citadel-tenant` y `x-citadel-key`?
3. Si hay fetch, ¿qué URL construye?

**Implementación:**

Reemplazar el handler de envío por la implementación real que llama a
`POST /api/agents/:agentId/reply` con headers Citadel.

Al recibir 200:
- Agregar el mensaje del usuario al thread en el store (`addThreadMessage`)
- Llamar `markAnswered(agentId)`
- Limpiar el input

Al recibir 404: mostrar mensaje inline "El supervisor ya no está esperando."

Al recibir error de red: mostrar "Error de conexión."

El input se deshabilita durante el envío (`isSending`).

Ver código completo en `governance/Tickets/CORE-284.md`.

**Obtener credenciales:**
```typescript
const { tenantId, sessionKey } = useAegisStore(s => ({
    tenantId: s.tenantId,
    sessionKey: s.sessionKey,
}));
```

---

## Verificación

```bash
npm run build
npm run lint
```

Probar manualmente:
1. Activar un proyecto para que el supervisor haga una pregunta
2. Abrir el hilo dedicado
3. Escribir una respuesta y presionar enviar
4. Verificar que el badge desaparece y el hilo muestra el mensaje del usuario

---

## Commit y PR

**Commit message:**
```
fix(shell): CORE-284 botón de reply al supervisor conectado al endpoint real
```

**PR title:**
```
fix(shell): CORE-284 — reply al supervisor funciona (fetch real, no console.log)
```

**PR description:**
```
## CORE-284 — Fix botón de envío en AgentThread

### Problema
El usuario podía ver el mensaje del supervisor y escribir una respuesta,
pero al presionar enviar no pasaba nada (console.log mock).

### Fix
- Handler de envío reemplazado por fetch real a POST /api/agents/:agentId/reply
- Headers Citadel correctos (x-citadel-tenant, x-citadel-key)
- Estado de carga (isSending) deshabilita el input durante el envío
- 200: agrega mensaje al thread, marca como answered, limpia input
- 404: muestra "El supervisor ya no está esperando" inline
- Error de red: muestra mensaje de error inline

## Verificación
npm run build ✅  npm run lint ✅
```

**Target branch:** `main`

---

*Briefing creado por Arquitecto IA — 2026-05-07*

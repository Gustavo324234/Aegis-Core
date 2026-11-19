# CORE-248 — Chat: indicador de estado enriquecido (modelo, provider, error amigable)

**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Crítica  
**Épica:** EPIC 48 — Shell Observability  
**Asignado a:** Shell Engineer  
**Componentes:** `shell/ui/src/components/ChatTerminal.tsx`, `shell/ui/src/store/useAegisStore.ts`

---

## Contexto

El chat actualmente muestra un dot de estado mínimo y ningún feedback sobre qué está ocurriendo internamente. Cuando hay un error, el usuario ve silencio total (relacionado con CORE-244). Incluso cuando el modelo responde bien, el usuario no sabe qué modelo se está usando ni si hay demora.

El store ya tiene `lastRoutingInfo: { model_id, provider, latency_ms }` que nunca se renderiza.

---

## Cambios requeridos

### 1. Banner de estado durante inferencia — `ChatTerminal.tsx`

Reemplazar el spinner estático `"ANK is processing payload..."` por un banner más informativo que use `lastRoutingInfo`:

```tsx
{status === 'thinking' && (
  <motion.div className="flex items-center gap-3 px-6 py-3 bg-white/5 border-t border-white/5 text-white/50 text-xs font-mono">
    <Loader2 className="w-3.5 h-3.5 animate-spin text-aegis-cyan" />
    <span>
      {lastRoutingInfo
        ? `${lastRoutingInfo.provider} / ${lastRoutingInfo.model_id}`
        : 'Procesando...'}
    </span>
    {lastRoutingInfo?.latency_ms && (
      <span className="text-white/20 ml-auto">{lastRoutingInfo.latency_ms}ms</span>
    )}
  </motion.div>
)}
```

Este banner aparece debajo del área de mensajes, encima del input. Es compacto — no interrumpe el flujo de lectura.

### 2. Mensaje de error amigable en el hilo del chat — `useAegisStore.ts`

En el handler del WebSocket, cuando `event === 'error'` o cuando llega un `kernel_event` con payload `{ error: ... }`, ademas de setear `status: 'error'`, inyectar un mensaje de assistant en el hilo:

```typescript
case 'error': {
  const errData = data as string;
  set({ status: 'error', lastError: errData || 'Unknown error' });
  
  // Inyectar mensaje amigable en el chat (no técnico)
  if (errData !== 'PASSWORD_MUST_CHANGE' && !errData?.includes('AUTH_FAILURE')) {
    get().addSystemMessage('No pude procesar tu mensaje. Intentá de nuevo en unos segundos.');
  }
  // ... resto del handler existente
  break;
}
```

Y en `kernel_event` con payload `error`:
```typescript
else if (payload.error) {
  // Mensaje amigable al usuario
  get().appendToken(payload.pid as string, 'No pude completar esta acción. Intentá de nuevo.', 'error');
  set({ status: 'error' });
}
```

### 3. Timeout de seguridad de 30s — `ChatTerminal.tsx`

Al enviar un mensaje, iniciar un timeout de 30 segundos. Si al cabo de ese tiempo el estado sigue siendo `'thinking'` y no llegó ninguna respuesta nueva, inyectar el mensaje de error amigable automáticamente y resetear el estado a `'idle'`.

```typescript
// En handleSend(), después de sendMessage():
const timeoutId = setTimeout(() => {
  if (useAegisStore.getState().status === 'thinking') {
    useAegisStore.getState().addSystemMessage(
      'El motor tardó demasiado en responder. Podés intentar de nuevo.'
    );
    useAegisStore.getState().setStatus('idle');
  }
}, 30_000);

// Limpiar el timeout cuando llegue STATE_COMPLETED o STATE_FAILED
// (hacerlo en el handler del WebSocket en el store)
```

Para limpiar el timeout, agregar al store un `clearThinkingTimeout: () => void` o manejar la referencia en el componente con un `useRef`.

### 4. Badge "En cola" — `useAegisStore.ts`

Cuando llega `event: 'status'` con data que contiene "Submitting" o "Task accepted", actualizar el estado a `'thinking'` pero también setear un flag `isQueued: true` que se limpia cuando llega el primer token. El banner puede mostrar "En cola..." durante ese intervalo breve.

Este punto es opcional si complica la implementación — los puntos 1, 2 y 3 son los críticos.

---

## Criterios de aceptación

- [x] Mientras el modelo procesa, se muestra un banner con provider y modelo (o "Procesando..." si no hay routing info).
- [x] Si hay un error del kernel, aparece un mensaje amigable en el hilo del chat — no silencio.
- [x] Si pasan 30s en estado `thinking` sin respuesta, aparece el mensaje de timeout y el estado vuelve a `idle`.
- [x] El `StatusBadge` en el header sigue funcionando igual.
- [x] TypeScript sin errores de compilación.

## Dependencias

- CORE-244 (relacionado — este ticket es el safety net mientras CORE-244 no esté deployado)

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: Implementado: banner con lastRoutingInfo (ChatTerminal.tsx:358+), mensajes amigables vía addSystemMessage, timeout de seguridad (120s) en ChatTerminal.tsx:190; los criterios del ticket ya estaban tildados.

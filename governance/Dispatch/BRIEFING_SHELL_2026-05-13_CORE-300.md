# BRIEFING — Shell Engineer
## CORE-300: selector de modelo en el chat
**Fecha:** 2026-05-13
**Branch:** `feat/core-300-model-selector`

## Contexto

El usuario quiere elegir qué modelo usa en el chat. Hay que agregar un
selector compacto en la barra de input que muestre los modelos disponibles
y mande `model_override` en el payload del WebSocket.

## Endpoint de datos

`GET /api/catalog/models` — retorna lista de modelos del catálogo.
Verificar el formato de respuesta al implementar.

Si falla, el selector muestra solo "Auto (CMR)" y no rompe el chat.

## Cambios

### `shell/ui/src/components/ChatTerminal.tsx`

**1. Estado local del modelo seleccionado:**
```ts
const [selectedModelId, setSelectedModelId] = useState<string | null>(null);
const [availableModels, setAvailableModels] = useState<ModelEntry[]>([]);
```

**2. Fetch de modelos al montar:**
```ts
useEffect(() => {
    if (!tenantId || !sessionKey) return;
    fetch('/api/catalog/models', {
        headers: { 'x-citadel-tenant': tenantId, 'x-citadel-key': sessionKey }
    })
    .then(r => r.ok ? r.json() : null)
    .then(data => { if (data?.models) setAvailableModels(data.models); })
    .catch(() => {}); // silencioso — fallback a "Auto"
}, [tenantId, sessionKey]);
```

**3. Modificar el send para incluir model_override:**
```ts
// En handleSend o donde se llama sendMessage:
const payload: Record<string, unknown> = {
    prompt: finalPrompt,
    task_type: taskType,
};
if (selectedModelId) payload.model_override = selectedModelId;
socket.send(JSON.stringify(payload));
```

O si se usa `store.sendMessage()`, agregar un parámetro opcional:
```ts
sendMessage: (prompt: string, modelOverride?: string) => {
    // ...
    socket.send(JSON.stringify({
        prompt: finalPrompt,
        task_type: get().taskType,
        ...(modelOverride ? { model_override: modelOverride } : {}),
    }));
}
```

**4. UI del selector — junto al input, antes del botón de enviar:**

Dropdown compacto con:
- Opción default: `⚡ Auto` (sin override)
- Modelos agrupados por provider
- `(free)` para modelos con `cost_input_per_mtok === 0`
- Mostrar `display_name` si existe, sino el model_id sin prefijo de provider

## Diseño visual

```
[ ⚡ Auto ▼ ] [ textarea del mensaje ] [ → ]
```

El selector tiene máximo 200px de ancho, texto truncado con ellipsis.
Al abrir, popover con scroll si hay muchos modelos.
Modelo actualmente seleccionado muestra un checkmark.

## Criterios de aceptación

- [ ] Selector visible en la barra de input del chat
- [ ] Modelos agrupados por provider con badge `(free)` donde corresponde
- [ ] Al seleccionar un modelo, el payload WS incluye `model_override`
- [ ] "Auto" no incluye `model_override`
- [ ] Si el fetch falla, solo muestra "Auto" — el chat sigue funcionando
- [ ] `npm run build` pasa

## Dependencias

- CORE-299 debe estar mergeado para que el override tenga efecto en el kernel
- La UI se puede buildear y mergear antes — simplemente no tendrá efecto hasta CORE-299

## Commit
```
feat(ui): CORE-300 add model selector to chat input bar
```

**No pushear a main. Abrir PR.**

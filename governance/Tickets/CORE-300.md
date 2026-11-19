# CORE-300 — Shell: selector de modelo en el chat

## Metadata
- **Epic:** 51 — Model Intelligence
- **Tipo:** feat
- **Prioridad:** Crítica
- **Asignado a:** Shell Engineer
- **Estado:✅ Done

## Contexto

El usuario no puede elegir qué modelo usa para cada conversación. El CMR
elige automáticamente, pero el usuario quiere ver los modelos disponibles
y poder seleccionar uno.

## Diseño

Un selector compacto en la barra de input del chat (junto al botón de enviar).
Muestra los modelos disponibles basados en las keys registradas.

### Flujo

1. Al montar el chat, fetchear `GET /api/catalog/models` → lista de modelos disponibles
2. Mostrar un dropdown/popover compacto en la barra de input
3. El modelo seleccionado se manda como `model_override` en cada mensaje
4. La selección persiste en estado local (no en el store persistido — resetea entre sesiones)
5. Default: `"auto"` = CMR elige (sin `model_override`)

### Endpoint de datos

`GET /api/catalog/models` ya existe y retorna los modelos del catálogo.
Filtrar por modelos cuyo provider tiene al menos una key registrada
(el endpoint ya hace esto — los modelos sin key no aparecen disponibles).

Si el endpoint no existe o falla, el selector muestra solo "Auto (CMR)".

### UI del selector

```
[⚡ Auto (CMR) ▼]  [textarea]  [→ enviar]
```

Al clickear el selector:
```
┌─────────────────────────────────┐
│ 🤖 Auto (CMR)          ✓ activo │
│ ─────────────────────────────── │
│ OpenRouter                      │
│   openrouter/free               │
│   nvidia/nemotron-3... (free)   │
│   minimax/minimax-m2.5 (free)   │
│ Ollama Cloud                    │
│   llama3.3:70b-cloud            │
└─────────────────────────────────┘
```

Mostrar `(free)` para modelos con `cost_input_per_mtok == 0`.
Agrupar por provider.
Mostrar `display_name` si existe, sino el `model_id` sin prefijo.

### Cambios en `useAegisStore` — `sendMessage`

```ts
// Estado nuevo (local al componente, no en el store global)
const [selectedModel, setSelectedModel] = useState<string | null>(null);

// Al enviar:
socket.send(JSON.stringify({
    prompt: finalPrompt,
    task_type: get().taskType,
    ...(selectedModel ? { model_override: selectedModel } : {}),
}));
```

## Archivos a modificar

- `shell/ui/src/components/ChatTerminal.tsx` — agregar selector de modelo
- `shell/ui/src/store/useAegisStore.ts` — si se necesita exponer el catalog en el store

## Criterios de aceptación

- [ ] El selector aparece en la barra de input del chat
- [ ] La lista muestra los modelos disponibles agrupados por provider
- [ ] Al seleccionar un modelo, el próximo mensaje lo usa (`model_override` en el payload WS)
- [ ] La opción "Auto (CMR)" no manda `model_override` — el CMR elige
- [ ] El `lastRoutingInfo` en el badge del chat refleja el modelo real usado
- [ ] Si la lista de modelos falla al fetchear, el selector muestra solo "Auto (CMR)"
- [ ] `npm run build` pasa sin errores

## Dependencias

- CORE-299 (el kernel tiene que aceptar `model_override` antes de que sirva de algo)

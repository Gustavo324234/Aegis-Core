# CORE-294 — Shell: CatalogViewer — columna Benchmark y badge Ollama Cloud

## Metadata
- **Epic:** 53 — Stabilization (Fase 4 — Model Intelligence)
- **Tipo:** feat
- **Prioridad:** Alta
- **Asignado a:** Shell Engineer
- **Estado:✅ Done

## Contexto

El tab Motor del tenant muestra los modelos disponibles por provider pero no
expone ninguna información de rendimiento al usuario. Esto es un problema de
visibilidad: el usuario elige modelos sin saber cuál es mejor para cada tarea.

**El CMR ya usa los scores correctamente** — `CognitiveRouter.compute_score()`
aplica `entry.score_for(task_type)` con peso del 40% en la decisión de routing.
El pipeline de scoring está completo en el kernel. Este ticket es **puramente
visual** — exponer al usuario lo que el router ya sabe internamente.

El flujo completo es:
```
PinchBench → tools/update_models.py → models.yaml → ModelCatalog
                                                          ↓
                                          CognitiveRouter.compute_score() ← ya funciona
                                                          ↓
                                          CatalogViewer (este ticket) ← falta implementar
```

Los `task_scores` ya están en `ModelEntry` y se serializan en la respuesta de
`GET /api/providers/models` via `catalog.all_entries()`. El Shell Engineer tiene
todos los datos disponibles sin cambios en el kernel.

---

## Cambios requeridos en la UI

### `CatalogViewer` (componente existente en `shell/ui/src/components/`)

**1. Columna nueva: "Bench"**

- Valor: score promedio de `task_scores` del modelo (promedio de los 6 campos: 
  `chat`, `coding`, `planning`, `analysis`, `summarization`, `extraction`, redondeado a 1 decimal)
- Escala visual: 1–5 representado como barra de progreso de 5 segmentos con color:
  - 1–2: rojo · 3: amarillo · 4: verde claro · 5: verde brillante

```typescript
// Cálculo del score promedio
const avgScore = (scores: TaskScores): number => {
  const fields = ['chat', 'coding', 'planning', 'analysis', 'summarization', 'extraction'];
  const values = fields.map(f => scores[f] ?? 0);
  return Math.round((values.reduce((a, b) => a + b, 0) / fields.length) * 10) / 10;
};
```

**2. Badges por tipo de modelo**

- `provider === "ollama_cloud"` → chip `☁ Cloud`
- `is_local === true` → chip `⚡ Local`
- Modelos cloud sin `is_local` → sin chip (comportamiento default)

**3. Tooltip en columna Bench**

Al hover sobre el score mostrar:
```
Score calculado desde PinchBench (benchmark de agentes reales)
Actualizado: <last_synced del endpoint>
Usado por el router para selección automática de modelos.
```

---

## Criterios de aceptación

- [ ] La columna "Bench" es visible en el tab Motor del tenant
- [ ] Los modelos sin score (promedio = 0) muestran "—" en lugar de 0
- [ ] La barra de color refleja correctamente el rango 1–5
- [ ] Los modelos `ollama_cloud` muestran el badge `☁ Cloud`
- [ ] Los modelos locales (`is_local: true`) muestran el badge `⚡ Local`
- [ ] El tooltip de Bench es visible en desktop (hover)
- [ ] La UI no rompe si `task_scores` llega vacío o nulo desde el kernel
- [ ] `npm run build` pasa sin errores ni warnings nuevos

## Dependencias

- CORE-291 ✅ (scores actualizados en models.yaml — ya hecho)
- CORE-292 📥 (provider ollama_cloud en kernel — necesario para badges Cloud)
- CORE-293 📥 (modelos ollama_cloud en catálogo — necesario para badges Cloud)

## Nota

CORE-292 y CORE-293 son necesarios solo para los badges `☁ Cloud`. La columna
Bench y los badges `⚡ Local` pueden implementarse independientemente.

# BRIEFING — Shell Engineer
## EPIC 51 / CORE-294: CatalogViewer — columna Benchmark + badge Ollama Cloud
**Fecha:** 2026-05-08
**Branch sugerido:** `feat/core-294-catalog-benchmark-ui`

---

## Contexto

El tab Motor del tenant muestra modelos disponibles. Queremos agregar dos mejoras
visuales que le dan al usuario información objetiva al elegir un modelo:

1. **Columna "Bench"** — score promedio del modelo (derivado de task_scores del kernel)
2. **Badges de origen** — `☁ Cloud` para modelos Ollama Cloud, `⚡ Local` para modelos locales

---

## Datos disponibles

El endpoint que ya existe (`GET /api/providers/models` o similar) retorna los
`ModelEntry` del catálogo. Verificar que el campo `task_scores` esté incluido
en la respuesta JSON. Si no está, es un ajuste mínimo al serializer del kernel
(coordinar directamente, sin ticket separado).

Los modelos `ollama_cloud` tienen `provider === "ollama_cloud"`.
Los modelos locales tienen `is_local === true`.

---

## Cambios en CatalogViewer

### Columna "Bench"

Calcular el score promedio de los 6 campos de `task_scores`:

```ts
const benchScore = (scores: TaskScores | undefined): string => {
  if (!scores) return "—";
  const vals = [scores.chat, scores.coding, scores.planning,
                scores.analysis, scores.summarization, scores.extraction];
  const avg = vals.reduce((a, b) => a + b, 0) / vals.length;
  return avg > 0 ? avg.toFixed(1) : "—";
};
```

Representación visual: barra de 5 segmentos coloreada por valor:
- ≤ 2.0 → rojo · 2.1–3.5 → amarillo · 3.6–4.5 → verde claro · > 4.5 → verde

Tooltip en hover: `"Score calculado desde PinchBench (agente real)"`

### Badges

```tsx
{model.provider === "ollama_cloud" && (
  <span className="badge-cloud">☁ Cloud</span>
)}
{model.is_local && (
  <span className="badge-local">⚡ Local</span>
)}
```

### Manejo de ausencia de datos

Si `task_scores` llega nulo o todos los campos son 0, mostrar `"—"` en la columna.
La UI no debe romper bajo ningún caso.

---

## Criterios de aceptación

- La columna "Bench" es visible en el tab Motor
- Los modelos sin score muestran "—" (no "0" ni NaN)
- Los modelos `ollama_cloud` muestran badge `☁ Cloud`
- Los modelos locales muestran badge `⚡ Local`
- Tooltip visible en desktop
- `npm run build` pasa sin errores

## Commit message

```
feat(ui): CORE-294 add benchmark score column and provider badges to CatalogViewer
```

---

**No correr tests manualmente. No pushear a main. Abrir PR hacia main.**
**Este ticket puede empezar en paralelo con CORE-292 — no depende del kernel para la UI.**

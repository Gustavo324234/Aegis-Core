# CORE-290 — `tools/update_models.py`: Script de sincronización de modelos y scores

## Metadata
- **Epic:** 51 — Model Intelligence
- **Tipo:** chore
- **Prioridad:** Alta
- **Asignado a:** Arquitecto IA
- **Estado:✅ Done

## Contexto

El catálogo de modelos (`kernel/crates/ank-core/src/router/models.yaml`) contiene
`task_scores` y `cost_*` editados manualmente sin fuente objetiva. Esto genera dos
problemas:

1. Los scores se desactualizan silenciosamente a medida que los modelos evolucionan.
2. Los costos divergen de los precios reales de los providers.

Este ticket crea `tools/update_models.py`: un script local que Tavo ejecuta por comando
cada vez que quiere sincronizar el catálogo contra fuentes externas reales.

## Fuentes de datos

| Fuente | Datos | Endpoint |
|---|---|---|
| **PinchBench** | success rate por modelo (agente real) | HTML scraping + JSON embebido |
| **OpenRouter** | precios exactos por modelo | `https://openrouter.ai/api/v1/models` (público) |

## Diseño del script

### Uso

```bash
# Sincronización completa (precios + scores):
python tools/update_models.py

# Solo actualizar precios (OpenRouter):
python tools/update_models.py --only-prices

# Solo actualizar scores (PinchBench):
python tools/update_models.py --only-scores

# Dry-run: muestra diff sin escribir:
python tools/update_models.py --dry-run

# Agregar modelos nuevos encontrados (no los agrega por default):
python tools/update_models.py --add-new
```

### Lógica de derivación de task_scores desde PinchBench

PinchBench reporta `success_rate` (0–100%) por modelo y por categoría de tarea.
El script mapea las categorías de PinchBench a `TaskType` de Aegis:

| Categoría PinchBench | TaskType Aegis |
|---|---|
| Code / Code Devops | `coding` |
| Core Agent / Agent | `chat` |
| Data Analysis / Log Analysis | `analysis` |
| Research Knowledge | `analysis` |
| File Ops | `extraction` |
| Productivity | `planning` |
| Meeting Analysis | `summarization` |
| Creative | `chat` (fallback) |

Fórmula de conversión: `score = max(1, ceil(success_rate / 20.0))`

Ejemplos: 91% → 5 · 75% → 4 · 55% → 3 · 35% → 2 · 15% → 1

### Lógica de merge

- Los modelos en `models.yaml` que **tienen match** en PinchBench o OpenRouter → actualiza campos.
- Los modelos que **no tienen match** → se preservan sin cambios (no se borran).
- Los modelos nuevos encontrados → solo se agregan si se pasa `--add-new`.
- El `model_id` de Aegis se normaliza para matching (strip de prefijos `provider/`).

### Dependencias Python

```
requests>=2.31
pyyaml>=6.0
beautifulsoup4>=4.12
```

## Archivos creados / modificados

- `tools/update_models.py` — script principal
- `tools/requirements.txt` — dependencias del script
- `tools/README.md` — instrucciones de uso

## Criterios de aceptación

- [ ] `python tools/update_models.py --dry-run` imprime diff sin modificar `models.yaml`
- [ ] `python tools/update_models.py` actualiza costos desde OpenRouter para al menos 10 modelos
- [ ] `python tools/update_models.py --only-scores` actualiza `task_scores` para los modelos presentes en PinchBench
- [ ] Los modelos sin match en las fuentes externas no se modifican ni se eliminan
- [ ] El script es idempotente: ejecutarlo dos veces seguidas produce el mismo resultado

## Dependencias

- Ninguna (puede ejecutarse en cualquier momento)

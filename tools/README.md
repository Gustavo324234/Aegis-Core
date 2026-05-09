# tools/

Herramientas de desarrollo y mantenimiento del catálogo de Aegis.

## update_models.py — Model Catalog Synchronizer

Actualiza `kernel/crates/ank-core/src/router/models.yaml` con precios reales
y task scores derivados de benchmarks externos.

### Instalación

```bash
pip install -r tools/requirements.txt
```

### Uso

```bash
# Desde la raíz del repo:

# Sincronización completa (precios + scores):
python tools/update_models.py

# Dry-run — ver qué cambiaría sin escribir nada:
python tools/update_models.py --dry-run

# Solo actualizar precios (desde OpenRouter):
python tools/update_models.py --only-prices

# Solo actualizar scores (desde PinchBench):
python tools/update_models.py --only-scores

# Agregar modelos nuevos encontrados en las fuentes:
python tools/update_models.py --add-new

# Combinaciones:
python tools/update_models.py --only-prices --dry-run
```

### Fuentes de datos

| Fuente | Datos | Notas |
|---|---|---|
| **OpenRouter** | Precios en $/Mtok | API pública, no requiere key |
| **PinchBench** | Success rates por modelo | Scraping del sitio |

### Después de ejecutar

Siempre verificar que el YAML sigue siendo válido:

```bash
cargo build --workspace
```

Si el YAML está malformado, el build falla en compile-time con un error claro.

### Cuándo ejecutar

- Cada vez que un provider anuncia cambio de precios
- Mensualmente para mantener los scores actualizados
- Antes de una release pública

### Notas sobre PinchBench

PinchBench puede cambiar la estructura de su sitio. Si el script reporta
`PinchBench: no structured data found`, revisar la estrategia de scraping
en `_parse_pinchbench_results()`. Los precios de OpenRouter siempre funcionan
ya que es una API REST estable.

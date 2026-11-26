# CORE-301 — CMR v2: scoring contextual + latencia real + fix peso fantasma

## Metadata
- **Epic:** 51 — Model Intelligence
- **Tipo:** feat
- **Prioridad:** Alta
- **Asignado a:** Kernel Engineer
- **Estado:✅ Done

## Contexto

El CMR actual tiene cuatro problemas en `compute_score` que degradan la calidad
del routing:

1. **Peso fantasma:** `1.0_f64 * 0.30` es una constante — aporta el mismo valor
   a todos los modelos. El 30% del score es idéntico para todos. Esto reduce el
   rango efectivo del scoring al 70%.

2. **Sin contexto del prompt:** el modelo no sabe si el prompt es corto o largo,
   si tiene código, si es una pregunta analítica. Elige en base a task_type
   declarado, no al contenido real.

3. **Latencia estática:** `avg_latency_ms` viene del YAML y nunca cambia. Si un
   modelo está lento hoy, el CMR no lo sabe.

4. **Sin penalización por fallos recientes:** si un modelo devolvió error, el CMR
   lo sigue eligiendo con el mismo score.

## Ventaja sobre openrouter/auto

openrouter/auto (NotDiamond) no tiene acceso a:
- El contexto del sistema Aegis (agente, project_id, historial del tenant)
- Las keys disponibles en tiempo real
- El perfil de uso de cada modelo en esta instancia
- La latencia real observada por Aegis para este tenant

El CMR v2 usa estos inputs para routing genuinamente personalizado.

---

## Cambios requeridos

### 1. `kernel/crates/ank-core/src/router/rate_tracker.rs`

Agregar tracking de latencia real por modelo:

```rust
pub struct ModelUsageTracker {
    minute_window: Arc<RwLock<HashMap<String, VecDeque<Instant>>>>,
    daily_counts: Arc<RwLock<HashMap<String, (u32, NaiveDate)>>>,
    // NUEVO: latencias observadas — VecDeque de últimas 20 latencias por modelo
    latency_samples: Arc<RwLock<HashMap<String, VecDeque<u32>>>>,
    // NUEVO: contador de errores recientes (ventana 5 min)
    error_window: Arc<RwLock<HashMap<String, VecDeque<Instant>>>>,
}
```

Agregar métodos:

```rust
/// Registra la latencia real observada tras completar un request (en ms).
pub async fn record_latency(&self, model_id: &str, latency_ms: u32) {
    let mut map = self.latency_samples.write().await;
    let samples = map.entry(model_id.to_string()).or_default();
    if samples.len() >= 20 {
        samples.pop_front();
    }
    samples.push_back(latency_ms);
}

/// Registra un error de provider para penalizar el modelo temporalmente.
pub async fn record_error(&self, model_id: &str) {
    let now = Instant::now();
    let mut map = self.error_window.write().await;
    let queue = map.entry(model_id.to_string()).or_default();
    // Limpiar errores de más de 5 minutos
    let cutoff = now - Duration::from_secs(300);
    while queue.front().map(|t| *t < cutoff).unwrap_or(false) {
        queue.pop_front();
    }
    queue.push_back(now);
}

/// Devuelve la latencia promedio observada, o None si no hay samples.
pub async fn observed_latency_ms(&self, model_id: &str) -> Option<u32> {
    let map = self.latency_samples.read().await;
    let samples = map.get(model_id)?;
    if samples.is_empty() { return None; }
    let avg = samples.iter().map(|&v| v as u64).sum::<u64>() / samples.len() as u64;
    Some(avg as u32)
}

/// Devuelve el número de errores en los últimos 5 minutos.
pub async fn recent_errors(&self, model_id: &str) -> u32 {
    let now = Instant::now();
    let cutoff = now - Duration::from_secs(300);
    let mut map = self.error_window.write().await;
    let queue = map.entry(model_id.to_string()).or_default();
    while queue.front().map(|t| *t < cutoff).unwrap_or(false) {
        queue.pop_front();
    }
    queue.len() as u32
}
```

---

### 2. `kernel/crates/ank-core/src/router/mod.rs`

#### 2a. Función `compute_score` — reemplazar completamente

```rust
fn compute_score(
    &self,
    entry: &ModelEntry,
    task_type: TaskType,
    prompt: &str,
    max_cost: f64,
    max_latency: f64,
    observed_latency: Option<u32>,
    recent_errors: u32,
) -> f64 {
    // ── 1. Quality (40%) ─────────────────────────────────────────
    // Base: task_score normalizado (0-1)
    let base_quality = entry.score_for(task_type) as f64 / 5.0;

    // Boost por contenido detectado en el prompt
    let content_boost = detect_content_type(prompt, task_type);
    let quality = (base_quality * (1.0 + content_boost)).min(1.0);

    // ── 2. Cost (25%) ────────────────────────────────────────────
    let total_cost = entry.cost_input_per_mtok + entry.cost_output_per_mtok;
    let cost_inv = if max_cost > 0.0 {
        1.0 - (total_cost / max_cost)
    } else {
        1.0
    };

    // ── 3. Speed (20%) ───────────────────────────────────────────
    // Usar latencia observada si hay samples, sino la del catálogo
    let effective_latency = observed_latency
        .unwrap_or(entry.avg_latency_ms.unwrap_or(2000)) as f64;
    let speed_inv = if max_latency > 0.0 {
        1.0 - (effective_latency / max_latency).min(1.0)
    } else {
        1.0
    };

    // ── 4. Context fit (10%) ─────────────────────────────────────
    // Penalizar modelos cuya context window es menor que el prompt estimado
    let estimated_tokens = (prompt.len() / 4).max(1); // ~4 chars por token
    let context_fit = if entry.context_window as usize > estimated_tokens * 4 {
        1.0  // Amplio margen
    } else if entry.context_window as usize > estimated_tokens * 2 {
        0.7  // Margen ajustado
    } else if entry.context_window as usize > estimated_tokens {
        0.3  // Riesgo de truncamiento
    } else {
        0.0  // Excluir: el prompt no cabe
    };

    // ── 5. Penalización por errores recientes ────────────────────
    // 1 error → 10% de penalización, 3+ errores → 30% cap
    let error_penalty = (recent_errors as f64 * 0.10).min(0.30);

    // ── Score final ──────────────────────────────────────────────
    let raw = quality * 0.40
        + cost_inv  * 0.25
        + speed_inv * 0.20
        + context_fit * 0.15;

    (raw * (1.0 - error_penalty)).max(0.0)
}
```

#### 2b. Nueva función `detect_content_type`

```rust
/// Analiza el prompt y devuelve un boost (0.0-0.30) para el task_score
/// del modelo si su especialidad coincide con el contenido detectado.
fn detect_content_type(prompt: &str, task_type: TaskType) -> f64 {
    let lower = prompt.to_lowercase();

    // Indicadores de código
    let code_signals = ["```", "fn ", "def ", "function ", "import ", "class ",
                        "let ", "const ", "var ", "=>", "{}"];
    let code_score: f64 = code_signals.iter()
        .filter(|s| lower.contains(*s))
        .count() as f64 / code_signals.len() as f64;

    // Indicadores de análisis
    let analysis_signals = ["analiza", "analyze", "compare", "compara", "diferencia",
                            "¿por qué", "why", "explica", "explain", "cuál es mejor"];
    let analysis_score: f64 = analysis_signals.iter()
        .filter(|s| lower.contains(*s))
        .count() as f64 / analysis_signals.len() as f64;

    // Indicadores de planificación
    let planning_signals = ["plan", "roadmap", "pasos", "steps", "cómo hacer",
                            "how to", "estrategia", "strategy", "prioridad"];
    let planning_score: f64 = planning_signals.iter()
        .filter(|s| lower.contains(*s))
        .count() as f64 / planning_signals.len() as f64;

    // Devolver boost según task_type — si el contenido confirma la tarea declarada
    let boost = match task_type {
        TaskType::Code     => code_score,
        TaskType::Analysis => analysis_score,
        TaskType::Planning => planning_score,
        _                  => 0.0,
    };

    // Cap en 0.30 para no distorsionar demasiado
    (boost * 0.30_f64).min(0.30)
}
```

#### 2c. Actualizar la llamada a `compute_score` en `decide()`

Pasar el prompt y las métricas del tracker:

```rust
// En el loop de scoring, en lugar de:
let base = self.compute_score(&entry, task_type, max_cost, max_latency);

// Usar:
let observed_lat = self.tracker.observed_latency_ms(&entry.model_id).await;
let errors = self.tracker.recent_errors(&entry.model_id).await;
let base = self.compute_score(
    &entry,
    task_type,
    &pcb.l1_instruction,
    max_cost,
    max_latency,
    observed_lat,
    errors,
);
```

#### 2d. Para `max_latency` — incluir latencias observadas

```rust
// En la primera pasada (cálculo de max_latency):
let max_latency = {
    let mut ml = 0.0_f64;
    for e in &available {
        let obs = self.tracker.observed_latency_ms(&e.model_id).await;
        let lat = obs.unwrap_or(e.avg_latency_ms.unwrap_or(2000)) as f64;
        if lat > ml { ml = lat; }
    }
    ml
};
```

---

### 3. `kernel/crates/ank-core/src/chal/drivers/` — registrar latencia real

En el driver que hace el request HTTP al provider (probablemente `cloud_proxy.rs`
o similar), capturar el tiempo de respuesta y llamar al tracker:

```rust
let start = std::time::Instant::now();
let response = client.post(&api_url).send().await?;
let latency_ms = start.elapsed().as_millis() as u32;

// Registrar en el tracker
state.cognitive_router
    .tracker_ref()
    .record_latency(&model_id, latency_ms)
    .await;
```

Si el request falla:
```rust
state.cognitive_router
    .tracker_ref()
    .record_error(&model_id)
    .await;
```

Para exponer el tracker, agregar en `CognitiveRouter`:
```rust
pub fn tracker_ref(&self) -> &Arc<ModelUsageTracker> {
    &self.tracker
}
```

---

## Criterios de aceptación

- [ ] `cargo build --workspace` pasa sin warnings nuevos
- [ ] Un prompt con código (backticks) rutea hacia un modelo con `coding` score alto
- [ ] Un prompt corto (<100 chars) no penaliza modelos con context window mediano
- [ ] Un prompt que supera el context window de un modelo lo excluye del scoring
- [ ] Tras 3 errores de un modelo en 5 minutos, ese modelo baja de prioridad en el CMR
- [ ] La latencia observada real reemplaza la estática del YAML tras el primer request
- [ ] El test existente `test_decide_returns_decision_for_chat` sigue pasando

## Dependencias

- Ninguna (cambios contenidos en `rate_tracker.rs` y `mod.rs`)

# CORE-239 — fix(ank-core): CognitiveRouter — model_id con prefijo provoca 404 en APIs directas (Groq, Anthropic, etc.)

**Tipo:** fix  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Epic:** Bugs pre-lanzamiento  
**Estado:** ✅ Completo

---

## Contexto

El `ModelCatalog` usa `model_id` con prefijo de proveedor para identificación interna:
- `groq/llama-3.3-70b-versatile`
- `anthropic/claude-sonnet-4-6`
- `openai/gpt-4o`

Este prefijo es conveniente para distinguir modelos en el catálogo pero **no es el ID que las APIs nativas esperan**. Cuando el `CloudProxyDriver` envía este `model_id` directamente en el body del request, las APIs devuelven 404:

```
The model `groq/llama-3.3-70b-versatile` does not exist or you do not have access to it.
```

Evidencia en logs de producción (2026-04-30):
```
model=groq/llama-3.3-70b-versatile provider=groq
Cloud API returned error status status=404 Not Found
```

---

## Causa raíz

En `CognitiveRouter::decide()` (`router/mod.rs`), el `RoutingDecision` se construye con:

```rust
model_id: primary.model_id.clone(),  // "groq/llama-3.3-70b-versatile"
```

Este `model_id` llega al `CloudProxyDriver` que lo envía tal cual en el body JSON:

```rust
model: self.model_id.clone(),  // va directo a la API
```

Las APIs nativas (Groq, Anthropic, OpenAI) esperan el bare ID sin prefijo:
- Groq API → `llama-3.3-70b-versatile`
- Anthropic API → `claude-sonnet-4-6`
- OpenAI API → `gpt-4o`

OpenRouter es la excepción: acepta `meta-llama/llama-3.3-70b-instruct` con slash porque ese es su formato nativo.

---

## Cambios requeridos

### 1. Función `bare_model_id()` en `router/mod.rs`

Agregar una función que extrae el bare ID según el provider:

```rust
/// Returns the model ID as expected by the provider's native API.
/// Strips the leading "provider/" prefix for APIs that don't use it.
/// OpenRouter is the exception — it expects the full "org/model" format.
fn bare_model_id(model_id: &str, provider: &str) -> String {
    match provider {
        // OpenRouter expects the full "org/model" format natively
        "openrouter" => model_id.to_string(),
        // All other providers: strip the leading "provider/" prefix if present
        _ => model_id
            .split_once('/')
            .map(|(_, bare)| bare.to_string())
            .unwrap_or_else(|| model_id.to_string()),
    }
}
```

### 2. Aplicar en `CognitiveRouter::decide()`

Al construir el `RoutingDecision`, reemplazar:

```rust
// ANTES
model_id: primary.model_id.clone(),
```

```rust
// DESPUÉS
model_id: bare_model_id(&primary.model_id, &primary.provider),
```

Lo mismo para los fallbacks:

```rust
// ANTES
model_id: entry.model_id.clone(),

// DESPUÉS
model_id: bare_model_id(&entry.model_id, &entry.provider),
```

### 3. Logging

Agregar log INFO en `decide()` que muestre tanto el catalog_id como el bare_id:

```rust
info!(
    catalog_id = %primary.model_id,
    api_model_id = %bare_model_id,
    provider = %primary.provider,
    "CognitiveRouter: resolved model for API request"
);
```

---

## Criterios de aceptación

- [x] Con una key de Groq configurada, el request a la API de Groq usa `llama-3.3-70b-versatile` sin prefijo
- [x] Con una key de Anthropic, el request usa `claude-sonnet-4-6` sin prefijo
- [x] Con una key de OpenRouter, el request usa `meta-llama/llama-3.3-70b-instruct` con el slash (sin cambios)
- [x] Los logs muestran `catalog_id=groq/llama-3.3-70b-versatile api_model_id=llama-3.3-70b-versatile` 
- [x] No hay errores 404 en las pruebas con Groq, Anthropic u OpenAI directos
- [x] `cargo build` pasa sin warnings nuevos

---

## Dependencias

Ninguna. Fix autónomo.

---

## Nota para el Arquitecto IA

Este fix desbloquea las pruebas de smoke test del pipeline de agentes.
Una vez mergeado, el tenant puede usar Groq o cualquier provider directo
sin errores 404, y se puede validar el spawn con un modelo capaz.

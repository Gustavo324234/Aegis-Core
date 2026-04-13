# CORE-081 — Fix: `CloudProxyDriver` no soporta el protocolo nativo de Anthropic

**Epic:** Audit Fixes — Post-Consolidación
**Agente:** Kernel Engineer
**Prioridad:** 🟠 ALTA
**Estado:** TODO

---

## Contexto

`kernel/crates/ank-core/src/chal/drivers/cloud.rs` implementa un cliente de
inferencia que habla exclusivamente el protocolo OpenAI Chat Completions:

```rust
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
    stream: bool,
    response_format: Option<ResponseFormat>,
}
// Authorization: Bearer <key>
// POST /v1/chat/completions
```

**Problema:** Anthropic tiene su propio protocolo (`/v1/messages`) con headers
distintos (`x-api-key`, `anthropic-version`) y un formato de request/response
diferente al de OpenAI.

La URL por defecto en `entry_api_url()` para Anthropic es:
```rust
"anthropic" => "https://api.anthropic.com/v1/messages".to_string(),
```

Pero el driver intenta llamarla con headers y body de OpenAI:
```
Authorization: Bearer sk-ant-...
{ "model": "...", "messages": [...], "stream": true }
```

Anthropic responde con error 400 (`x-api-key` header missing, wrong format).

**Impacto:** Los usuarios que configuren Anthropic como provider **no pueden
chatear**. El request falla silenciosamente con un error de API que el driver
reporta como `SystemError::HardwareFailure("API Error 400: ...")`.

El `CognitiveRouter` de `models.yaml` tiene modelos de Anthropic como
candidatos de alta puntuación para varios `TaskType`. Si un usuario agrega
una Anthropic key, el router la selecciona y todas las requests fallan.

---

## Solución

Dos opciones:

### Opción A — Anthropic via OpenRouter (recomendada para MVP)

OpenRouter actúa como proxy que acepta el protocolo OpenAI y enruta a
Anthropic internamente. Cambiar la URL por defecto de Anthropic a OpenRouter:

```rust
"anthropic" => "https://openrouter.ai/api/v1/chat/completions".to_string(),
```

El usuario agrega su key de OpenRouter (no de Anthropic directamente).
Documentar en README que Anthropic se accede via OpenRouter.

**Ventaja:** Cero cambios al driver. Funciona con la implementación actual.
**Desventaja:** Requiere una key de OpenRouter, no de Anthropic directamente.

### Opción B — Driver nativo para Anthropic (completo pero más complejo)

Agregar detección de provider en `generate_stream` y construir el request
apropiado según el protocolo:

```rust
if self.provider == "anthropic" {
    // Headers: x-api-key, anthropic-version: "2023-06-01"
    // Body: { model, messages, max_tokens, stream }
    // SSE format diferente al de OpenAI
}
```

**Para este ticket se implementa Opción A** como fix inmediato para MVP.
Opción B puede ser un ticket separado post-lanzamiento.

---

## Cambios requeridos

**Archivo:** `kernel/crates/ank-core/src/router/mod.rs`

### 1. Corregir `entry_api_url` para Anthropic

```rust
// ANTES:
"anthropic" => "https://api.anthropic.com/v1/messages".to_string(),

// DESPUÉS:
// Anthropic nativo no es compatible con el protocolo OpenAI.
// Enrutar via OpenRouter que actúa como proxy compatible.
// Para acceso directo a Anthropic, usar Option B (CORE-081-native).
"anthropic" => "https://openrouter.ai/api/v1/chat/completions".to_string(),
```

### 2. Actualizar `models.yaml` si tiene URLs de Anthropic hardcodeadas

Verificar `kernel/crates/ank-core/src/router/models.yaml` y asegurarse de
que los modelos de Anthropic no tengan `api_url: "https://api.anthropic.com/..."`.

### 3. Documentar en README/AEGIS_CONTEXT

Agregar nota en `AEGIS_CONTEXT.md` bajo `LIM-*`:
```
LIM-005 | ank-core | Anthropic nativo no soportado — usar via OpenRouter
```

---

## Criterios de aceptación

- [ ] Un usuario que agrega una key de OpenRouter puede usar modelos de Anthropic
- [ ] La URL por defecto para provider `"anthropic"` apunta a OpenRouter
- [ ] `AEGIS_CONTEXT.md` documenta la limitación como `LIM-005`
- [ ] `cargo build` pasa sin errores

---

## Dependencias

Ninguna para Opción A. Verificar `models.yaml` antes de implementar.

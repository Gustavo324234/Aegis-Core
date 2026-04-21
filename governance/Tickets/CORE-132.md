# CORE-132 — Feature: Onboarding conversacional de Persona — primer mensaje sin config

**Epic:** 38 — Agent Persona System
**Repo:** Aegis-Core — `kernel/`
**Crates:** `ank-core`, `ank-http`
**Tipo:** feat
**Prioridad:** Alta
**Asignado a:** Kernel Engineer
**Depende de:** CORE-128 (firma `build_prompt` con `Option<&str>`)

---

## Contexto

Cuando un tenant inicia su primera conversación sin Persona configurada, el agente
debe preguntarle cómo quiere llamarlo y qué personalidad darle — directamente en el
chat, sin pantallas adicionales, sin configuración manual.

Si el usuario ya tiene una Persona y dice algo como "cambia tu nombre", "quiero
llamarte de otra forma", "modifica tu personalidad", el agente inicia el mismo
flujo de edición conversacional.

La Persona resultante se persiste en el enclave SQLCipher del tenant (ADR-039).

---

## Diseño del flujo

```
Primer mensaje del tenant (sin Persona en enclave)
    │
    ▼
WebSocket handler detecta: persona == None
    │
    ▼
Inyecta ONBOARDING_PROMPT en lugar de la instrucción normal
    │
    ▼
LLM responde: "¡Hola! Soy Aegis. ¿Cómo querés llamarme?
              ¿Querés que sea formal, amigable, técnico...?"
    │
    ▼  (usuario responde, ej: "Llámame Eve, sé amigable y concisa")
    ▼
WebSocket handler detecta respuesta de onboarding pendiente
    │
    ▼
Segunda inferencia: extrae nombre + descripción de personalidad
    │  (prompt estructurado → el modelo devuelve JSON)
    ▼
Handler persiste Persona en enclave → set_persona()
    │
    ▼
Confirma al usuario: "¡Perfecto! A partir de ahora soy Eve."
    │
    ▼
Conversaciones siguientes: Persona activa en cada build_prompt()
```

**Flujo de edición** (Persona ya existe, usuario pide cambio):
- Si el mensaje contiene palabras clave de cambio de identidad
  ("cámbialo", "cambia tu nombre", "modifica tu personalidad", "quiero llamarte",
  "olvida tu identidad", etc.) → mismo flujo de onboarding, sobreescribe la persona actual.

---

## Cambios requeridos

### 1. `ank-core` — Constantes de onboarding en `chal/mod.rs`

```rust
/// Prompt que dispara el onboarding cuando no hay Persona configurada.
pub const ONBOARDING_PROMPT: &str = "Un nuevo usuario acaba de iniciar una conversación \
contigo por primera vez. No tiene una identidad personalizada configurada para ti. \
Preséntate brevemente como \"Aegis\" y pregúntale de forma amigable y concisa: \
(1) ¿cómo quiere llamarte? y (2) ¿qué tipo de personalidad o tono prefiere? \
(formal, amigable, técnico, creativo, etc.). No expliques qué es Aegis ni des \
instrucciones técnicas. Solo haz las dos preguntas de forma natural.";

/// Prompt que extrae nombre y descripción de una respuesta de onboarding.
/// Devuelve JSON estricto: {"name": "...", "description": "..."}
pub const ONBOARDING_EXTRACT_PROMPT: &str = "El usuario respondió a la pregunta sobre \
cómo llamarte y qué personalidad quiere darte. Su respuesta es:\n\n\
\"{user_response}\"\n\n\
Extrae dos cosas:\n\
1. El nombre que quiere darle al agente (si no menciona nombre, usa \"Aegis\").\n\
2. Una descripción de personalidad de 1-3 oraciones para usar como system prompt.\n\
Responde ÚNICAMENTE con un objeto JSON válido, sin markdown, sin explicación:\n\
{\"name\": \"<nombre>\", \"description\": \"<descripción de 1-3 oraciones>\"}";
```

### 2. `ank-http` — Estado de onboarding por tenant en `AppState`

En `kernel/crates/ank-http/src/state.rs`, agregar:

```rust
use std::collections::HashSet;

pub struct AppState {
    // ... campos existentes ...
    /// Tenants que están en medio del flujo de onboarding de Persona.
    /// Formato: tenant_id → raw_user_response (primera respuesta al onboarding)
    pub onboarding_pending: Arc<RwLock<HashMap<String, String>>>,
    /// Tenants que recibieron la pregunta de onboarding y esperan respuesta.
    pub onboarding_asked: Arc<RwLock<HashSet<String>>>,
}
```

Inicializar ambos como `Arc::new(RwLock::new(Default::default()))` en `main.rs`.

### 3. `ank-http` — Lógica de onboarding en `ws/chat.rs`

Modificar `handle_chat()` para insertar la lógica **antes** de construir el PCB normal:

```rust
// --- ONBOARDING FLOW ---
// Paso A: ¿El tenant tiene Persona? Leer del enclave (best-effort)
let session_key_hash = hash_passphrase(&session_key);
let existing_persona: Option<String> = {
    match ank_core::enclave::TenantDB::open(&tenant_id, &session_key_hash) {
        Ok(db) => db.get_persona().unwrap_or(None),
        Err(_) => None,
    }
};

// Paso B: ¿El tenant ya recibió la pregunta de onboarding y está respondiendo?
let is_awaiting_onboarding_response = {
    state.onboarding_asked.read().await.contains(&tenant_id)
};

if is_awaiting_onboarding_response {
    // El usuario está respondiendo la pregunta de onboarding
    // → Extraer nombre + personalidad via inferencia estructurada
    state.onboarding_asked.write().await.remove(&tenant_id);

    let extract_prompt = ONBOARDING_EXTRACT_PROMPT
        .replace("{user_response}", &prompt);

    // Ejecutar inferencia de extracción (tarea de baja prioridad, sin persona)
    // → recibir JSON → parsear → set_persona() en enclave → confirmar al usuario
    // Ver implementación detallada abajo.
    handle_onboarding_extraction(&mut socket, &tenant_id, &session_key_hash, &extract_prompt, &state).await;
    continue; // No procesar como mensaje normal
}

if existing_persona.is_none() && !is_awaiting_onboarding_response {
    // Primera conversación sin Persona → disparar onboarding
    state.onboarding_asked.write().await.insert(tenant_id.clone());

    // Ejecutar inferencia con ONBOARDING_PROMPT como instrucción
    // La respuesta del LLM llega al usuario via stream normal
    // pero el prompt inyectado es ONBOARDING_PROMPT, no el mensaje del usuario
    let mut pcb = PCB::new(tenant_id.clone(), 5, ONBOARDING_PROMPT.to_string());
    pcb.model_pref = pref;
    pcb.tenant_id = Some(tenant_id.clone());
    pcb.session_key = Some(session_key_hash.clone());
    // ... dispatch normal al scheduler ...
    continue;
}

// --- FIN ONBOARDING — flujo normal con persona cargada ---
```

**Función `handle_onboarding_extraction`:**

```rust
async fn handle_onboarding_extraction(
    socket: &mut WebSocket,
    tenant_id: &str,
    session_key_hash: &str,
    extract_prompt: &str,
    state: &AppState,
) {
    // 1. Inferencia estructurada para extraer JSON de nombre + descripción
    // 2. Parsear JSON: {"name": "Eve", "description": "Eres Eve, ..."}
    // 3. Construir Persona: "Eres {name}. {description}"
    // 4. Persistir: TenantDB::open(tenant_id, session_key_hash)?.set_persona(&persona)
    // 5. Enviar confirmación al usuario via socket:
    //    "¡Perfecto! A partir de ahora soy {name}. Podés pedirme que cambie
    //     en cualquier momento diciéndome 'cambia tu personalidad'."
    // Si el JSON parsing falla → usar nombre "Aegis" + descripción genérica
    // y avisar al usuario con un mensaje de error amigable.
}
```

### 4. Detección de intención de cambio de Persona

En el flujo normal (persona ya existe), agregar detección de palabras clave:

```rust
const PERSONA_CHANGE_TRIGGERS: &[&str] = &[
    "cambia tu nombre", "cámbialo", "modifica tu personalidad",
    "quiero llamarte", "olvida tu identidad", "resetea tu personalidad",
    "change your name", "change your personality",
];

let wants_persona_change = PERSONA_CHANGE_TRIGGERS.iter()
    .any(|kw| prompt.to_lowercase().contains(kw));

if wants_persona_change {
    // Borrar persona actual y disparar onboarding
    if let Ok(db) = ank_core::enclave::TenantDB::open(tenant_id, &session_key_hash) {
        let _ = db.delete_persona();
    }
    state.onboarding_asked.write().await.insert(tenant_id.to_string());
    // Dispatch de ONBOARDING_PROMPT como en Paso B
    continue;
}
```

---

## Criterios de aceptación

- [ ] `cargo build --workspace` sin errores ni warnings Clippy
- [ ] Tenant nuevo sin Persona: el primer mensaje recibe la pregunta de onboarding
- [ ] Respuesta al onboarding: el modelo confirma el nombre y la personalidad
- [ ] Mensajes siguientes: el agente usa la Persona persistida
- [ ] "Cambia tu personalidad" reinicia el flujo de onboarding limpiamente
- [ ] Si el parsing del JSON falla: el agente usa "Aegis" como fallback, no crashea
- [ ] Tenant con Persona ya configurada: primer mensaje va directo al chat sin onboarding

---

## Dependencias

- CORE-128 — firma `build_prompt` con `Option<&str>`
- CORE-129 — `TenantDB::get_persona()`, `set_persona()`, `delete_persona()`

---

## Commit message

```
feat(ank-http): CORE-132 conversational persona onboarding — first message setup flow
```

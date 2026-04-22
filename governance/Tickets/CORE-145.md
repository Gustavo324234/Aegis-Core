# CORE-145 — Feature: Onboarding conversacional de Persona — rediseño

**Epic:** 41 — UX & Onboarding
**Repo:** Aegis-Core — `kernel/` + `shell/`
**Crates:** `ank-http`, `ank-core`
**Tipo:** feat
**Prioridad:** Alta
**Asignado a:** Kernel Engineer + Shell Engineer

---

## Problema con CORE-132 (implementación anterior)

CORE-132 implementó un onboarding de Persona pero en la pantalla de Settings,
como un formulario frío. El usuario quiere que el agente **le pregunte directamente
en el chat** al primer uso — como una conversación real, no un formulario.

---

## Comportamiento deseado

### Primera vez que el usuario abre el chat (sin Persona configurada):

```
Aegis:  ¡Hola! Soy tu asistente personal. Antes de empezar, me gustaría
        conocerte mejor.

        ¿Cómo te gustaría que me llame? (podés darme cualquier nombre)
```

El usuario responde con un nombre, por ejemplo "Nova".

```
Aegis:  Perfecto, seré Nova para vos 😊

        ¿Cómo preferís que me comunique contigo?
        · Formal y profesional
        · Casual y cercano
        · Directo y conciso
        · Creativo y expresivo
```

El usuario elige o describe libremente.

```
Aegis:  Entendido. Ya soy Nova, y voy a comunicarme de forma casual y cercana.
        ¿En qué te puedo ayudar hoy?
```

A partir de ese momento, **el agente usa esa Persona en todas las conversaciones**.

### Flujo técnico

1. Al primer WebSocket `submit`, el kernel detecta que no hay Persona configurada
2. En lugar de procesar el prompt con el LLM, el kernel inicia el flujo de onboarding
3. El onboarding es un **state machine de 2 pasos** manejado en el servidor:
   - Step 1: pedir nombre → guardar en `kv_store` como `onboarding_name`
   - Step 2: pedir estilo → construir Persona y guardarla con `set_persona()`
4. Una vez completado, el flag `onboarding_complete` se setea en el enclave
5. El mensaje original del usuario (que disparó el onboarding) se procesa normalmente

---

## Cambios requeridos

### 1. `ank-core` — Estado de onboarding en `TenantDB`

```rust
const ONBOARDING_STEP_KEY: &str = "onboarding_step";
const ONBOARDING_NAME_KEY: &str = "onboarding_pending_name";

impl TenantDB {
    /// Retorna el step actual del onboarding:
    /// None = no iniciado, Some("name") = esperando nombre, Some("style") = esperando estilo
    pub fn get_onboarding_step(&self) -> Result<Option<String>> {
        self.get_kv(ONBOARDING_STEP_KEY)
    }

    pub fn set_onboarding_step(&self, step: &str) -> Result<()> {
        self.set_kv(ONBOARDING_STEP_KEY, step)
    }

    pub fn clear_onboarding(&self) -> Result<()> {
        self.connection.execute("DELETE FROM kv_store WHERE key IN (?1, ?2)",
            [ONBOARDING_STEP_KEY, ONBOARDING_NAME_KEY])?;
        Ok(())
    }

    pub fn set_onboarding_name(&self, name: &str) -> Result<()> {
        self.set_kv(ONBOARDING_NAME_KEY, name)
    }

    pub fn get_onboarding_name(&self) -> Result<Option<String>> {
        self.get_kv(ONBOARDING_NAME_KEY)
    }
}
```

### 2. `ank-http/src/ws/chat.rs` — Interceptor de onboarding

Antes de despachar el prompt al scheduler, verificar si el tenant necesita onboarding:

```rust
// En handle_chat(), después de autenticar y antes del loop principal:

// Verificar si hay onboarding pendiente
let needs_onboarding = {
    if let Ok(db) = ank_core::enclave::TenantDB::open(&tenant_id, &hash) {
        db.get_persona().ok().flatten().is_none()
            && db.get_onboarding_step().ok().flatten().is_none()
    } else { false }
};

if needs_onboarding {
    // Iniciar onboarding — step 1: pedir nombre
    if let Ok(db) = ank_core::enclave::TenantDB::open(&tenant_id, &hash) {
        let _ = db.set_onboarding_step("awaiting_name");
    }
    let greeting = "¡Hola! Soy tu asistente personal. Antes de empezar, \
                    me gustaría conocerte mejor.\n\n\
                    ¿Cómo te gustaría que me llame?";
    let _ = socket.send(Message::Text(
        json!({ "event": "kernel_event", "data": { "output": greeting } }).to_string()
    )).await;
    let _ = socket.send(Message::Text(
        json!({ "event": "kernel_event", "data": {
            "status_update": { "state": "STATE_COMPLETED" }
        }}).to_string()
    )).await;
}
```

En el loop de mensajes, antes de crear el PCB, interceptar el onboarding:

```rust
// Al recibir un submit, verificar si hay onboarding en curso
let onboarding_step = {
    if let Ok(db) = ank_core::enclave::TenantDB::open(&tenant_id, &hash) {
        db.get_onboarding_step().ok().flatten()
    } else { None }
};

match onboarding_step.as_deref() {
    Some("awaiting_name") => {
        // El prompt ES el nombre
        let name = prompt.trim().to_string();
        if let Ok(db) = ank_core::enclave::TenantDB::open(&tenant_id, &hash) {
            let _ = db.set_onboarding_name(&name);
            let _ = db.set_onboarding_step("awaiting_style");
        }
        let response = format!(
            "Perfecto, seré **{}** para vos 😊\n\n\
            ¿Cómo preferís que me comunique contigo?\n\
            · Formal y profesional\n\
            · Casual y cercano\n\
            · Directo y conciso\n\
            · Creativo y expresivo\n\n\
            (o describí libremente cómo querés que sea)",
            name
        );
        let _ = socket.send(Message::Text(
            json!({ "event": "kernel_event", "data": { "output": response } }).to_string()
        )).await;
        let _ = socket.send(Message::Text(
            json!({ "event": "kernel_event", "data": {
                "status_update": { "state": "STATE_COMPLETED" }
            }}).to_string()
        )).await;
        continue; // No procesar como prompt normal
    }
    Some("awaiting_style") => {
        // El prompt ES el estilo elegido
        let style = prompt.trim().to_string();
        if let Ok(db) = ank_core::enclave::TenantDB::open(&tenant_id, &hash) {
            let name = db.get_onboarding_name().ok().flatten()
                .unwrap_or_else(|| "Aegis".to_string());

            // Construir la Persona
            let persona = format!(
                "Tu nombre es {}. {}\n\
                 Eres el asistente personal del usuario. Siempre te presentás \
                 con tu nombre cuando es apropiado. Mantenés este estilo en \
                 todas tus respuestas.",
                name,
                style_to_persona_instruction(&style)
            );
            let _ = db.set_persona(&persona);
            let _ = db.clear_onboarding();

            let response = format!(
                "Entendido. Ya soy **{}** 🎉\n\n\
                 A partir de ahora me voy a comunicar de esta forma. \
                 Podés cambiar mi personalidad en cualquier momento desde \
                 Configuración → Identidad.\n\n\
                 ¿En qué te puedo ayudar hoy?",
                name
            );
            let _ = socket.send(Message::Text(
                json!({ "event": "kernel_event", "data": { "output": response } }).to_string()
            )).await;
            let _ = socket.send(Message::Text(
                json!({ "event": "kernel_event", "data": {
                    "status_update": { "state": "STATE_COMPLETED" }
                }}).to_string()
            )).await;
        }
        continue;
    }
    _ => {
        // Flujo normal — continuar con el PCB
    }
}
```

Función auxiliar para convertir la elección de estilo en instrucción de Persona:

```rust
fn style_to_persona_instruction(style: &str) -> &str {
    let s = style.to_lowercase();
    if s.contains("formal") || s.contains("profesional") {
        "Comunicás de forma formal, precisa y profesional. Usás un tono respetuoso y estructurado."
    } else if s.contains("casual") || s.contains("cercano") || s.contains("amigable") {
        "Comunicás de forma casual y cercana, como un amigo de confianza. Usás un tono cálido y natural."
    } else if s.contains("directo") || s.contains("conciso") || s.contains("breve") {
        "Sos directo y conciso. Respondés sin rodeos, priorizando la claridad sobre la extensión."
    } else if s.contains("creativo") || s.contains("expresivo") || s.contains("divertido") {
        "Sos creativo y expresivo. Añadís personalidad a tus respuestas y no tenés miedo de ser original."
    } else {
        // Usar el texto libre del usuario directamente como instrucción
        // (se inyecta dinámicamente en el llamador, no acá)
        "Comunicás siguiendo el estilo preferido del usuario."
    }
}
```

**Nota para el Kernel Engineer:** Si el estilo no coincide con ninguna opción, usar el texto libre del usuario como instrucción directa en la Persona. Para eso, `style_to_persona_instruction` debería recibir el style original y devolverlo si no hay match.

### 3. Shell — Renombrar "Persona" → "Identidad" en Settings

En `SettingsPanel.tsx`, cambiar la tab "Persona" a "Identidad":
- Tab label: `Identidad` (con icono `Sparkles` o `User`)
- Descripción: "Configurá el nombre y personalidad de tu agente"
- El botón "Resetear" debería aclarar: "Al resetear, el agente te hará las preguntas de nuevo al próximo mensaje"

En `AdminDashboard.tsx`, el tab "Persona" también se renombra a "Identidad".

---

## Criterios de aceptación

- [ ] `cargo build --workspace` sin errores
- [ ] Primer mensaje al chat (sin Persona): el agente pregunta el nombre
- [ ] Al responder con un nombre: el agente confirma y pregunta el estilo
- [ ] Al elegir el estilo: Persona guardada en SQLCipher, onboarding limpiado
- [ ] Segundo inicio de sesión: sin onboarding — va directo al chat con la Persona activa
- [ ] Si el usuario resetea la Persona desde Settings: el onboarding vuelve a dispararse
- [ ] El texto del agente durante el onboarding usa el idioma del sistema (español por defecto)

---

## Dependencias

- CORE-129 (Persona en SQLCipher) — DONE ✅

---

## Commit message

```
feat(ank-http,shell): CORE-145 conversational persona onboarding in chat — name and style setup
```

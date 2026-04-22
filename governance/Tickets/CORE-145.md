# CORE-145 — Feature: Onboarding conversacional de identidad — la IA pregunta en el chat

**Epic:** 41 — UX & Onboarding
**Repo:** Aegis-Core — `kernel/`
**Crates:** `ank-http`, `ank-core`
**Tipo:** feat
**Prioridad:** Alta
**Asignado a:** Kernel Engineer

---

## Qué se quiere lograr

La primera vez que un usuario abre el chat, **el agente mismo inicia una conversación
natural** para establecer su identidad. No hay formularios, no hay Settings, no hay
pantallas de configuración. El propio agente pregunta, escucha, reacciona con
personalidad, y se transforma.

---

## Conversación de ejemplo (flujo real)

```
[Agente, primer mensaje automático al conectar]:
Hola! Soy tu nuevo asistente personal 👋
¿Cómo querés que me llame?

[Usuario]:
Te voy a llamar Jarvis.

[Agente]:
¡Jarvis! Me encanta ese nombre 😄
A partir de ahora me llamaré Jarvis.

Ahora dime, ¿qué tipo de personalidad preferís que adopte?

· Profesional y preciso
· Casual y amigable
· Directo y sin rodeos
· Curioso y creativo

(o describímela con tus palabras)

[Usuario]:
Casual y amigable

[Agente]:
Perfecto! Ya soy Jarvis, tu asistente casual y amigable 🚀
¿En qué te puedo ayudar hoy?
```

A partir de ese momento el agente usa esa identidad en **todas las conversaciones**.
Si el usuario quiere cambiarla, puede hacerlo desde Settings → tab que sea
(el nombre no importa — el flujo de chat siempre tiene prioridad).

---

## Implementación

### 1. `ank-core/src/enclave/mod.rs` — Estado de onboarding en TenantDB

```rust
const ONBOARDING_STEP_KEY: &str = "onboarding_step";
const ONBOARDING_NAME_KEY: &str = "onboarding_pending_name";

impl TenantDB {
    pub fn get_onboarding_step(&self) -> Result<Option<String>> {
        self.get_kv(ONBOARDING_STEP_KEY)
    }

    pub fn set_onboarding_step(&self, step: &str) -> Result<()> {
        self.set_kv(ONBOARDING_STEP_KEY, step)
    }

    pub fn set_onboarding_name(&self, name: &str) -> Result<()> {
        self.set_kv(ONBOARDING_NAME_KEY, name)
    }

    pub fn get_onboarding_name(&self) -> Result<Option<String>> {
        self.get_kv(ONBOARDING_NAME_KEY)
    }

    pub fn clear_onboarding(&self) -> Result<()> {
        let _ = self.connection.execute(
            "DELETE FROM kv_store WHERE key IN (?1, ?2)",
            [ONBOARDING_STEP_KEY, ONBOARDING_NAME_KEY],
        );
        Ok(())
    }
}
```

### 2. `ank-http/src/ws/chat.rs` — Interceptor de onboarding

El flujo tiene tres estados. Se intercepta **antes** de crear el PCB.

**Al conectar** (en `handle_chat()`, después de autenticar, antes del loop):

```rust
// Verificar si necesita onboarding
let should_onboard = {
    match ank_core::enclave::TenantDB::open(&tenant_id, &hash) {
        Ok(db) => {
            let has_persona = db.get_persona().ok().flatten().is_some();
            let has_step   = db.get_onboarding_step().ok().flatten().is_some();
            !has_persona && !has_step
        }
        Err(_) => false,
    }
};

if should_onboard {
    // Iniciar onboarding — guardar step e inmediatamente saludar
    if let Ok(db) = ank_core::enclave::TenantDB::open(&tenant_id, &hash) {
        let _ = db.set_onboarding_step("awaiting_name");
    }
    send_onboarding_message(
        &mut socket,
        "Hola! Soy tu nuevo asistente personal 👋\n¿Cómo querés que me llame?"
    ).await;
}
```

**En el loop de mensajes**, antes de crear el PCB, interceptar el step:

```rust
// Leer el step actual del enclave
let onboarding_step: Option<String> = {
    ank_core::enclave::TenantDB::open(&tenant_id, &hash)
        .ok()
        .and_then(|db| db.get_onboarding_step().ok().flatten())
};

match onboarding_step.as_deref() {

    // ── STEP 1: El usuario está respondiendo con el nombre ──
    Some("awaiting_name") => {
        let name = prompt.trim().to_string();
        if let Ok(db) = ank_core::enclave::TenantDB::open(&tenant_id, &hash) {
            let _ = db.set_onboarding_name(&name);
            let _ = db.set_onboarding_step("awaiting_style");
        }

        let msg = format!(
            "¡{}! Me encanta ese nombre 😄\n\
             A partir de ahora me llamaré {}.\n\n\
             Ahora dime, ¿qué tipo de personalidad preferís que adopte?\n\n\
             · Profesional y preciso\n\
             · Casual y amigable\n\
             · Directo y sin rodeos\n\
             · Curioso y creativo\n\n\
             (o describímela con tus palabras)",
            name, name
        );
        send_onboarding_message(&mut socket, &msg).await;
        continue;
    }

    // ── STEP 2: El usuario está eligiendo la personalidad ──
    Some("awaiting_style") => {
        let style_input = prompt.trim().to_string();

        if let Ok(db) = ank_core::enclave::TenantDB::open(&tenant_id, &hash) {
            let name = db.get_onboarding_name()
                .ok().flatten()
                .unwrap_or_else(|| "Aegis".to_string());

            let style_desc = map_style_to_description(&style_input);

            let persona = format!(
                "Tu nombre es {}. {}\n\
                 Eres el asistente personal del usuario. Cuando sea apropiado \
                 te referís a vos mismo como {}. Mantenés este estilo en \
                 todas tus respuestas sin excepción.",
                name, style_desc, name
            );

            let _ = db.set_persona(&persona);
            let _ = db.clear_onboarding();

            let msg = format!(
                "Perfecto! Ya soy **{}**, tu asistente {} 🚀\n\n\
                 Podés cambiar mi personalidad cuando quieras desde \
                 Configuración.\n\n\
                 ¿En qué te puedo ayudar hoy?",
                name,
                friendly_style_label(&style_input)
            );
            send_onboarding_message(&mut socket, &msg).await;
        }
        continue;
    }

    // ── Sin onboarding: flujo normal ──
    _ => {
        // Continuar con el PCB normalmente
    }
}
```

**Funciones auxiliares:**

```rust
/// Convierte la elección del usuario en una instrucción de Persona.
/// Si no coincide con ninguna opción, usa el texto libre directamente.
fn map_style_to_description(input: &str) -> String {
    let s = input.to_lowercase();
    if s.contains("profesional") || s.contains("preciso") || s.contains("formal") {
        "Sos profesional y preciso. Comunicás con claridad y rigor, \
         sin lenguaje informal.".to_string()
    } else if s.contains("casual") || s.contains("amigable") || s.contains("cercano") {
        "Sos casual y amigable, como un amigo de confianza. \
         Usás un tono cálido, natural y relajado.".to_string()
    } else if s.contains("directo") || s.contains("sin rodeos") || s.contains("conciso") {
        "Sos directo y sin rodeos. Respondés de forma breve y clara, \
         sin relleno innecesario.".to_string()
    } else if s.contains("curioso") || s.contains("creativo") || s.contains("expresivo") {
        "Sos curioso y creativo. Aportás perspectivas originales \
         y no tenés miedo de ser expresivo.".to_string()
    } else {
        // Usar el texto libre del usuario como instrucción directa
        format!("Tu estilo de comunicación: {}.", input)
    }
}

/// Label amigable para el mensaje de confirmación.
fn friendly_style_label(input: &str) -> &str {
    let s = input.to_lowercase();
    if s.contains("profesional") || s.contains("preciso") { "profesional y preciso" }
    else if s.contains("casual") || s.contains("amigable") { "casual y amigable" }
    else if s.contains("directo") || s.contains("sin rodeos") { "directo y sin rodeos" }
    else if s.contains("curioso") || s.contains("creativo") { "curioso y creativo" }
    else { "personalizado" }
}

/// Envía un mensaje de onboarding al WebSocket como si fuera el agente.
async fn send_onboarding_message(socket: &mut WebSocket, text: &str) {
    let _ = socket.send(Message::Text(
        json!({ "event": "kernel_event", "data": { "output": text } }).to_string()
    )).await;
    let _ = socket.send(Message::Text(
        json!({ "event": "kernel_event", "data": {
            "status_update": { "state": "STATE_COMPLETED" }
        }}).to_string()
    )).await;
}
```

### 3. Comportamiento al resetear la Persona desde Settings

Cuando el usuario hace `DELETE /api/persona`, el onboarding se reactiva
automáticamente en el próximo mensaje. Esto ya funciona porque:
- `get_onboarding_step()` retorna `None` (no hay step)
- `get_persona()` retorna `None` (la borraron)
- El `should_onboard` flag en el `handle_chat()` se activa

No se necesita código adicional para esto.

---

## Lo que NO cambia en la Shell

- El tab en Settings puede seguir existiendo para editar la Persona manualmente
  (usuarios avanzados)
- No hay que agregar ni quitar pantallas de la Shell para este ticket
- El onboarding es 100% manejado por el kernel en el WebSocket

---

## Criterios de aceptación

- [ ] `cargo build --workspace` sin errores
- [ ] Al conectar sin Persona: el agente saluda y pregunta el nombre automáticamente
- [ ] Al responder con un nombre: el agente lo repite con entusiasmo y pregunta la personalidad
- [ ] Al elegir la personalidad: Persona guardada en SQLCipher, agente confirma con el nombre
- [ ] Segunda sesión (Persona ya configurada): el agente NO hace onboarding — va directo al chat
- [ ] Si el usuario responde algo inusual como nombre (ej: "123"): el agente lo acepta igual
- [ ] Si el usuario describe la personalidad con texto libre: se usa como instrucción directa
- [ ] Al borrar la Persona desde Settings → próximo mensaje dispara el onboarding nuevamente

---

## Dependencias

- CORE-129 (Persona en SQLCipher) — DONE ✅
- CORE-147 (TLS fix) — despachar primero por estabilidad del servidor

---

## Commit message

```
feat(ank-http,ank-core): CORE-145 conversational identity onboarding — agent asks name and style in chat
```

# CORE-247 — Historial de chat persistente: cargar al conectar y unificar entre IP y Cloudflare

**Tipo:** feat  
**Prioridad:** Crítica  
**Épica:** UX & Onboarding (EPIC 41)  
**Asignado a:Kernel + Shell
**Componentes:**
- `kernel/crates/ank-http/src/routes/` — nuevo endpoint `GET /api/chat/history`
- `shell/ui/src/store/useAegisStore.ts` — carga del historial al conectar
- `shell/ui/src/components/ChatTerminal.tsx` — eliminar banner de workaround

**Estado:** ✅ Done
---

## Contexto

El kernel ya persiste el historial de chat en disco correctamente:
- Ruta: `{AEGIS_DATA_DIR}/users/{tenant_id}/workspace/chat_history.log`
- Formato: `\n[ISO8601] ROLE: texto\n`
- Tanto mensajes del usuario como del asistente se escriben via `append_to_chat_history`

El problema: **el frontend nunca recupera ese historial**. El store arranca con `messages: []` siempre, sin hacer ninguna llamada al backend para cargar mensajes previos. Cuando el usuario recarga la página, o accede desde otro dispositivo (IP vs Cloudflare), el chat aparece vacío aunque el historial existe en el servidor.

El banner existente en `ChatTerminal.tsx` es un workaround que confirma el problema y debe eliminarse con esta fix.

---

## Cambios requeridos

### Parte 1 — Kernel: endpoint `GET /api/chat/history`

Crear un nuevo endpoint HTTP autenticado que lea y parsee `chat_history.log` del tenant y devuelva los últimos N mensajes en formato JSON.

**Ruta:** `GET /api/chat/history?limit=50`  
**Auth:** headers estándar `x-citadel-tenant` + `x-citadel-key`

**Lógica de lectura** (en `ank-http/src/routes/`, nuevo archivo `chat_history.rs` o agregar a `workspace.rs`):

```rust
// Leer el archivo de historial del tenant
let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
let log_path = Path::new(&base_dir)
    .join("users")
    .join(&tenant_id)
    .join("workspace")
    .join("chat_history.log");

let content = tokio::fs::read_to_string(&log_path).await.unwrap_or_default();
```

**Parseo del formato existente:**

El log tiene entradas con el formato:
```
\n[2026-05-03T05:45:31Z] USER: mensaje del usuario\n
\n[2026-05-03T05:45:33Z] ASSISTANT: respuesta del asistente\n
```

Parsear cada línea no vacía que empiece con `[` en un struct:

```rust
#[derive(Serialize)]
struct ChatMessage {
    role: String,       // "user" | "assistant"
    content: String,
    timestamp: String,  // ISO8601
}
```

Devolver los últimos `limit` mensajes (default 50, máximo 200):

```json
{
  "messages": [
    { "role": "user", "content": "Hola", "timestamp": "2026-05-03T05:45:31Z" },
    { "role": "assistant", "content": "Hola! ¿En qué te puedo ayudar?", "timestamp": "2026-05-03T05:45:33Z" }
  ]
}
```

Registrar la ruta en el router de `ank-http` junto a las demás rutas de workspace/chat.

---

### Parte 2 — Shell: cargar historial al conectar

**En `useAegisStore.ts`**, en la función que establece la sesión (donde se configura el WebSocket), agregar una llamada a `GET /api/chat/history` y pre-poblar el store:

```typescript
// Llamar al backend para recuperar historial persistido
const loadChatHistory = async (tenantId: string, sessionKey: string) => {
  try {
    const res = await fetch('/api/chat/history?limit=50', {
      headers: {
        'x-citadel-tenant': tenantId,
        'x-citadel-key': sessionKey,
      },
    });
    if (!res.ok) return;
    const data = await res.json();
    const historicalMessages: Message[] = (data.messages || []).map((m: any) => ({
      id: crypto.randomUUID(),
      role: m.role === 'USER' ? 'user' : 'assistant',
      content: m.content,
      timestamp: m.timestamp,
      type: 'text',
    }));
    // Setear mensajes históricos ANTES de que el WebSocket empiece a recibir
    set({ messages: historicalMessages });
  } catch {
    // Silencioso — si falla, el chat arranca vacío (comportamiento actual)
  }
};
```

Llamar a `loadChatHistory` justo antes de establecer el WebSocket, en el mismo flujo de conexión. El historial se carga una sola vez al conectar — los mensajes nuevos se siguen agregando via el store como hasta ahora.

**Deduplicación:** si por timing el WebSocket llega a agregar un mensaje que ya está en el historial (improbable pero posible), el frontend no necesita deduplicar — el log siempre contiene mensajes anteriores a la conexión actual.

---

### Parte 3 — Shell: eliminar banner de workaround

En `ChatTerminal.tsx`, eliminar completamente el bloque del banner HTTP/IP (líneas ~230-240 actuales):

```tsx
// ELIMINAR ESTO COMPLETO:
const isHttpContext = window.location.protocol === 'http:' && ...
const [showHttpBanner, setShowHttpBanner] = useState(isHttpContext);
// ...
{showHttpBanner && (
    <div className="...">...</div>
)}
```

El banner ya no tiene razón de existir una vez que el historial se unifica por tenant_id en el servidor.

---

## Criterios de aceptación

- [x] `GET /api/chat/history` devuelve los últimos 50 mensajes del tenant autenticado en formato JSON.
- [x] El endpoint falla con 401 si las credenciales son incorrectas.
- [x] Si el archivo de historial no existe (tenant nuevo), el endpoint devuelve `{ "messages": [] }` sin error.
- [x] Al abrir el chat (IP o Cloudflare), los mensajes previos aparecen cargados en la UI.
- [x] El historial cargado se ve visualmente igual que los mensajes nuevos (mismo componente `MessageItem`).
- [x] El banner de workaround "El historial de Cloudflare no está disponible aquí" ya no existe.
- [x] `cargo build` y TypeScript sin errores.

## Dependencias

Ninguna. Este ticket es autocontenido.

## Notas de implementación

- El parseo del log debe ser tolerante a entradas malformadas (líneas incompletas, encoding issues) — ignorar líneas que no matcheen el patrón esperado.
- No cambiar el formato del `chat_history.log` — solo leerlo, no modificarlo.
- El límite de 50 mensajes es suficiente para contexto visual. El VCM ya usa el archivo completo para contexto del LLM por separado.

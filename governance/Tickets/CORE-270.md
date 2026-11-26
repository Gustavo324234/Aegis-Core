# CORE-270 — Shell: ruta `/chat/agent/:agent_id` + componente `AgentThread`

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** EPIC 50 — Agent Inbox  
**Asignado a:** Shell Engineer  
**Componentes:**
- `shell/ui/src/app/` (nueva ruta)
- `shell/ui/src/components/AgentThread.tsx` (nuevo)
- `shell/ui/src/components/AgentInboxList.tsx` (nuevo)
**Estado:** ✅ Completo

---

## Contexto

Con CORE-269 el usuario sabe que hay mensajes pendientes. Este ticket implementa la
UI para verlos y responderlos directamente — sin pasar por el chat principal.

---

## Vistas a implementar

### Vista 1: `/chat/agents` — lista de supervisores activos

Muestra todos los supervisores con mensajes pendientes o activos en la sesión.

```
┌─────────────────────────────────┐
│  Agentes activos                │
├─────────────────────────────────┤
│  🟡 Aegis-Core                  │
│  ProjectSupervisor              │
│  "¿Puedo ejecutar comandos...?" │
│  hace 2 minutos        [Abrir]  │
├─────────────────────────────────┤
│  ✅ Backend API                 │
│  Supervisor                     │
│  Tarea completada               │
│  hace 15 minutos                │
└─────────────────────────────────┘
```

Componente `AgentInboxList`: lee del `agentInboxStore`, renderiza una fila por agente.
Estado visual: 🟡 pendiente / 🔵 trabajando / ✅ completado.

### Vista 2: `/chat/agent/:agent_id` — hilo dedicado

```
┌─────────────────────────────────┐
│  ← Agentes   ProjectSupervisor  │
│              Aegis-Core    🟡   │
├─────────────────────────────────┤
│                                 │
│  [Supervisor]                   │
│  ¿Puedo ejecutar comandos en    │
│  el entorno para verificar y    │
│  clonar el repositorio?         │
│  22:16                          │
│                                 │
│                  [Tú]           │
│              Sí, podés clonar   │
│              y ejecutar lo que  │
│              necesites.         │
│              22:17              │
│                                 │
├─────────────────────────────────┤
│  Respondé al supervisor...  [↑] │
└─────────────────────────────────┘
```

Componente `AgentThread`:
- Header: nombre del proyecto, rol del supervisor, estado (badge de color)
- Historial de mensajes: preguntas del supervisor (alineadas a la izquierda) y respuestas del usuario (a la derecha), en el mismo estilo visual que el chat principal
- Input en el footer: al enviar, llama a CORE-271 (`AgentDirectMessage`)

**Importante:** el historial de este hilo viene del store en memoria (`agentInboxStore`).
Al recargar la página se pierde — eso es aceptable por ahora.

---

## Estado del hilo en el store

Extender `AgentMessage` en el store (CORE-269) para incluir el historial de intercambio:

```typescript
export interface AgentMessage {
  // ... campos existentes de CORE-269 ...
  thread: ThreadMessage[]; // historial del hilo
}

export interface ThreadMessage {
  role: 'supervisor' | 'user';
  content: string;
  timestamp: string;
}
```

Agregar método al store:
```typescript
addThreadMessage: (agentId: string, msg: ThreadMessage) => void;
```

Llamar a `addThreadMessage` tanto cuando llega el frame `supervisor_question` (agrega
el mensaje inicial del supervisor) como cuando el usuario responde (CORE-271 agrega la
respuesta del usuario al thread).

---

## Criterios de aceptación

- [x] La ruta `/chat/agents` lista todos los supervisores activos en la sesión con su estado
- [x] La ruta `/chat/agent/:agent_id` muestra el hilo de conversación con ese supervisor
- [x] El hilo muestra la pregunta del supervisor y las respuestas del usuario en orden cronológico
- [x] El input en el footer permite escribir y enviar una respuesta (dispatch a CORE-271)
- [x] Al hacer click en el badge (CORE-269), navega a `/chat/agents`
- [x] El estilo visual es consistente con el resto de la UI (usar las mismas clases de chat)
- [x] Si el agente completó su tarea, el input está deshabilitado con un mensaje "Tarea completada"

## Dependencias

- CORE-269 (store agentInbox existe)
- CORE-271 (envío de respuesta directa — puede mockearse inicialmente)

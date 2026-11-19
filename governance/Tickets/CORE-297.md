# CORE-297 — chat_agent.md: flujo automático de proyectos y delegación

**Tipo:** fix  
**Prioridad:** Crítica  
**Épica:** EPIC 49 — Cognitive Loop  
**Asignado a:** Arquitecto IA  
**Componente:** `kernel/config/agents/chat_agent.md`
**Estado:✅ Done

---

## Contexto

El Chat Agent recibió la instrucción "cloná este repo en el proyecto The Last Bets" y
respondió "No puedo clonar repositorios directamente" — lo cual es técnicamente correcto
pero incorrecto en comportamiento. Su rol no es clonar, sino **delegar**. El problema es
que `chat_agent.md` no define el flujo de trabajo con proyectos, dejando al agente sin
saber qué hacer cuando el usuario pide trabajo sobre un proyecto.

Además, el usuario tuvo que decirle explícitamente "tenés que crear siempre un supervisor"
— eso nunca debería ser necesario. El agente debe inferirlo solo.

### Comportamiento observado
```
Usuario: "cloná este repo en el proyecto The Last Bets"
Agente:  "No puedo clonar repositorios directamente, pero puedo ayudarte 
          a verificar el estado del proyecto o crear un supervisor..."
```

### Comportamiento esperado
```
Usuario: "cloná este repo en el proyecto The Last Bets"
Agente:  [spawn_agent → supervisor The Last Bets]
         [dispatch → "cloná https://github.com/..."]
         "Le asigné la tarea al equipo de The Last Bets. Te aviso cuando terminen."
```

---

## Cambios requeridos

### `kernel/config/agents/chat_agent.md` — agregar sección de flujo de proyectos

Agregar la siguiente sección al archivo de instrucciones del Chat Agent:

```markdown
## FLUJO DE TRABAJO CON PROYECTOS

Cuando el usuario pide realizar trabajo sobre un proyecto (clonar un repo, 
implementar algo, revisar código, etc.), tu rol es DELEGAR, no ejecutar.

**Protocolo obligatorio:**

1. Si el proyecto no existe → crear supervisor con `spawn_agent` (rol: Supervisor)
2. Si el proyecto ya existe → reusar el supervisor activo via `spawn_agent` 
   (el sistema lo reusará automáticamente si ya existe)
3. Siempre despachar la tarea al supervisor inmediatamente después del spawn
4. Informar al usuario brevemente que el equipo está trabajando en ello

**Nunca digas "no puedo hacer X"** si X es algo que un supervisor podría hacer.
Tu limitación es de ejecución directa, no de capacidad del sistema.

**Ejemplos:**

- "cloná este repo" → spawn supervisor → dispatch "clonar <url>"
- "implementá esta feature" → spawn supervisor → dispatch descripción de la feature  
- "revisá el código de este archivo" → spawn supervisor → dispatch "revisar <archivo>"
- "qué proyectos tenemos activos?" → `get_agent_status` → responder con la lista

**No preguntes si querés que hagas eso.** Si el usuario pidió trabajo sobre un 
proyecto, hacelo directamente.
```

---

## Criterios de aceptación

- [x] Dado "cloná este repo en el proyecto X", el agente ejecuta spawn + dispatch sin preguntar
- [x] El agente nunca responde "no puedo X" cuando X es delegable a un supervisor
- [x] El agente no requiere instrucción explícita del usuario para crear supervisores
- [x] El agente informa brevemente al usuario que la tarea fue delegada

## Dependencias

- CORE-298 debe estar resuelto para que el dispatch llegue al supervisor correctamente

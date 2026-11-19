# CORE-210 — fix(chat_agent): declarar ausencia de contexto cuando no hay proyecto activo

**Epic:** 45 — Cognitive Agent Architecture  
**Repo:** Aegis-Core / kernel/config/agents  
**Tipo:** fix  
**Estado:** ✅ Done  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Dependencias:** ninguna — es un fix de instrucciones y de inyección de contexto

---

## Contexto

El Chat Agent responde preguntas sobre el estado del proyecto con datos inventados.

**Síntoma observado (log 2026-04-28):**

El usuario pregunta "¿cuál es el estado del proyecto Aegis?" y Sol responde:

> "El proyecto está en `/workspace/Aegis-Core`. 317 archivos. `.js` (138 archivos)..."

Aegis-Core es un proyecto Rust/TypeScript — no tiene 138 archivos `.js`.
El Chat Agent está usando conocimiento de entrenamiento del LLM base (openrouter/free)
porque no tiene ProjectSupervisor activo al que hacer Query ni Dispatch.

**Causa raíz — dos problemas:**

1. **`chat_agent.md` no tiene instrucción de fallback** para cuando el sistema de agentes
   no tiene un ProjectSupervisor activo para el proyecto mencionado. El agente intenta
   responder de todas formas usando conocimiento del LLM.

2. **El contexto inyectado al Chat Agent no incluye un flag `agents_available`** que indique
   si el orquestrador tiene proyectos activos. Sin este dato el Chat Agent no puede saber
   si su respuesta está basada en estado real o en inventiva.

---

## Cambios requeridos

### 1. `kernel/config/agents/chat_agent.md`

Agregar una sección **"Cuando no hay información disponible"** antes de la sección
"Restricciones absolutas":

```markdown
## Cuando no tenés información del proyecto

Si el usuario pregunta por el estado de un proyecto y no hay ningún ProjectSupervisor
activo para ese proyecto (es decir, `agents_available: false` en tu contexto, o el
proyecto no aparece en la lista de proyectos activos), **no inventés datos**.

Respuesta correcta en ese caso:
✓ "Todavía no hay un equipo activo para ese proyecto. ¿Querés que arranquemos a trabajar en él?"
✓ "No tengo información actualizada de ese proyecto. ¿Lo activamos?"

Nunca respondas con estructura de archivos, conteo de archivos, o cualquier
descripción técnica del proyecto si no vino de un QueryReply real de un supervisor.
```

### 2. Inyección de contexto al Chat Agent — `ank-core`

En el módulo que construye el prompt del Chat Agent (probablemente
`ank-core/src/agents/instructions.rs` o el código que llama a
`InstructionLoader::build_system_prompt` para el `ChatAgent`), agregar al bloque
de contexto inyectado:

```
[CONTEXT: agents_available={true|false}]
[CONTEXT: active_projects={lista de nombres separados por coma, o "ninguno"}]
```

Esto se genera consultando `AgentOrchestrator::tree_snapshot()` antes de armar el prompt.
Si el snapshot está vacío → `agents_available=false`, `active_projects=ninguno`.

> **Nota al Kernel Engineer:** Leer `ank-core/src/agents/instructions.rs` y el código
> del `chal` (ChatAgent Loop en `ank-core/src/chal/`) para determinar exactamente dónde
> se arma el prompt del Chat Agent y cómo inyectar este contexto sin romper el VCM.

---

## Criterios de aceptación

- [x] Cuando no hay ProjectSupervisor activo, el Chat Agent responde con la frase
  de fallback y no inventa estructura del proyecto
- [x] El contexto inyectado incluye `agents_available` y `active_projects`
- [x] Cuando sí hay un ProjectSupervisor activo, el comportamiento de Dispatch/Query
  no cambia
- [x] `cargo build` sin errores

---

## Commit message

```
fix(chat_agent): CORE-210 declarar ausencia de contexto cuando no hay proyecto activo
```

---

*Arquitecto IA — 2026-04-28*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: chat_agent.md líneas 91-96/128/137: 'never assert anything about a project without a QueryReply', ejemplos ✓/✗ y 'Reglas de comunicación honesta'.

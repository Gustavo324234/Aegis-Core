# CORE-238 — docs: Agent files + PROTOCOL.md — reescritura post tool use

**Tipo:feat
**Prioridad:** Alta  
**Asignado a:Kernel Engineer
**Epic:** EPIC 47 — Agent Protocol v2  
**Estado:✅ Done

---

## Contexto

Los archivos en `kernel/config/agents/` fueron escritos asumiendo el protocolo
de tokens en texto (`[SYS_AGENT_SPAWN(...)]`). Con la migración a Tool Use
(ADR-CAA-015), estos archivos deben ser reescritos para:

1. Eliminar toda referencia a sintaxis de tokens
2. Describir las herramientas disponibles en términos de cuándo y cómo usarlas
3. Reducir el tamaño (sin sección de sintaxis, los archivos bajan ~20-30 líneas)
4. Actualizar el PROTOCOL.md para documentar las tres herramientas con schema real

Este ticket se ejecuta **después** de que CORE-234, CORE-235 y CORE-236 estén completos,
para que los agent files reflejen el comportamiento real del sistema.

---

## Archivos a actualizar

### `kernel/config/agents/chat_agent.md`
- Eliminar sección "When to Spawn" con sintaxis `[SYS_AGENT_SPAWN(...)]`
- Reemplazar por: "Use the `spawn_agent` tool when..."
- Eliminar sección "When to Query" con sintaxis `[SYS_AGENT_QUERY(...)]`
- Reemplazar por: "Use the `query_agent` tool when..."
- Resultado esperado: ~50 líneas (desde 81 actuales)

### `kernel/config/agents/project_supervisor.md`
- Misma transformación: tokens → tool descriptions
- Agregar descripción del uso de `report` tool
- Resultado esperado: ~55 líneas (desde 71 actuales)

### `kernel/config/agents/supervisor.md`
- Misma transformación
- Clarificar que `report` es obligatorio al terminar (no implícito)
- Resultado esperado: ~75 líneas (desde 99 actuales)

### `kernel/config/agents/specialist.md`
- Solo tiene `report` disponible — simplificar la sección de ejecución
- Resultado esperado: ~45 líneas (desde 56 actuales)

### `kernel/config/agents/PROTOCOL.md`
- Reemplazar completamente la sección "Token Syntax"
- Documentar las tres herramientas con su schema JSON real (igual que CORE-236)
- Agregar sección "Provider compatibility" referenciando ADR-CAA-015
- Agregar sección "Degraded mode" referenciando CORE-237
- Mantener las secciones de comunicación rules y agent tree lifecycle

---

## Criterios de aceptación

- [ ] Ningún archivo en `kernel/config/agents/` contiene la cadena `SYS_AGENT_SPAWN` o `SYS_AGENT_QUERY`
- [ ] Cada agent file describe las herramientas disponibles para ese rol
- [ ] El PROTOCOL.md documenta el schema JSON de las tres herramientas
- [ ] El PROTOCOL.md referencia ADR-CAA-015 y CORE-237 (degraded mode)
- [ ] Los archivos no hacen referencia a sintaxis de parsing — solo a semántica de uso

---

## Dependencias

- CORE-234, CORE-235, CORE-236 deben estar completos
- Este es el último ticket del Epic 47

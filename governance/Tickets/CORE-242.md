# CORE-242 — fix(ank-core): eliminar MAKER_INSTRUCTIONS del prompt del Chat Agent

**Tipo:** fix  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Epic:** EPIC 47 — Agent Protocol v2  
**Estado:** ✅ Completo

---

## Contexto

En producción (2026-05-01), el Chat Agent agota el límite de TPM de Groq en
pocas llamadas:

```
Rate limit reached TPM: Limit 12000, Used 5911, Requested 6751
```

Un solo request del Chat Agent consume ~6.751 tokens. El `chat_agent.md` pesa
~2.602 chars. La diferencia son las `MAKER_INSTRUCTIONS` que se inyectan en
`build_prompt()` para todos los agentes:

```rust
// chal/mod.rs — línea final de build_prompt()
format!(
    "{}{}{}",
    MAKER_INSTRUCTIONS, SPAWN_INSTRUCTIONS, final_prompt
)
```

`MAKER_INSTRUCTIONS` agrega ~800 tokens de instrucciones de scripting JS que
el Chat Agent no necesita — el Chat Agent no ejecuta scripts, delega esa tarea
a Specialists.

---

## Cambios requeridos

### 1. Condicionar `MAKER_INSTRUCTIONS` al rol del agente

En `CognitiveHAL::build_prompt()`, solo inyectar `MAKER_INSTRUCTIONS` cuando
el agente es un `Specialist` con `task_type = "code"` o cuando el PCB no tiene
`agent_id` y el plugin Maker está activo:

```rust
// Solo inyectar Maker si el plugin está activo Y el agente puede ejecutar scripts
let maker_section = if has_maker_plugin && pcb.agent_id.is_none() {
    MAKER_INSTRUCTIONS
} else {
    ""
};

format!("{}{}{}", maker_section, SPAWN_INSTRUCTIONS, final_prompt)
```

### 2. Verificar que el plugin Maker esté activo antes de inyectar

El `PluginManager` ya tiene `is_plugin_active("maker")`. Usar ese check
de la misma forma que se usa para `music_search` (CORE-148):

```rust
let has_maker_plugin = self
    .plugin_manager
    .read()
    .await
    .is_plugin_active("maker");
```

---

## Criterios de aceptación

- [x] El request del Chat Agent a Groq no supera 3.500 tokens en condiciones normales
- [x] El plugin Maker sigue funcionando cuando está activo y el agente puede ejecutar scripts
- [x] Los logs de `build_prompt` muestran el tamaño reducido del prompt
- [x] `cargo build` pasa sin warnings nuevos

---

## Dependencias

Ninguna. Fix autónomo en `chal/mod.rs`.

# CORE-123 — Fix: LLM genera syscalls en lugar de responder en lenguaje natural

**Status:** DONE — 2026-04-20

## Síntoma

El chat respondía:
```
[SYS_CALL_PLUGIN("USER_PROCESS_INSTRUCTION", {"instruction": "hola"})]
```
en lugar de una respuesta en lenguaje natural.

## Causa raíz

`build_prompt()` en `ank-core/src/chal/mod.rs` construía el prompt así:

```
{SYSTEM_PROMPT_MASTER}
{tool_prompt}
{mcp_tool_prompt}

[USER_PROCESS_INSTRUCTION]
{instrucción del usuario}
```

El `SYSTEM_PROMPT_MASTER` le enseña al LLM el formato de syscall:
`[SYS_CALL_PLUGIN("nombre", {...})]`

Cuando el modelo veía `[USER_PROCESS_INSTRUCTION]` como separador, lo
interpretaba como una instrucción de syscall a ejecutar — exactamente el
formato que le habían enseñado. El resultado era que generaba la syscall
en lugar de responder al usuario.

## Fix

`build_prompt()` ahora tiene dos ramas:

**Sin tools disponibles** (caso normal en instalación cloud-only):
```
{SYSTEM_PROMPT_MASTER}

{instrucción del usuario}
```
Prompt limpio, sin tags de syscall, sin sección de herramientas.
El LLM responde directamente en lenguaje natural.

**Con tools disponibles** (cuando hay plugins Wasm o MCP activos):
```
{SYSTEM_PROMPT_MASTER}

HERRAMIENTAS DISPONIBLES:
{tool_prompt}
{mcp_tool_prompt}

INSTRUCCIÓN:
{instrucción del usuario}
```
Sección de herramientas explícitamente delimitada.
El separador `INSTRUCCIÓN:` no confunde al LLM con formato de syscall.

## Test agregado

`test_build_prompt_no_tools_is_clean` — verifica que sin tools el prompt
no contiene `[USER_PROCESS_INSTRUCTION]` ni `HERRAMIENTAS DISPONIBLES`.

## Archivos modificados

- `kernel/crates/ank-core/src/chal/mod.rs`

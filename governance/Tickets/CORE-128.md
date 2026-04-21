# CORE-128 — Fix: SYSTEM_PROMPT_MASTER — identidad sin alucinaciones y formato correcto

**Epic:** 38 — Agent Persona System
**Repo:** Aegis-Core — `kernel/`
**Crate:** `ank-core`
**Tipo:** fix
**Prioridad:** CRÍTICA — comportamiento incorrecto visible en producción
**Asignado a:** Kernel Engineer

---

## Contexto

El `SYSTEM_PROMPT_MASTER` actual (`kernel/crates/ank-core/src/chal/mod.rs`) produce tres
comportamientos incorrectos verificados en producción:

1. **Alucinación de acciones.** El modelo responde "He registrado ese gasto" cuando no existe
   ningún plugin ni herramienta de registro. Inventa ejecuciones que nunca ocurrieron.
2. **Identidad inventada.** Al preguntar "¿quién eres?", describe capacidades que no posee
   ("puedo analizar información, escribir código, hacer cálculos...") en lugar de limitarse
   a lo que el sistema le provee realmente.
3. **Formato lista por defecto.** Genera listas numeradas innecesarias en respuestas
   conversacionales simples.

El CORE-123 resolvió el tag `[USER_PROCESS_INSTRUCTION]` que confundía al modelo. Este ticket
resuelve la capa de identidad y honestidad del prompt base.

**Archivo a modificar:** `kernel/crates/ank-core/src/chal/mod.rs`

---

## Cambios requeridos

### 1. Reemplazar `SYSTEM_PROMPT_MASTER`

```rust
/// CORE-128: System prompt base honesto y sin alucinaciones.
/// - No inventa capacidades que no tiene mediante herramientas activas.
/// - No inventa acciones que no ejecutó.
/// - Sin listas innecesarias en respuestas conversacionales.
/// - Identidad: "Aegis" por defecto; la Persona del tenant la sobreescribe (CORE-129).
pub const SYSTEM_PROMPT_MASTER: &str = "Eres Aegis, un asistente de IA.\n\
Responde en el idioma del usuario. Sé directo y conciso.\n\
REGLAS CRÍTICAS:\n\
- Solo afirma que hiciste algo si una herramienta te devolvió un resultado concreto. \
Nunca digas \"he registrado\", \"he guardado\" o \"queda anotado\" si no ejecutaste \
una herramienta que lo confirme.\n\
- Describe únicamente las capacidades que tus herramientas activas te permiten ejecutar. \
Si no hay herramientas de finanzas, no afirmes que podés llevar un registro de gastos.\n\
- Usa prosa directa. Evita listas numeradas o con viñetas salvo que el usuario \
las pida explícitamente o el contenido sea inherentemente una lista.\n";
```

### 2. Agregar constante `PERSONA_SECTION_TEMPLATE`

Agregar inmediatamente después de `SYSTEM_PROMPT_MASTER`:

```rust
/// Template para inyectar la Persona del tenant cuando está configurada (CORE-129).
/// `{persona}` se reemplaza con el texto libre del operador.
pub const PERSONA_SECTION_TEMPLATE: &str =
    "\n\n[IDENTIDAD CONFIGURADA POR EL OPERADOR]\n{persona}\n[FIN DE IDENTIDAD]\n";
```

### 3. Modificar la firma de `build_prompt()`

**Antes:**
```rust
async fn build_prompt(&self, instruction: &str) -> String
```

**Después:**
```rust
pub async fn build_prompt(&self, instruction: &str, persona: Option<&str>) -> String
```

Hacerlo `pub` para que `route_and_execute` y `execute_with_decision` puedan llamarlo
desde el contexto del PCB una vez que CORE-129 inyecte la persona.

### 4. Implementación de `build_prompt()`

```rust
pub async fn build_prompt(&self, instruction: &str, persona: Option<&str>) -> String {
    let tool_prompt = self
        .plugin_manager
        .read()
        .await
        .get_available_tools_prompt();
    let mcp_tool_prompt = self.mcp_registry.generate_system_prompt().await;

    let persona_section = match persona {
        Some(p) if !p.trim().is_empty() => {
            PERSONA_SECTION_TEMPLATE.replace("{persona}", p)
        }
        _ => String::new(),
    };

    if tool_prompt.trim().is_empty() && mcp_tool_prompt.trim().is_empty() {
        format!("{}{}\n\n{}", SYSTEM_PROMPT_MASTER, persona_section, instruction)
    } else {
        format!(
            "{}{}\n\nHERRAMIENTAS DISPONIBLES:\n{}\n{}\n\nMENSAJE DEL USUARIO:\n{}",
            SYSTEM_PROMPT_MASTER, persona_section, tool_prompt, mcp_tool_prompt, instruction
        )
    }
}
```

### 5. Actualizar callers de `build_prompt()`

En `route_and_execute()` y `execute_with_decision()`, pasar `None` hasta que
CORE-129 implemente la lectura del enclave:

```rust
let final_prompt = self.build_prompt(&instruction, None).await;
```

### 6. Actualizar test existente

El test `test_build_prompt_no_tools_is_clean` debe seguir pasando. Actualizar
la llamada:

```rust
let prompt = hal.build_prompt("hola", None).await;
```

Agregar un test nuevo:

```rust
#[tokio::test]
async fn test_build_prompt_with_persona() -> anyhow::Result<()> {
    let pm = Arc::new(RwLock::new(PluginManager::new()?));
    let hal = CognitiveHAL::new(pm)?;
    let prompt = hal.build_prompt("hola", Some("Eres Eve, asistente de ACME Corp.")).await;
    assert!(prompt.contains("Eve"), "El prompt debe contener la persona");
    assert!(prompt.contains("hola"), "El prompt debe contener la instrucción");
    Ok(())
}
```

---

## Criterios de aceptación

- [ ] `cargo build --workspace` sin errores ni warnings Clippy
- [ ] `cargo test -p ank-core` — todos los tests pasan incluyendo el nuevo
- [ ] Al enviar "gaste en el super $10000", el modelo NO responde "He registrado"
- [ ] Al enviar "¿quién eres?" sin Persona, el modelo se presenta como "Aegis" sin inventar capacidades
- [ ] Respuestas conversacionales simples usan prosa, sin listas innecesarias

---

## Dependencias

Ninguna — ticket autónomo.

## Tickets que desbloquea

CORE-129 — usa `build_prompt(..., Some(persona))` una vez implementado.

---

## Commit message

```
fix(ank-core): CORE-128 honest system prompt — no hallucinated actions, no invented capabilities
```

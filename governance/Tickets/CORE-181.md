# CORE-181 — fix: MakerExecutor — Boa Engine context sin top-level return ni CommonJS

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** fix  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-150 (MakerExecutor — implementación base)
**Estado:** ✅ Completo

---

## Problema

El `MakerExecutor` ejecuta código JS generado por el LLM en Boa Engine (runtime embebido).
Dos errores críticos observados en producción (logs 2026-04-25):

```
ERROR ank_server: Syscall failed: Internal Kernel Error: JS Error: SyntaxError: unexpected token 'return', statement at line 1, col 72
ERROR ank_server: Syscall failed: Internal Kernel Error: JS Error: ReferenceError: require is not defined
```

**Causa A — `SyntaxError: unexpected token 'return'`**  
El LLM genera `return read_file(...)` como expresión de nivel superior. Boa Engine
ejecuta el código como un script (no como función), donde `return` en top-level es
syntax error según la spec ES. El modelo asume un entorno Node.js con función wrapper.

**Causa B — `ReferenceError: require is not defined`**  
El LLM genera `const fs = require('fs')` u otras llamadas CommonJS. Boa Engine es un
runtime embebido sin CommonJS, sin módulos, sin `require`. Solo están disponibles
`read_file`, `write_file` y `params`.

**Causa raíz común:** el system prompt del Maker no le comunica al LLM las restricciones
del entorno JS. El modelo asume Node.js estándar.

---

## Cambios Requeridos

### 1. Wrap automático del script en función anónima — `maker.rs`

En `MakerExecutor::execute()`, antes de pasar el `code` a `context.eval()`,
envolver el código del usuario en una IIFE para que `return` sea válido:

```rust
// ANTES
match context.eval(Source::from_bytes(code)) { ... }

// DESPUÉS
let wrapped = format!("(function() {{\n{}\n}})();", code);
match context.eval(Source::from_bytes(wrapped.as_bytes())) { ... }
```

Esto hace que `return read_file('archivo.txt')` sea válido JS sin modificar el
código generado por el LLM.

### 2. Stub de `require` que retorna error descriptivo — `maker.rs`

Registrar un `require` global que retorne un mensaje de error claro en lugar de
lanzar `ReferenceError`:

```rust
let _ = context.register_global_builtin_callable(
    boa_engine::js_string!("require"),
    1,
    boa_engine::native_function::NativeFunction::from_copy_closure(|_this, args, _ctx| {
        let module = args
            .first()
            .and_then(|v| v.as_string())
            .map(|s| s.to_std_string().unwrap_or_default())
            .unwrap_or_else(|| "unknown".to_string());
        Ok(JsValue::from(boa_engine::js_string!(format!(
            "Error: require('{}') is not available in Aegis Maker sandbox. Use read_file() and write_file() instead.",
            module
        ))))
    }),
);
```

### 3. Actualizar el system prompt del Maker — `chal/mod.rs` o donde se construya

Agregar una sección explícita al prompt que recibe el LLM cuando genera código Maker:

```
MAKER — ENTORNO DE EJECUCIÓN JS:
- El entorno es Boa Engine embebido (NO Node.js, NO browser).
- No existe `require`, `import`, `module`, `exports`, `process`, `__dirname`.
- Solo están disponibles: read_file(path), write_file(path, content), params (objeto con los parámetros).
- Podés usar `return` para retornar el resultado — el script se ejecuta como función.
- El resultado del script es el valor retornado (o el último valor evaluado).
- Usá solo JavaScript puro ES2020 sin módulos externos.
```

Localizar en el código dónde se construye el prompt del Maker (buscar `SyscallRequest`
o `MAKER` en `chal/mod.rs` o `syscalls/mod.rs`) y agregar esta sección.

---

## Criterios de Aceptación

- [x] `return read_file('archivo.txt')` como top-level NO produce `SyntaxError`
- [x] `require('fs')` NO produce `ReferenceError` — retorna mensaje de error descriptivo
- [x] Test unitario: `test_maker_top_level_return` — código con `return` funciona correctamente
- [x] Test unitario: `test_maker_require_stub` — `require('fs')` retorna string de error, no panic
- [x] Los tests existentes en `maker.rs` siguen pasando sin modificación
- [x] `cargo build` sin warnings (`-D warnings`)
- [x] No usar `unwrap()` ni `expect()` en código nuevo

---

## Commit message

```
fix(ank-core): CORE-181 MakerExecutor — wrap IIFE para top-level return + stub require con error descriptivo
```

---

*Arquitecto IA — 2026-04-25*

---

## Actualización 2026-04-26 — Causa C agregada

**Nuevo error en producción:**

```
ERROR ank_server: Syscall failed: Internal Kernel Error: JS Error: SyntaxError: unexpected '\' at line 2, column 26
```

**Causa C — backslashes JSON en el código JS:**  
El LLM a veces serializa el código JS con escapes JSON (`\\n`, `\\t`, `\\/`) antes
de emitirlo. El código llega al `MakerExecutor` con backslashes literales que Boa
Engine no puede parsear como JS válido.

**Fix adicional requerido en `MakerExecutor::execute()`** — sanitizar el código
antes del IIFE wrap:

```rust
// Deserializar escapes JSON si el código llega serializado
let sanitized_code = code
    .replace("\\\\n", "\n")   // \\n → newline real
    .replace("\\\\t", "\t")   // \\t → tab real
    .replace("\\\\/", "/")    // \\/ → /
    .replace("\\\\\"", "\""); // \\" → "

let wrapped = format!("(function() {{\n{}\n}})();", sanitized_code);
```

**Test unitario adicional:**

```rust
#[test]
fn test_maker_backslash_escapes() {
    // Código con escapes JSON dobles como los que genera el LLM
    let code_with_escapes = "const x = 1;\\nreturn x;";
    // Después de sanitizar debe ser JS válido
    let sanitized = sanitize_maker_code(code_with_escapes);
    assert!(!sanitized.contains("\\\\n"));
    assert!(sanitized.contains('\n'));
}
```

Criterio de aceptación adicional:
- [x] Código con `\\n` y `\\t` se sanitiza antes de pasarlo a Boa Engine — no produce `SyntaxError: unexpected '\'`
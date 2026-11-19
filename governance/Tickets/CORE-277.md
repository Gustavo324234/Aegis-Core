# CORE-277 — Specialist: tool `web_search`

**Tipo:** feat  
**Prioridad:** Alta  
**Épica:** EPIC 50 — Agent Inbox  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/agents/tool_registry.rs`
- `kernel/crates/ank-core/src/chal/mod.rs` (`execute_tool_call_internal`)
- `kernel/config/agents/specialist.md`
**Estado:** ✅ Completo

---

## Contexto

Los specialists necesitan poder buscar información externa para tareas que
requieren conocimiento actualizado: documentación técnica, precios, tendencias,
contenidos de cursos, etc. El sistema ya tiene infraestructura de búsqueda
(SirenRouter / VCM) — este ticket la expone como tool para specialists.

Aplica a **todos los dominios**: un specialist de software busca documentación,
un specialist de un curso busca material educativo, uno de una peluquería busca
tendencias o precios de productos.

---

## Cambios requeridos

### 1. `tool_registry.rs` — agregar `web_search` a Specialist

```rust
AgentRole::Specialist { .. } => vec![
    Self::report(),
    Self::read_file(),
    Self::write_file(),
    Self::list_files(),
    Self::web_search(),   // nuevo
],
```

Definición:

```rust
fn web_search() -> ToolDefinition {
    ToolDefinition {
        name: "web_search",
        description: "Search the web for current information. Returns a list of results with titles, URLs, and snippets. Use when you need documentation, current data, or information not available in local files.",
        parameters: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query. Be specific — 3-6 words work best."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum results to return. Default: 5. Max: 10."
                }
            },
            "required": ["query"]
        }),
    }
}
```

### 2. `chal/mod.rs` — arm `web_search`

Verificar qué cliente de búsqueda existe ya en el codebase antes de implementar.
Buscar cómo el VCM ejecuta búsquedas — reutilizar ese mecanismo.

Estructura del arm:

```rust
"web_search" => {
    let query = args["query"].as_str().unwrap_or("").to_string();
    let max_results = args.get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(5)
        .min(10) as usize;

    if query.is_empty() {
        return "{\"error\":\"empty_query\"}".to_string();
    }

    // Reutilizar el cliente de búsqueda existente en el sistema.
    // Buscar en ank-core cómo el VCM o SirenRouter ejecuta búsquedas web
    // y usar el mismo mecanismo aquí.
    //
    // Formato de retorno esperado:
    // {
    //   "results": [
    //     { "title": "...", "url": "...", "snippet": "..." },
    //     ...
    //   ],
    //   "query": "...",
    //   "count": N
    // }

    match perform_web_search(&query, max_results).await {
        Ok(results) => serde_json::to_string(&results)
            .unwrap_or_else(|_| "{\"error\":\"serialization_error\"}".to_string()),
        Err(e) => format!("{{\"error\":\"search_failed\",\"detail\":\"{}\"}}", e),
    }
}
```

**Nota al Kernel Engineer:** antes de implementar `perform_web_search`, verificar
si `ank-core` ya tiene un cliente HTTP de búsqueda (Brave, DuckDuckGo, Serper, etc.)
usado por el VCM. Si existe, reutilizarlo directamente. Si no existe, usar `reqwest`
con la API de búsqueda que ya esté configurada en el sistema. No agregar dependencias
nuevas si hay una ruta existente.

### 3. `specialist.md` — agregar instrucción

```markdown
## Web search

Use `web_search` when you need information not available in local files:
documentation, current prices, tutorials, recent news, API references.

Be specific in your queries. Prefer 3-6 word queries over long sentences.
Read the snippets returned — fetch a URL only if the snippet is insufficient
and you have `read_file` access or another mechanism to retrieve it.

Do not search for information you already have in your context.
```

---

## Criterios de aceptación

- [x] `Specialist` recibe `web_search` en el ToolRegistry
- [x] El arm en `execute_tool_call_internal` retorna resultados reales de búsqueda
- [x] Retorna al menos `title`, `url`, `snippet` por resultado
- [x] `max_results` funciona y está limitado a 10
- [x] Query vacío retorna error sin crashear
- [x] Reutiliza infraestructura de búsqueda existente — no agrega dependencias nuevas
- [x] `cargo build --workspace` sin errores

## Dependencias

- CORE-275 (specialist tools — este ticket extiende el mismo set)
- No requiere CORE-276

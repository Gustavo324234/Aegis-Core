# CORE-275 — Specialist: tools de filesystem (`read_file`, `write_file`, `list_files`)

**Tipo:** feat  
**Prioridad:** Crítica — desbloquea trabajo real de specialists  
**Épica:** EPIC 50 — Agent Inbox  
**Asignado a:** Kernel Engineer  
**Componentes:**
- `kernel/crates/ank-core/src/agents/tool_registry.rs`
- `kernel/crates/ank-core/src/chal/mod.rs` (`execute_tool_call_internal`)
- `kernel/crates/ank-core/src/agents/node.rs` (opcional — approved_paths)
- `kernel/config/agents/specialist.md`
**Estado:** ✅ Completo

---

## Contexto

Hoy `AgentRole::Specialist` solo tiene el tool `report`. Un specialist recibe
instrucciones de ejecución precisas pero no puede leer ni escribir nada.
Este ticket le da las tres operaciones de filesystem fundamentales.

---

## Modelo de seguridad — dos zonas

### Zona libre: workspace propio del tenant
```
{AEGIS_DATA_DIR}/users/{tenant_id}/workspace/
```
Acceso sin restricciones. El specialist puede leer y escribir libremente.

**Regla de confinamiento (obligatoria):**
Antes de cualquier operación, el kernel llama a `Path::canonicalize()` sobre
la ruta resuelta y verifica que empiece con el workspace del tenant.
Rechaza silenciosamente (retorna error) cualquier path que escape — incluyendo
`../`, symlinks que apunten afuera, y paths absolutos que no estén aprobados.

### Zona con permiso explícito: paths externos
Paths fuera del workspace requieren aprobación previa del usuario.
La aprobación se registra en el enclave del tenant como lista de paths permitidos.
Ver CORE-276 para el mecanismo de aprobación (reutiliza `ask_user`).

Para este ticket: si el path está fuera del workspace y NO está en la lista
de paths aprobados del enclave, el tool retorna:
```json
{
  "error": "path_requires_approval",
  "path": "/home/tavo/Aegis-Core",
  "message": "Este path está fuera del workspace. El usuario debe aprobarlo primero."
}
```
El supervisor recibe este error, invoca `ask_user` para pedir permiso, y reintenta
una vez aprobado. El flujo de aprobación se implementa en CORE-276.

---

## Cambios requeridos

### 1. `tool_registry.rs` — tres tools nuevos para Specialist

```rust
AgentRole::Specialist { .. } => vec![
    Self::report(),
    Self::read_file(),
    Self::write_file(),
    Self::list_files(),
],
```

Definiciones:

```rust
fn read_file() -> ToolDefinition {
    ToolDefinition {
        name: "read_file",
        description: "Read the contents of a file. Path is relative to your workspace unless an absolute path was explicitly approved by the user.",
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path. Relative paths resolve inside the tenant workspace."
                },
                "offset": {
                    "type": "integer",
                    "description": "Optional: start reading from this line (0-based). Useful for large files."
                },
                "length": {
                    "type": "integer",
                    "description": "Optional: max number of lines to read. Default: 200."
                }
            },
            "required": ["path"]
        }),
    }
}

fn write_file() -> ToolDefinition {
    ToolDefinition {
        name: "write_file",
        description: "Write or overwrite a file. Creates parent directories if needed. Path must be inside the workspace.",
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path relative to workspace."
                },
                "content": {
                    "type": "string",
                    "description": "Full content to write."
                },
                "mode": {
                    "type": "string",
                    "enum": ["rewrite", "append"],
                    "description": "rewrite (default) replaces the file. append adds to the end."
                }
            },
            "required": ["path", "content"]
        }),
    }
}

fn list_files() -> ToolDefinition {
    ToolDefinition {
        name: "list_files",
        description: "List files and directories at a path. Defaults to workspace root.",
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path. Defaults to workspace root if omitted."
                },
                "depth": {
                    "type": "integer",
                    "description": "Max recursion depth. Default: 2. Max: 4."
                }
            },
            "required": []
        }),
    }
}
```

### 2. `chal/mod.rs` — arms en `execute_tool_call_internal`

Agregar función helper de resolución de paths (usada por los tres arms):

```rust
fn resolve_path(
    workspace: &std::path::Path,
    input_path: &str,
    approved_paths: &[String],
) -> Result<std::path::PathBuf, String> {
    let candidate = if std::path::Path::new(input_path).is_absolute() {
        std::path::PathBuf::from(input_path)
    } else {
        workspace.join(input_path)
    };

    // Canonicalizar para resolver .. y symlinks
    // Si el archivo no existe aún (write_file), canonicalizar el padre
    let canonical = if candidate.exists() {
        candidate.canonicalize()
            .map_err(|e| format!("{{\"error\":\"invalid_path\",\"detail\":\"{}\"}}", e))?
    } else {
        let parent = candidate.parent()
            .ok_or_else(|| "{\"error\":\"invalid_path\",\"detail\":\"no parent\"}".to_string())?;
        let canonical_parent = parent.canonicalize()
            .unwrap_or_else(|_| parent.to_path_buf());
        canonical_parent.join(candidate.file_name().unwrap_or_default())
    };

    // Verificar confinamiento
    if canonical.starts_with(workspace) {
        return Ok(canonical);
    }

    // Path externo — verificar aprobaciones
    let canonical_str = canonical.to_string_lossy().to_string();
    let approved = approved_paths.iter().any(|approved| {
        canonical_str.starts_with(approved.as_str())
    });

    if approved {
        Ok(canonical)
    } else {
        Err(format!(
            "{{\"error\":\"path_requires_approval\",\"path\":\"{}\",\"message\":\"Este path está fuera del workspace. El usuario debe aprobarlo primero.\"}}",
            canonical_str
        ))
    }
}
```

Arm `read_file`:
```rust
"read_file" => {
    let input_path = args["path"].as_str().unwrap_or("").to_string();
    let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let length = args.get("length").and_then(|v| v.as_u64()).unwrap_or(200) as usize;

    let workspace = get_tenant_workspace(pcb);
    let approved = get_approved_paths(pcb).await;

    let resolved = match resolve_path(&workspace, &input_path, &approved) {
        Ok(p) => p,
        Err(e) => return e,
    };

    match tokio::fs::read_to_string(&resolved).await {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let total = lines.len();
            let slice = &lines[offset.min(total)..((offset + length).min(total))];
            serde_json::json!({
                "content": slice.join("\n"),
                "total_lines": total,
                "offset": offset,
                "returned_lines": slice.len()
            }).to_string()
        }
        Err(e) => format!("{{\"error\":\"read_failed\",\"detail\":\"{}\"}}", e),
    }
}
```

Arm `write_file`:
```rust
"write_file" => {
    let input_path = args["path"].as_str().unwrap_or("").to_string();
    let content = args["content"].as_str().unwrap_or("").to_string();
    let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("rewrite");

    let workspace = get_tenant_workspace(pcb);
    let approved = get_approved_paths(pcb).await;

    // write_file solo opera dentro del workspace — nunca en paths externos
    let resolved = match resolve_path(&workspace, &input_path, &[]) {
        Ok(p) => p,
        Err(_) => return "{\"error\":\"write_outside_workspace\",\"message\":\"write_file solo puede escribir dentro del workspace del tenant.\"}".to_string(),
    };

    // Crear directorios intermedios si es necesario
    if let Some(parent) = resolved.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            return format!("{{\"error\":\"mkdir_failed\",\"detail\":\"{}\"}}", e);
        }
    }

    let result = if mode == "append" {
        use tokio::io::AsyncWriteExt;
        tokio::fs::OpenOptions::new()
            .create(true).append(true).open(&resolved).await
            .and_then(|mut f| async move { f.write_all(content.as_bytes()).await }.into())
    } else {
        tokio::fs::write(&resolved, content.as_bytes()).await
    };

    match result {
        Ok(_) => serde_json::json!({
            "status": "written",
            "path": resolved.to_string_lossy()
        }).to_string(),
        Err(e) => format!("{{\"error\":\"write_failed\",\"detail\":\"{}\"}}", e),
    }
}
```

Arm `list_files`:
```rust
"list_files" => {
    let input_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let depth = args.get("depth").and_then(|v| v.as_u64()).unwrap_or(2).min(4) as usize;

    let workspace = get_tenant_workspace(pcb);
    let approved = get_approved_paths(pcb).await;

    let resolved = match resolve_path(&workspace, input_path, &approved) {
        Ok(p) => p,
        Err(e) => return e,
    };

    fn walk(dir: &std::path::Path, depth: usize, current: usize) -> Vec<String> {
        if current > depth { return vec![]; }
        let mut entries = vec![];
        if let Ok(rd) = std::fs::read_dir(dir) {
            let mut items: Vec<_> = rd.flatten().collect();
            items.sort_by_key(|e| e.file_name());
            for entry in items {
                let path = entry.path();
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                // Ignorar directorios de build y hidden
                if name.starts_with('.') || name == "target" || name == "node_modules" {
                    continue;
                }
                if path.is_dir() {
                    entries.push(format!("[DIR]  {}", name));
                    let sub = walk(&path, depth, current + 1);
                    for s in sub { entries.push(format!("  {}", s)); }
                } else {
                    entries.push(format!("[FILE] {}", name));
                }
            }
        }
        entries
    }

    let listing = walk(&resolved, depth, 0);
    serde_json::json!({
        "path": resolved.to_string_lossy(),
        "entries": listing
    }).to_string()
}
```

### 3. Helpers de contexto

Agregar dos helpers que extraen info del PCB para los arms:

```rust
fn get_tenant_workspace(pcb: &PCB) -> std::path::PathBuf {
    let data_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
    let tenant = pcb.tenant_id.as_deref().unwrap_or("default");
    std::path::Path::new(&data_dir)
        .join("users")
        .join(tenant)
        .join("workspace")
}

async fn get_approved_paths(pcb: &PCB) -> Vec<String> {
    // Leer del enclave del tenant — implementado en CORE-276
    // Por ahora retorna vacío (solo workspace propio está disponible)
    vec![]
}
```

### 4. `specialist.md` — actualizar instrucciones

Agregar sección al agent file:

```markdown
## Filesystem tools

You have access to `read_file`, `write_file`, and `list_files`.

All paths are relative to your workspace unless an absolute path was
explicitly approved by the user.

If `read_file` or `list_files` returns `path_requires_approval`, report
this to your parent supervisor — do not try to access the path directly.
Your supervisor will coordinate approval with the user via `ask_user`.

`write_file` only works inside your workspace. Never attempt to write
outside it.

When reading large files, use `offset` and `length` to avoid loading
more than you need.
```

---

## Criterios de aceptación

- [x] `Specialist` recibe `read_file`, `write_file`, `list_files` en el ToolRegistry
- [x] `read_file` con path relativo resuelve dentro del workspace del tenant
- [x] `read_file` con path absoluto dentro del workspace funciona
- [x] `read_file` con path absoluto fuera del workspace sin aprobación retorna `path_requires_approval`
- [x] `write_file` rechaza cualquier path fuera del workspace — retorna `write_outside_workspace`
- [x] `list_files` sin path retorna el workspace root
- [x] `../` y symlinks que escapan el workspace son rechazados por `canonicalize()`
- [x] `cargo build --workspace` sin errores

## Dependencias

- CORE-262 ✅ (specialists llaman al LLM real — este ticket agrega tools a ese loop)
- CORE-276 (aprueba paths externos — `get_approved_paths` retorna `[]` hasta que esté implementado)

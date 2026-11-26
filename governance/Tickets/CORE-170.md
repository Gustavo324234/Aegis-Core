# CORE-170 — feat: FileSystemBridge — Endpoints /api/fs/tree y /api/fs/file

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-http  
**Tipo:** feat  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer  
**Dependencias:** CORE-167 (project_root desde workspace_config)
**Estado:** ✅ Completo

---

## Contexto

Para que el Code Viewer en el Dashboard pueda mostrar el árbol de archivos y el contenido del código, el backend necesita exponer el filesystem del proyecto de forma controlada y segura.

---

## Cambios Requeridos

### Nuevos endpoints en `ank-http/src/routes/fs.rs`

```
GET /api/fs/tree?path=.
    → Árbol de archivos y directorios (hasta 3 niveles de profundidad)
    → Requiere: Citadel auth
    → Retorna: FileTreeDto

GET /api/fs/file?path=src/main.rs
    → Contenido de un archivo como texto
    → Requiere: Citadel auth
    → Retorna: FileContentDto
```

### DTOs

```rust
#[derive(Serialize)]
pub struct FileTreeDto {
    pub path: String,
    pub entries: Vec<FsEntry>,
}

#[derive(Serialize)]
pub struct FsEntry {
    pub name: String,
    pub path: String,      // relativo al project_root
    pub kind: FsEntryKind,
    pub children: Option<Vec<FsEntry>>,  // None para archivos, Some para dirs
    pub extension: Option<String>,
}

#[derive(Serialize)]
pub enum FsEntryKind { File, Directory }

#[derive(Serialize)]
pub struct FileContentDto {
    pub path: String,
    pub content: String,
    pub language: String,  // "rust" | "typescript" | "toml" | "json" | "markdown" | etc.
    pub lines: usize,
}
```

### Seguridad (obligatoria)

```rust
fn resolve_safe_path(project_root: &Path, requested: &str) -> anyhow::Result<PathBuf> {
    // 1. Rechazar rutas absolutas
    if requested.starts_with('/') || requested.contains(':') {
        anyhow::bail!("Absolute paths not allowed");
    }
    // 2. Resolver la ruta relativa al project_root
    let resolved = project_root.join(requested).canonicalize()?;
    // 3. Verificar que la ruta resuelta siga siendo hija del project_root
    if !resolved.starts_with(project_root) {
        anyhow::bail!("Path traversal detected");
    }
    Ok(resolved)
}
```

**Extensiones permitidas para `/api/fs/file`:**
`.rs`, `.ts`, `.tsx`, `.js`, `.jsx`, `.toml`, `.json`, `.md`, `.yaml`, `.yml`, `.env.example`, `.gitignore`, `.sh`

**Archivos bloqueados explícitamente** (retornan 403 aunque pasen el path check):
- `.env`
- `aegis.env`
- `*.key`, `*.pem`, `*.p12`
- Cualquier archivo cuyo nombre contenga `secret`, `password`, `token`

**Directorios ignorados en `/api/fs/tree`:**
`node_modules`, `target`, `.git`, `dist`, `build`, `.aegis`

### Tamaño máximo de archivo

`/api/fs/file` rechaza archivos mayores a 500KB con `413 Content Too Large`.

---

## Criterios de Aceptación

- [x] `GET /api/fs/tree?path=.` retorna el árbol del project_root configurado para el tenant
- [x] `GET /api/fs/tree?path=../etc` retorna `400 Bad Request`
- [x] `GET /api/fs/file?path=.env` retorna `403 Forbidden`
- [x] `GET /api/fs/file?path=src/main.rs` retorna el contenido y `language: "rust"`
- [x] El árbol no incluye `node_modules`, `target`, ni `.git`
- [x] El árbol tiene profundidad máxima de 3 niveles
- [x] Archivos > 500KB retornan `413`
- [x] `cargo build` pasa sin warnings

---

*Arquitecto IA — 2026-04-24*
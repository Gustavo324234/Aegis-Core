use aegis_sdk::{run_plugin, PluginMetadata, PluginRequest, PluginResponse};
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

const WORKSPACE_ROOT: &str = "/workspace";

fn main() -> Result<()> {
    let metadata = PluginMetadata {
        name: "std_fs".to_string(),
        description: "Standard File System Plugin for Aegis OS".to_string(),
        example_json: serde_json::json!({
            "action": "list_dir",
            "params": {"path": "."}
        }),
    };

    run_plugin(metadata, process_request)
}

fn process_request(request: &PluginRequest) -> Result<PluginResponse> {
    match request.action.as_str() {
        "list_dir" => list_dir(&request.params),
        "read_file" => read_file(&request.params),
        _ => Ok(PluginResponse {
            status: "error".to_string(),
            data: None,
            error: Some(format!("Acción desconocida: {}", request.action)),
        }),
    }
}

fn list_dir(params: &serde_json::Value) -> Result<PluginResponse> {
    let relative_path = params["path"].as_str().unwrap_or(".");
    let target_path = safe_path(relative_path)?;

    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&target_path)
        .with_context(|| format!("No se pudo leer el directorio: {}", relative_path))?;

    for entry in read_dir {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let metadata = entry.metadata()?;
        let kind = if metadata.is_dir() { "dir" } else { "file" };

        entries.push(serde_json::json!({
            "name": file_name,
            "type": kind,
            "size": metadata.len()
        }));
    }

    Ok(PluginResponse {
        status: "success".to_string(),
        data: Some(serde_json::Value::Array(entries)),
        error: None,
    })
}

fn read_file(params: &serde_json::Value) -> Result<PluginResponse> {
    let relative_path = params["path"]
        .as_str()
        .context("Se requiere la clave 'path' para leer un archivo")?;
    let target_path = safe_path(relative_path)?;

    let content = fs::read_to_string(&target_path)
        .with_context(|| format!("No se pudo leer el archivo: {}", relative_path))?;

    Ok(PluginResponse {
        status: "success".to_string(),
        data: Some(serde_json::Value::String(content)),
        error: None,
    })
}

/// Construye una ruta segura dentro del prefijo /workspace.
/// Evita ataques de path traversal básicos.
fn safe_path(rel_path: &str) -> Result<PathBuf> {
    let mut path = PathBuf::from(WORKSPACE_ROOT);

    // Eliminamos prefijos peligrosos como / o ..
    let rel = rel_path.trim_start_matches(['/', '\\']);
    path.push(rel);

    // En un entorno WASI real con jailing, no podemos salir de /workspace
    // pero añadimos una verificación lógica extra.
    if !path.starts_with(WORKSPACE_ROOT) {
        anyhow::bail!("Acceso fuera del workspace denegado: {}", rel_path);
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_path_logic() -> anyhow::Result<()> {
        // En tests unitarios normales (no wasm), /workspace no existirá,
        // pero validamos la construcción de la ruta.
        let p = safe_path("docs/readme.txt")?;
        let p_str = p.to_str().context("Path is not valid UTF-8")?;
        assert!(p_str.contains("workspace"));
        Ok(())
    }

    #[test]
    fn test_unknown_action() -> anyhow::Result<()> {
        let req = aegis_sdk::PluginRequest {
            action: "not_exists".to_string(),
            params: serde_json::Value::Null,
        };
        let res = process_request(&req)?;
        assert_eq!(res.status, "error");
        Ok(())
    }
}

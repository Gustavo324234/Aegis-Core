use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

/// Representa la solicitud recibida por el plugin.
#[derive(Debug, Deserialize)]
pub struct PluginRequest {
    pub action: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Representa la respuesta enviada por el plugin.
#[derive(Debug, Serialize)]
pub struct PluginResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Metadatos que describen el plugin y que el core utilizará para autodescubrimiento.
#[derive(Debug, Serialize, Clone)]
pub struct PluginMetadata {
    pub name: String,
    pub description: String,
    pub example_json: serde_json::Value,
}

/// Macro / Función principal de ejecución del SDK. Aisla stdin/stdout
/// y maneja la serialización y errores sistemáticamente sin panics.
pub fn run_plugin<F>(metadata: PluginMetadata, handler: F) -> Result<()>
where
    F: Fn(&PluginRequest) -> Result<PluginResponse>,
{
    let mut buffer = String::new();

    // Zero panic policy: Intentamos leer stdin y devolvemos Error en caso de fallo crítico
    if let Err(e) = io::stdin().read_to_string(&mut buffer) {
        let err_res = PluginResponse {
            status: "error".to_string(),
            data: None,
            error: Some(format!("Error reading stdin: {:?}", e)),
        };
        write_output(&err_res);
        return Ok(());
    }

    let response = process_internal(metadata, &buffer, handler);
    write_output(&response);

    Ok(())
}

fn process_internal<F>(metadata: PluginMetadata, input: &str, handler: F) -> PluginResponse
where
    F: Fn(&PluginRequest) -> Result<PluginResponse>,
{
    let request: PluginRequest = match serde_json::from_str(input) {
        Ok(req) => req,
        Err(e) => {
            return PluginResponse {
                status: "error".to_string(),
                data: None,
                error: Some(format!("Error parsing JSON request: {:?}", e)),
            };
        }
    };

    // Auto-Discovery: manejamos 'get_metadata' limpiamente sin ir al handler
    if request.action == "get_metadata" {
        return PluginResponse {
            status: "success".to_string(),
            data: Some(serde_json::to_value(&metadata).unwrap_or(serde_json::Value::Null)),
            error: None,
        };
    }

    match handler(&request) {
        Ok(res) => res,
        Err(e) => PluginResponse {
            status: "error".to_string(),
            data: None,
            // Guardamos el detalle del anyhow::Result
            error: Some(format!("{:?}", e)),
        },
    }
}

fn write_output(response: &PluginResponse) {
    if let Ok(output) = serde_json::to_string(response) {
        let _ = io::stdout().write_all(output.as_bytes());
        let _ = io::stdout().flush();
    }
}

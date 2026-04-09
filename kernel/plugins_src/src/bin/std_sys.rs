use aegis_sdk::{run_plugin, PluginMetadata, PluginRequest, PluginResponse};
use anyhow::{Context, Result};
use chrono::Utc;

fn main() -> Result<()> {
    let metadata = PluginMetadata {
        name: "std_sys".to_string(),
        description: "Standard System Plugin for Aegis OS (Time & OS Info)".to_string(),
        example_json: serde_json::json!({
            "action": "get_time"
        }),
    };

    run_plugin(metadata, process_request)
}

fn process_request(request: &PluginRequest) -> Result<PluginResponse> {
    match request.action.as_str() {
        "get_time" => {
            let now = Utc::now();
            Ok(PluginResponse {
                status: "success".to_string(),
                data: Some(
                    serde_json::to_value(now).context("Error al convertir Utc::now() a JSON")?,
                ),
                error: None,
            })
        }
        _ => Ok(PluginResponse {
            status: "error".to_string(),
            data: None,
            error: Some(format!("Acción desconocida: {}", request.action)),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;

    #[test]
    fn test_process_get_time() -> anyhow::Result<()> {
        let req = PluginRequest {
            action: "get_time".to_string(),
            params: serde_json::Value::Null,
        };
        let result = process_request(&req)?;
        assert_eq!(result.status, "success");
        assert!(result.data.is_some());

        // Verificamos que sea una fecha válida (formato ISO 8601 que usa chrono por defecto en Serde)
        let data = result.data.context("data should be present")?;
        let date_str = data
            .as_str()
            .context("data should be a string")?
            .to_string();
        assert!(date_str.contains("Z")); // UTC
        Ok(())
    }

    #[test]
    fn test_unknown_action() -> anyhow::Result<()> {
        let req = PluginRequest {
            action: "unknown_cmd".to_string(),
            params: serde_json::Value::Null,
        };
        let result = process_request(&req)?;
        assert_eq!(result.status, "error");
        let err_msg = result.error.context("error should be present")?;
        assert!(err_msg.contains("Acción desconocida"));
        Ok(())
    }
}

use anyhow::Result;
use aegis_sdk::{run_plugin, PluginMetadata, PluginRequest, PluginResponse};
use serde_json::json;

/// Entry point for the "Hello World" plugin.
/// This demonstrates the minimal implementation of an Aegis OS plugin.
fn main() -> Result<()> {
    let metadata = PluginMetadata {
        name: "hello_world".to_string(),
        description: "A simple hello world plugin for educational purposes.".to_string(),
        example_json: json!({
            "action": "greet",
            "params": {
                "name": "World"
            }
        }),
    };

    // run_plugin handles the stdin/stdout bridge and avoids panics.
    run_plugin(metadata, handle_request)
}

/// The core logic of the plugin. 
/// Dispatches actions based on the request.
fn handle_request(request: &PluginRequest) -> Result<PluginResponse> {
    match request.action.as_str() {
        "greet" => {
            // Extracts the "name" from params, defaulting to "Stranger" if not present.
            let name = request.params["name"].as_str().unwrap_or("Stranger");
            let message = format!("Hello, {}! From Aegis OS.", name);
            
            Ok(PluginResponse {
                status: "success".to_string(),
                data: Some(json!({ "message": message })),
                error: None,
            })
        }
        "echo" => {
            // Simply returns the input params back to the caller.
            Ok(PluginResponse {
                status: "success".to_string(),
                data: Some(request.params.clone()),
                error: None,
            })
        }
        _ => {
            // Returns a structured error for any unrecognized action.
            Ok(PluginResponse {
                status: "error".to_string(),
                data: None,
                error: Some(format!("Unknown action: {}", request.action)),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() -> Result<()> {
        let req = PluginRequest {
            action: "greet".to_string(),
            params: json!({ "name": "Alice" }),
        };
        let res = handle_request(&req)?;
        assert_eq!(res.status, "success");
        assert_eq!(res.data.unwrap()["message"], "Hello, Alice! From Aegis OS.");
        Ok(())
    }

    #[test]
    fn test_unknown_action() -> Result<()> {
        let req = PluginRequest {
            action: "unknown".to_string(),
            params: json!({}),
        };
        let res = handle_request(&req)?;
        assert_eq!(res.status, "error");
        assert!(res.error.unwrap().contains("Unknown action"));
        Ok(())
    }
}

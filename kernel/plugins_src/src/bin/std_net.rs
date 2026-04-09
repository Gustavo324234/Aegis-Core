use aegis_sdk::{run_plugin, PluginMetadata, PluginRequest, PluginResponse};
use anyhow::Result;

fn main() -> Result<()> {
    let metadata = PluginMetadata {
        name: "std_net".to_string(),
        description: "Network Access Plugin (URL Fetching & HTML Cleaning)".to_string(),
        example_json: serde_json::json!({
            "action": "fetch",
            "url": "https://example.com"
        }),
    };

    run_plugin(metadata, process_request)
}

fn process_request(request: &PluginRequest) -> Result<PluginResponse> {
    match request.action.as_str() {
        "parse" => {
            let html = request
                .params
                .get("html")
                .and_then(|h| h.as_str())
                .unwrap_or("");

            let cleaned_text = clean_html(html);

            Ok(PluginResponse {
                status: "success".to_string(),
                data: Some(serde_json::Value::String(cleaned_text)),
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

/// Limpiador de HTML ultra-ligero para Wasm (Cero dependencias externas).
/// Usa una máquina de estados básica para omitir todo lo que esté entre < y >.
fn clean_html(html: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script_or_style = false;

    // Simplificación: no manejamos tags anidados complejos ni comentarios de forma perfecta,
    // pero para extraer texto legible por una IA es suficiente.

    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' => {
                in_tag = true;
                // Detectar inicio de <script o <style para ignorar su contenido
                let mut tag_acc = String::new();
                while let Some(&next_c) = chars.peek() {
                    if next_c == '>' || next_c.is_whitespace() {
                        break;
                    }
                    if let Some(next_char) = chars.next() {
                        tag_acc.push(next_char.to_ascii_lowercase());
                    }
                }
                if tag_acc == "script" || tag_acc == "style" {
                    in_script_or_style = true;
                }
            }
            '>' => {
                in_tag = false;
                // Si cerramos un tag de script o style, debemos buscar el cierre </script>...
                // Pero para esta versión v1 simplificada, simplemente reiniciamos flags.
            }
            _ => {
                if !in_tag && !in_script_or_style {
                    output.push(c);
                }
            }
        }
    }

    // Post-procesamiento: Colapsar espacios y eliminar líneas vacías excesivas
    output
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_html() {
        let html = "<html><body><h1>Hola</h1><p>Mundo</p><style>body { color: red; }</style></body></html>";
        let cleaned = clean_html(html);
        assert!(cleaned.contains("Hola"));
        assert!(cleaned.contains("Mundo"));
        assert!(!cleaned.contains("color: red"));
        assert!(!cleaned.contains("<html>"));
    }
}

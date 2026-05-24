use crate::agents::node::AgentRole;
use crate::agents::tool_registry::ToolRegistry;
use crate::chal::ToolCallRecord;
use serde_json::{json, Value};

/// Normaliza el nombre de la herramienta corrigiendo typos comunes.
pub fn normalize_tool_name(name: &str) -> String {
    let clean = name
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>();

    match clean.as_str() {
        "spawnagent" | "spawn_agents" | "spawnagent_tool" => "spawn_agent".to_string(),
        "queryagent" | "query_agents" | "queryagent_tool" => "query_agent".to_string(),
        "readfile" | "read_files" | "readfile_tool" => "read_file".to_string(),
        "writefile" | "write_files" | "writefile_tool" => "write_file".to_string(),
        "listfiles" | "listfile" | "listfiles_tool" => "list_files".to_string(),
        "askuser" | "ask_users" | "askuser_tool" => "ask_user".to_string(),
        "answersupervisor" | "answer_supervisors" | "answersupervisor_tool" => {
            "answer_supervisor".to_string()
        }
        "addledgerentry" | "add_ledger" | "ledgerentry" => "add_ledger_entry".to_string(),
        "approvepath" | "approve_paths" => "approve_path".to_string(),
        "getprojectledger" | "projectledger" => "get_project_ledger".to_string(),
        "getagentstatus" | "agentstatus" => "get_agent_status".to_string(),
        "executecommand" | "runcommand" | "run_command" | "execute_command_tool" => {
            "execute_command".to_string()
        }
        "websearch" | "searchweb" | "search_web" => "web_search".to_string(),
        "report" | "report_tool" => "report".to_string(),
        _ => name.to_string(),
    }
}

/// Intenta balancear llaves y corchetes en un JSON potencialmente truncado.
pub fn balance_braces(s: &str) -> String {
    let mut balanced = s.to_string();
    let mut braces = 0i32;
    let mut brackets = 0i32;
    let mut in_string = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            // Saltear el carácter escapado si estamos en un string
            let _ = chars.peek();
        } else if c == '"' {
            in_string = !in_string;
        } else if !in_string {
            if c == '{' {
                braces += 1;
            } else if c == '}' {
                braces -= 1;
            } else if c == '[' {
                brackets += 1;
            } else if c == ']' {
                brackets -= 1;
            }
        }
    }

    if in_string {
        balanced.push('"');
    }

    while brackets > 0 {
        balanced.push(']');
        brackets -= 1;
    }
    while braces > 0 {
        balanced.push('}');
        braces -= 1;
    }

    balanced
}

/// Remueve delimitadores markdown de código JSON si existieran en los argumentos.
pub fn sanitize_json_string(s: &str) -> String {
    let trimmed = s.trim();
    let mut clean = trimmed.to_string();

    // Eliminar triple backticks
    if clean.starts_with("```") {
        if let Some(first_newline) = clean.find('\n') {
            clean = clean[first_newline..].to_string();
        } else {
            clean = clean[3..].to_string();
        }
    }
    if clean.ends_with("```") {
        clean = clean[..clean.len() - 3].to_string();
    }

    let clean = clean.trim().to_string();
    balance_braces(&clean)
}

/// Aplica sanitización sintáctica, validación semántica y coerción de tipos.
pub fn validate_and_sanitize_tool_call(
    tc: &ToolCallRecord,
    role: &AgentRole,
) -> Result<ToolCallRecord, String> {
    // 1. Normalizar el nombre de la herramienta
    let original_name = tc.function.name.trim();
    let norm_name = normalize_tool_name(original_name);

    // Obtener las herramientas permitidas para el rol
    let allowed_definitions = ToolRegistry::definitions_for(role);
    let allowed_names: Vec<&str> = allowed_definitions.iter().map(|d| d.name).collect();

    if !allowed_names.contains(&norm_name.as_str()) {
        return Err(format!(
            "Error: La herramienta '{}' no está permitida o no existe para tu rol. \
             Herramientas permitidas: {:?}",
            original_name, allowed_names
        ));
    }

    // Obtener la definición específica
    let tool_def = allowed_definitions
        .iter()
        .find(|d| d.name == norm_name)
        .ok_or_else(|| format!("Definición no encontrada para {}", norm_name))?;

    // 2. Sanitizar y parsear los argumentos
    let raw_args = &tc.function.arguments;
    let sanitized_args_str = sanitize_json_string(raw_args);

    let mut args_value: Value = match serde_json::from_str(&sanitized_args_str) {
        Ok(v) => v,
        Err(e) => {
            // Si el parser de JSON falla rotundamente
            return Err(format!(
                "Error: Los argumentos para '{}' no son un JSON válido. \
                 Detalle del error de parseo: {}. Argumentos recibidos: '{}'",
                norm_name, e, raw_args
            ));
        }
    };

    // Asegurarse de que sea un objeto JSON
    if !args_value.is_object() {
        return Err(format!(
            "Error: Los argumentos de '{}' deben ser un objeto JSON (dict). Recibido: '{}'",
            norm_name, sanitized_args_str
        ));
    }

    let args_obj = args_value.as_object_mut().unwrap();

    // 3. Coerción de tipos heurística según el schema de la herramienta
    if let Some(properties) = tool_def
        .parameters
        .get("properties")
        .and_then(|p| p.as_object())
    {
        for (prop_name, prop_schema) in properties {
            if let Some(expected_type) = prop_schema.get("type").and_then(|t| t.as_str()) {
                if let Some(val) = args_obj.get_mut(prop_name) {
                    match expected_type {
                        "integer" | "number" => {
                            // Coaccionar string numérico a número
                            if let Some(val_str) = val.as_str() {
                                if let Ok(parsed_int) = val_str.trim().parse::<i64>() {
                                    *val = json!(parsed_int);
                                } else if let Ok(parsed_float) = val_str.trim().parse::<f64>() {
                                    *val = json!(parsed_float);
                                }
                            }
                        }
                        "string" => {
                            // Coaccionar número o boolean a string
                            if val.is_number() {
                                *val = json!(val.to_string());
                            } else if let Some(b) = val.as_bool() {
                                *val = json!(b.to_string());
                            }
                        }
                        "boolean" => {
                            // Coaccionar string a boolean
                            if let Some(val_str) = val.as_str() {
                                if val_str.to_lowercase() == "true" {
                                    *val = json!(true);
                                } else if val_str.to_lowercase() == "false" {
                                    *val = json!(false);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // 4. Validar parámetros requeridos
    if let Some(required) = tool_def
        .parameters
        .get("required")
        .and_then(|r| r.as_array())
    {
        for req_field_val in required {
            if let Some(req_field) = req_field_val.as_str() {
                if !args_obj.contains_key(req_field) || args_obj.get(req_field).unwrap().is_null() {
                    return Err(format!(
                        "Error: El parámetro '{}' es requerido para la herramienta '{}'. \
                         Asegurate de incluirlo en tu llamada. Parámetros esperados: {:?}",
                        req_field, norm_name, tool_def.parameters
                    ));
                }
            }
        }
    }

    // 5. Validaciones semánticas específicas adicionales
    match norm_name.as_str() {
        "spawn_agent" => {
            if let Some(role_val) = args_obj.get("role").and_then(|v| v.as_str()) {
                let norm_role = role_val.to_lowercase();
                if norm_role != "project_supervisor"
                    && norm_role != "supervisor"
                    && norm_role != "specialist"
                {
                    return Err("Error: El parámetro 'role' de 'spawn_agent' debe ser uno de: ['project_supervisor', 'supervisor', 'specialist'].".to_string());
                }
            }
            if let Some(type_val) = args_obj.get("task_type").and_then(|v| v.as_str()) {
                let norm_type = type_val.to_lowercase();
                if norm_type != "code"
                    && norm_type != "analysis"
                    && norm_type != "planning"
                    && norm_type != "creative"
                {
                    return Err("Error: El parámetro 'task_type' de 'spawn_agent' debe ser uno de: ['code', 'analysis', 'planning', 'creative'].".to_string());
                }
            }
        }
        "report" => {
            if let Some(status_val) = args_obj.get("status").and_then(|v| v.as_str()) {
                let norm_status = status_val.to_lowercase();
                if norm_status != "completed" && norm_status != "error" && norm_status != "blocked"
                {
                    return Err("Error: El parámetro 'status' de 'report' debe ser uno de: ['completed', 'error', 'blocked'].".to_string());
                }
            }
        }
        "write_file" => {
            if let Some(mode_val) = args_obj.get("mode").and_then(|v| v.as_str()) {
                let norm_mode = mode_val.to_lowercase();
                if norm_mode != "rewrite" && norm_mode != "append" {
                    return Err("Error: El parámetro 'mode' de 'write_file' debe ser uno de: ['rewrite', 'append'].".to_string());
                }
            }
        }
        "answer_supervisor" => {
            if let Some(agent_id_str) = args_obj.get("agent_id").and_then(|v| v.as_str()) {
                if uuid::Uuid::parse_str(agent_id_str).is_err() {
                    return Err("Error: El parámetro 'agent_id' de 'answer_supervisor' debe ser un UUID válido.".to_string());
                }
            }
        }
        _ => {}
    }

    // Retornar el tool call sanitizado
    Ok(ToolCallRecord {
        id: tc.id.clone(),
        type_: tc.type_.clone(),
        function: crate::chal::FunctionCallRecord {
            name: norm_name,
            arguments: serde_json::to_string(&args_value).unwrap_or_default(),
        },
    })
}

/// Sanitiza el contenido de un prompt/mensaje para evitar la fuga de credenciales sensibles.
pub fn sanitize_prompt_content(content: &str) -> String {
    static OPENAI_REGEX: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static OPENROUTER_REGEX: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static GEMINI_REGEX: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static ANTHROPIC_REGEX: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static PRIVATE_KEY_REGEX: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static DB_PASSWORD_REGEX: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

    let openai = OPENAI_REGEX.get_or_init(|| regex::Regex::new(r"sk-[a-zA-Z0-9]{48}").unwrap());
    let openrouter =
        OPENROUTER_REGEX.get_or_init(|| regex::Regex::new(r"sk-or-v1-[a-f0-9]{64}").unwrap());
    let gemini =
        GEMINI_REGEX.get_or_init(|| regex::Regex::new(r"AIzaSy[a-zA-Z0-9_-]{33}").unwrap());
    let anthropic = ANTHROPIC_REGEX
        .get_or_init(|| regex::Regex::new(r"sk-ant-sid01-[a-zA-Z0-9_-]{93}").unwrap());
    let private_key = PRIVATE_KEY_REGEX.get_or_init(|| {
        regex::Regex::new(
            r"(?s)-----BEGIN [A-Z\s]+ PRIVATE KEY-----.+?-----END [A-Z\s]+ PRIVATE KEY-----",
        )
        .unwrap()
    });
    let db_password = DB_PASSWORD_REGEX.get_or_init(|| {
        regex::Regex::new(r"([a-zA-Z0-9+.-]+://[^:\s]+):([^@\s]+)(@[^:\s]+)").unwrap()
    });

    let s = openai.replace_all(content, "[REDACTED_OPENAI_KEY]");
    let s = openrouter.replace_all(&s, "[REDACTED_OPENROUTER_KEY]");
    let s = gemini.replace_all(&s, "[REDACTED_GEMINI_KEY]");
    let s = anthropic.replace_all(&s, "[REDACTED_ANTHROPIC_KEY]");
    let s = private_key.replace_all(&s, "[REDACTED_PRIVATE_KEY]");
    let s = db_password.replace_all(&s, "$1:[REDACTED_PASSWORD]$3");

    s.into_owned()
}

/// Detecta si un prompt corresponde a una tarea trivial/rápida (greetings, simple queries).
pub fn is_light_task(prompt: &str, task_type: crate::pcb::TaskType) -> bool {
    if !matches!(task_type, crate::pcb::TaskType::Chat) {
        return false;
    }
    let trimmed = prompt.trim();
    let char_count = trimmed.chars().count();
    if char_count < 30 {
        return true;
    }
    if char_count < 120 {
        let lower = trimmed.to_lowercase();
        let greeting_keywords = [
            "hola",
            "hello",
            "help",
            "status",
            "hi",
            "hey",
            "ping",
            "test",
            "buenos dias",
            "buenas tardes",
            "buenas noches",
            "buenos días",
            "estado",
        ];
        let words: std::collections::HashSet<&str> = lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect();
        if greeting_keywords.iter().any(|&kw| {
            if kw.contains(' ') {
                lower.contains(kw)
            } else {
                words.contains(kw)
            }
        }) {
            return true;
        }
    }
    false
}

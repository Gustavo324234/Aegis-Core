use crate::agents::node::AgentRole;
use serde_json::{json, Value};

/// Proveedor de inferencia — determina el formato de serialización de tool definitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderKind {
    Anthropic,
    OpenAI,
    Groq,
    Gemini,
    Ollama,
    OllamaCloud,
    OpenRouter,
    Xai,
    Mistral,
    DeepSeek,
    Qwen,
}

impl ProviderKind {
    pub fn from_string(s: &str) -> Self {
        // CORE-FIX: normalise aliases ("google" → "gemini", "claude" →
        // "anthropic", "grok" → "xai", case + punctuation variants) so the
        // tool serializer doesn't silently default to OpenAI when the caller
        // happens to spell the provider differently from this match.
        let normalised = crate::router::normalize_provider_id(s);
        match normalised.as_str() {
            "anthropic" => Self::Anthropic,
            "openai" => Self::OpenAI,
            "groq" => Self::Groq,
            "gemini" => Self::Gemini,
            "ollama" => Self::Ollama,
            "ollama_cloud" => Self::OllamaCloud,
            "openrouter" => Self::OpenRouter,
            "xai" => Self::Xai,
            "mistral" => Self::Mistral,
            "deepseek" => Self::DeepSeek,
            "qwen" => Self::Qwen,
            _ => {
                tracing::warn!(
                    provider = s,
                    normalised = %normalised,
                    "ProviderKind::from_string: unknown provider, defaulting to OpenAI"
                );
                Self::OpenAI
            }
        }
    }
}

/// Definición canónica de una herramienta del protocolo Agent Protocol v2.
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

impl From<&ToolDefinition> for crate::pcb::ToolDefinition {
    fn from(def: &ToolDefinition) -> Self {
        Self {
            name: def.name.to_string(),
            description: def.description.to_string(),
            parameters: def.parameters.clone(),
        }
    }
}

impl From<ToolDefinition> for crate::pcb::ToolDefinition {
    fn from(def: ToolDefinition) -> Self {
        Self {
            name: def.name.to_string(),
            description: def.description.to_string(),
            parameters: def.parameters,
        }
    }
}

/// Registra y serializa las herramientas del Agent Protocol v2 (EPIC 47).
///
/// Cada agente recibe un set de herramientas según su rol:
/// - ChatAgent            → [spawn_agent, answer_supervisor]
/// - ProjectSupervisor    → [spawn_agent, query_agent, report, ask_user, add_ledger_entry]
/// - Supervisor           → [spawn_agent, query_agent, report, ask_user, add_ledger_entry]
/// - Specialist           → [report, read_file, write_file, list_files]
pub struct ToolRegistry;

impl ToolRegistry {
    /// Retorna las tool definitions para el rol dado, serializadas para el proveedor indicado.
    pub fn tools_for(role: &AgentRole, provider: &ProviderKind) -> Vec<Value> {
        Self::definitions_for(role)
            .iter()
            .map(|def| Self::serialize(def, provider))
            .collect()
    }

    pub fn definitions_for(role: &AgentRole) -> Vec<ToolDefinition> {
        match role {
            // CORE-243: ChatAgent no tiene agent_id — query_agent y report requieren uno.
            // Solo puede hacer spawn_agent (para crear un ProjectSupervisor) y answer_supervisor (CORE-263).
            // CORE-375: Herramientas personales instantáneas (finanzas, memoria, recordatorios).
            AgentRole::ChatAgent => vec![
                Self::spawn_agent(),
                Self::answer_supervisor(),
                Self::send_to_antigravity(),
                Self::manage_ai_provider(),
                Self::manage_modules(),
                Self::get_project_ledger(),
                Self::get_agent_status(), // CORE-289
                Self::log_expense_or_entry(),
                Self::query_ledger(),
                Self::remember_fact(),
                Self::manage_reminder(),
                Self::manage_google_calendar(),
                Self::manage_whatsapp(),
            ],
            AgentRole::ProjectSupervisor { .. } | AgentRole::Supervisor { .. } => {
                vec![
                    Self::spawn_agent(),
                    Self::query_agent(),
                    Self::report(),
                    Self::ask_user(),
                    Self::add_ledger_entry(),
                    Self::approve_path(),
                    Self::read_file(),
                    Self::write_file(),
                    Self::list_files(),
                    Self::execute_command(),
                    Self::web_search(),
                ]
            }
            AgentRole::Specialist { .. } => vec![
                Self::report(),
                Self::read_file(),
                Self::write_file(),
                Self::list_files(),
                Self::execute_command(),
                Self::web_search(),
            ],
        }
    }

    /// Serializa una ToolDefinition al formato del proveedor.
    ///
    /// CORE-FIX: every provider Aegis currently talks to is reached through
    /// an OpenAI-compatible chat-completions endpoint:
    ///   - OpenAI / Groq / xAI / DeepSeek / Mistral / Qwen → native OpenAI compat
    ///   - OpenRouter → OpenAI compat (handles all underlying providers)
    ///   - Gemini → Google's `/v1beta/openai/chat/completions` (OpenAI compat)
    ///   - Anthropic → reached via OpenRouter (OpenAI compat)
    ///   - Ollama / Ollama Cloud → OpenAI compat
    ///
    /// So they all want the same OpenAI tool shape. The previous code branched
    /// to `functionDeclarations` for Gemini (native shape) and `input_schema`
    /// for Anthropic (native shape), but neither path is ever the upstream we
    /// actually hit — every Gemini tool call was being 400'd because the body
    /// shape didn't match the URL.
    ///
    /// If CloudProxyDriver ever grows native Anthropic / native Gemini
    /// support, branch here on a future `ProviderKind::AnthropicNative` /
    /// `GeminiNative` variant.
    pub fn serialize(def: &ToolDefinition, _provider: &ProviderKind) -> Value {
        json!({
            "type": "function",
            "function": {
                "name": def.name,
                "description": def.description,
                "parameters": def.parameters,
            }
        })
    }

    /// CORE-376 / CORE-377: Serializa una `crate::pcb::ToolDefinition` canónica al formato OpenAI-compat
    /// usado por `CloudProxyDriver` y `NativeOllamaDriver` (consistente con `ToolRegistry::serialize`).
    pub fn serialize_canonical(
        def: &crate::pcb::ToolDefinition,
        _provider: &ProviderKind,
    ) -> Value {
        json!({
            "type": "function",
            "function": {
                "name": def.name,
                "description": def.description,
                "parameters": def.parameters,
            }
        })
    }

    /// CORE-376: Serializa una herramienta al formato nativo del proveedor o formato OpenAI compatible.
    pub fn serialize_for_driver(
        def: &ToolDefinition,
        provider: &ProviderKind,
        native_format: bool,
    ) -> Value {
        if !native_format {
            return Self::serialize(def, provider);
        }

        match provider {
            ProviderKind::Anthropic => json!({
                "name": def.name,
                "description": def.description,
                "input_schema": def.parameters,
            }),
            ProviderKind::Gemini => json!({
                "name": def.name,
                "description": def.description,
                "parameters": def.parameters,
            }),
            _ => Self::serialize(def, provider),
        }
    }

    /// CORE-376: Deserializa llamadas a herramientas estructuradas de cualquier proveedor soportado.
    pub fn deserialize_tool_calls(
        provider: &ProviderKind,
        payload: &Value,
    ) -> Result<Vec<crate::pcb::ToolCall>, String> {
        let mut calls = Vec::new();

        match provider {
            ProviderKind::Anthropic => {
                let blocks = if let Some(arr) = payload.as_array() {
                    arr
                } else if let Some(arr) = payload.get("content").and_then(|c| c.as_array()) {
                    arr
                } else {
                    return Ok(calls);
                };

                for block in blocks {
                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        let id = block
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("toolu_unknown")
                            .to_string();
                        let name = block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| "Missing 'name' in Anthropic tool_use".to_string())?
                            .to_string();
                        let input = block
                            .get("input")
                            .cloned()
                            .unwrap_or(Value::Object(Default::default()));
                        calls.push(crate::pcb::ToolCall::new(id, name, input));
                    }
                }
            }
            ProviderKind::Gemini => {
                let parts = if let Some(parts) = payload
                    .get("candidates")
                    .and_then(|c| c.get(0))
                    .and_then(|c0| c0.get("content"))
                    .and_then(|cnt| cnt.get("parts"))
                    .and_then(|p| p.as_array())
                {
                    parts
                } else if let Some(parts) = payload.get("parts").and_then(|p| p.as_array()) {
                    parts
                } else {
                    return Ok(calls);
                };

                for (idx, part) in parts.iter().enumerate() {
                    if let Some(fc) = part.get("functionCall") {
                        let name = fc
                            .get("name")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| "Missing 'name' in Gemini functionCall".to_string())?
                            .to_string();
                        let args = fc
                            .get("args")
                            .cloned()
                            .unwrap_or(Value::Object(Default::default()));
                        let id = format!("call_gemini_{}", idx);
                        calls.push(crate::pcb::ToolCall::new(id, name, args));
                    }
                }
            }
            _ => {
                let tool_calls_arr = if let Some(arr) = payload.as_array() {
                    arr
                } else if let Some(arr) = payload.get("tool_calls").and_then(|t| t.as_array()) {
                    arr
                } else if let Some(arr) = payload
                    .get("choices")
                    .and_then(|c| c.get(0))
                    .and_then(|c0| c0.get("message"))
                    .and_then(|m| m.get("tool_calls"))
                    .and_then(|t| t.as_array())
                {
                    arr
                } else {
                    return Ok(calls);
                };

                for tc in tool_calls_arr {
                    let id = tc
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("call_unknown")
                        .to_string();
                    let func = tc
                        .get("function")
                        .ok_or_else(|| "Missing 'function' in tool_call".to_string())?;
                    let name = func
                        .get("name")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| "Missing 'name' in tool_call function".to_string())?
                        .to_string();
                    let args = match func.get("arguments") {
                        Some(Value::String(s)) => {
                            serde_json::from_str::<Value>(s).unwrap_or(Value::String(s.clone()))
                        }
                        Some(v) => v.clone(),
                        None => Value::Object(Default::default()),
                    };
                    calls.push(crate::pcb::ToolCall::new(id, name, args));
                }
            }
        }

        Ok(calls)
    }

    // --- Definiciones de las tres herramientas del protocolo ---

    fn spawn_agent() -> ToolDefinition {
        ToolDefinition {
            name: "spawn_agent",
            description: "Create a subordinate agent to handle a specific task or domain.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "role": {
                        "type": "string",
                        "enum": ["project_supervisor", "supervisor", "specialist"],
                        "description": "The role of the new agent."
                    },
                    "name": {
                        "type": "string",
                        "description": "Human-readable identifier. Required for project_supervisor."
                    },
                    "scope": {
                        "type": "string",
                        "description": "Task or domain description. Injected into the agent's instructions."
                    },
                    "task_type": {
                        "type": "string",
                        "enum": ["code", "analysis", "planning", "creative"],
                        "description": "Cognitive nature of the task. Used by the CMR to select a model."
                    }
                },
                "required": ["role", "scope"]
            }),
        }
    }

    fn query_agent() -> ToolDefinition {
        ToolDefinition {
            name: "query_agent",
            description: "Query an active project for information without creating any work.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "project": {
                        "type": "string",
                        "description": "Project name to query."
                    },
                    "question": {
                        "type": "string",
                        "description": "The specific question to answer."
                    }
                },
                "required": ["project", "question"]
            }),
        }
    }

    fn report() -> ToolDefinition {
        ToolDefinition {
            name: "report",
            description: "Report the result of your work to your parent agent.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": ["completed", "error", "blocked"],
                        "description": "Outcome of the task."
                    },
                    "summary": {
                        "type": "string",
                        "description": "Concise summary of what was done or what failed."
                    },
                    "observations": {
                        "type": "string",
                        "description": "Optional findings relevant to the parent agent."
                    }
                },
                "required": ["status", "summary"]
            }),
        }
    }

    /// CORE-263: Permite a supervisores pausar y preguntar al usuario via Chat Agent.
    fn ask_user() -> ToolDefinition {
        ToolDefinition {
            name: "ask_user",
            description: "Pausar la tarea y hacerle una pregunta al usuario via el Chat Agent. Usar cuando necesités una decisión que solo el usuario puede tomar. El supervisor queda en pausa hasta recibir la respuesta.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "La pregunta para el usuario."
                    },
                    "context": {
                        "type": "string",
                        "description": "Contexto breve de por qué necesitás esta información."
                    }
                },
                "required": ["question"]
            }),
        }
    }

    /// CORE-263: Permite al Chat Agent enviar la respuesta del usuario a un supervisor pausado.
    fn answer_supervisor() -> ToolDefinition {
        ToolDefinition {
            name: "answer_supervisor",
            description: "Enviar la respuesta del usuario a un supervisor que está esperando input. Usar cuando el usuario responde a una pregunta de un supervisor activo.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "agent_id": {
                        "type": "string",
                        "description": "UUID del agente supervisor que está esperando."
                    },
                    "answer": {
                        "type": "string",
                        "description": "La respuesta del usuario."
                    }
                },
                "required": ["agent_id", "answer"]
            }),
        }
    }

    /// CORE-385: Permite al Chat Agent enviar una instrucción o respuesta directamente a Google Antigravity.
    fn send_to_antigravity() -> ToolDefinition {
        ToolDefinition {
            name: "send_to_antigravity",
            description: "Envía una instrucción, pedido técnico o respuesta del usuario a Google Antigravity (tanto al SDK oficial google-antigravity como a la sesión activa del IDE). Usalo cuando el usuario diga 'decile a Antigravity que...', 'pedile a Antigravity...', o quiera delegar trabajo en Antigravity.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "instruction": {
                        "type": "string",
                        "description": "Instrucción o mensaje completo para enviar a Antigravity."
                    },
                    "project_name": {
                        "type": "string",
                        "description": "Nombre opcional del proyecto o sesión (por defecto 'Antigravity')."
                    }
                },
                "required": ["instruction"]
            }),
        }
    }

    /// Permite al Chat Agent agregar nuevas API keys, proveedores o modelos directamente desde el chat.
    fn manage_ai_provider() -> ToolDefinition {
        ToolDefinition {
            name: "manage_ai_provider",
            description: "Lista proveedores configurados, consulta el catálogo de modelos disponibles (por ejemplo OpenRouter o modelos gratuitos), o agrega/elimina claves API (Groq, Gemini, OpenRouter, OpenAI, Anthropic, DeepSeek, xAI, Mistral, Ollama). Si el usuario pega una API key (gsk_..., AIza..., sk-...), el proveedor se detecta automáticamente.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["add", "add_model", "list", "catalog", "remove"],
                        "description": "'list' para ver los proveedores configurados y su estado; 'catalog' para listar los modelos disponibles en el catálogo (filtrables por provider, free_only y limit — usalo cuando pidan la lista de modelos de OpenRouter o de otro proveedor); 'add' para registrar una nueva API key/proveedor; 'add_model' para sumar un modelo a un proveedor ya configurado; 'remove' para eliminar un proveedor."
                    },
                    "provider": {
                        "type": "string",
                        "description": "Nombre del proveedor: 'groq', 'gemini', 'openrouter', 'openai', 'anthropic', 'deepseek', 'xai', 'mistral', 'ollama'. Opcional si se provee una api_key con prefijo reconocible o en 'list'/'catalog'."
                    },
                    "api_key": {
                        "type": "string",
                        "description": "Clave API del proveedor (no requerida para 'ollama', 'list', 'catalog', 'add_model' o 'remove')."
                    },
                    "model": {
                        "type": "string",
                        "description": "ID del modelo a activar o agregar (ej: 'gemini-2.5-flash', 'llama-3.3-70b-versatile', 'deepseek-chat', 'llama3.2'). Si se omite en 'add', se usa el modelo recomendado por defecto del proveedor."
                    },
                    "api_url": {
                        "type": "string",
                        "description": "URL base opcional (útil para Ollama local 'http://localhost:11434/v1' o endpoints custom)."
                    },
                    "set_primary": {
                        "type": "boolean",
                        "description": "Si es true, establece este proveedor y modelo como motor primario inmediato."
                    },
                    "free_only": {
                        "type": "boolean",
                        "description": "Usado con action='catalog'. Si es true, devuelve solo modelos gratuitos o de free tier."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Usado con action='catalog'. Cantidad máxima de modelos a devolver (por defecto 30, máximo 100)."
                    }
                },
                "required": ["action"]
            }),
        }
    }

    /// Permite al Chat Agent inspeccionar, habilitar o deshabilitar módulos del microkernel y plugins cargados.
    fn manage_modules() -> ToolDefinition {
        ToolDefinition {
            name: "manage_modules",
            description: "Lista los módulos del microkernel disponibles en el sistema (con su estado habilitado/deshabilitado y herramientas expuestas), lista los plugins activos cargados, y permite habilitar o deshabilitar un módulo por su module_id.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["list", "enable", "disable"],
                        "description": "'list' para listar todos los módulos disponibles e instalados y plugins cargados; 'enable' o 'disable' para activar o desactivar un módulo."
                    },
                    "module_id": {
                        "type": "string",
                        "description": "ID del módulo a habilitar o deshabilitar (requerido para 'enable' y 'disable', por ejemplo 'aegis-biz')."
                    }
                },
                "required": ["action"]
            }),
        }
    }

    // --- CORE-375: Personal Assistant Instant Tools ---

    fn log_expense_or_entry() -> ToolDefinition {
        ToolDefinition {
            name: "log_expense_or_entry",
            description: "Registra un gasto, compra, ingreso o movimiento numérico del usuario. Deducí la categoría por sentido común (ej: 'super' -> 'comida', 'nafta' -> 'transporte').",
            parameters: json!({
                "type": "object",
                "properties": {
                    "domain": {
                        "type": "string",
                        "enum": ["gastos", "ingresos", "otros"],
                        "description": "Dominio del registro (por defecto 'gastos')."
                    },
                    "category": {
                        "type": "string",
                        "description": "Categoría normalizada en minúsculas: 'comida', 'transporte', 'hogar', 'salud', 'ocio', 'tecnologia', 'impuestos', etc."
                    },
                    "amount": {
                        "type": "number",
                        "description": "Monto numérico del gasto o ingreso."
                    },
                    "description": {
                        "type": "string",
                        "description": "Descripción clara del ítem (ej: 'Supermercado', 'Carnicería', 'Carga de nafta')."
                    }
                },
                "required": ["category", "amount", "description"]
            }),
        }
    }

    fn query_ledger() -> ToolDefinition {
        ToolDefinition {
            name: "query_ledger",
            description: "Consulta los gastos acumulados, totales por categoría o historial de movimientos financieros.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "domain": {
                        "type": "string",
                        "description": "'gastos' o 'ingresos' (opcional)"
                    },
                    "category": {
                        "type": "string",
                        "description": "Categoría específica como 'comida', 'transporte' o 'all' para ver todas."
                    }
                }
            }),
        }
    }

    fn remember_fact() -> ToolDefinition {
        ToolDefinition {
            name: "remember_fact",
            description: "Guarda, actualiza (consolidando un recuerdo existente sin duplicar) o elimina un dato importante sobre el usuario en la memoria a largo plazo (preferencias, familia, rutinas, gustos, proyectos, reglas, salud).",
            parameters: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["upsert", "update", "delete"],
                        "description": "Acción sobre la memoria: 'upsert' (por defecto, actualiza si existe una nota del mismo tema o crea una nueva), 'update' (reemplaza la nota por 'id'), o 'delete' (elimina una nota por 'id')."
                    },
                    "id": {
                        "type": "string",
                        "description": "ID numérico de la nota existente (ej. '12' o '#12') tomado de [MEMORIA A LARGO PLAZO DEL USUARIO], requerido para 'delete' y recomendado para 'update'."
                    },
                    "category": {
                        "type": "string",
                        "description": "Categoría del recuerdo: 'preferencias', 'rutina', 'salud', 'trabajo', 'familia', 'general'."
                    },
                    "fact": {
                        "type": "string",
                        "description": "El hecho concreto y consolidado a recordar a largo plazo (no requerido si action='delete')."
                    }
                },
                "required": ["category", "fact"]
            }),
        }
    }

    fn manage_reminder() -> ToolDefinition {
        ToolDefinition {
            name: "manage_reminder",
            description: "Agrega, lista, actualiza, marca como completado o elimina un recordatorio o rutina recurrente del usuario.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["add", "list", "update", "complete", "delete"]
                    },
                    "id": {
                        "type": "string",
                        "description": "ID numérico del recordatorio (recomendado para 'update', 'complete' y 'delete')."
                    },
                    "text": {
                        "type": "string",
                        "description": "Descripción del recordatorio, o texto para buscar si no se indica 'id'."
                    },
                    "time_hint": {
                        "type": "string",
                        "description": "Fecha y hora calculada en formato 'YYYY-MM-DD HH:MM' según la fecha actual (ej: '2026-11-13 17:00') o referencia ('hoy 17hs', 'mañana 10hs')."
                    },
                    "recurrence": {
                        "type": "string",
                        "description": "Recurrencia opcional: 'none' (único), 'daily', 'weekdays', 'weekly:mon|tue|wed|thu|fri|sat|sun', 'monthly' (ej: 'todos los viernes' -> 'weekly:fri')."
                    }
                },
                "required": ["action"]
            }),
        }
    }

    fn manage_google_calendar() -> ToolDefinition {
        ToolDefinition {
            name: "manage_google_calendar",
            description: "Consulta, crea, modifica o elimina eventos reales en Google Calendar del usuario (requiere cuenta de Google vinculada en Settings → Cuentas Conectadas).",
            parameters: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["status", "list", "create", "update", "delete"],
                        "description": "'status' verifica si Google está conectado; 'list' lista eventos próximos; 'create' crea un evento; 'update' modifica por event_id; 'delete' cancela por event_id."
                    },
                    "days": {
                        "type": "integer",
                        "description": "Días hacia adelante a consultar en 'list' (por defecto 7, máximo 60)."
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Máximo de eventos a devolver en 'list' (por defecto 20, máximo 50)."
                    },
                    "query": {
                        "type": "string",
                        "description": "Filtro opcional de texto para buscar eventos en 'list'."
                    },
                    "event_id": {
                        "type": "string",
                        "description": "ID del evento de Google Calendar (obtenido con 'list'), requerido para 'update' y 'delete'."
                    },
                    "summary": {
                        "type": "string",
                        "description": "Título del evento para 'create' o 'update'."
                    },
                    "start": {
                        "type": "string",
                        "description": "Inicio en formato 'YYYY-MM-DD HH:MM', 'YYYY-MM-DD' (todo el día) o RFC3339."
                    },
                    "end": {
                        "type": "string",
                        "description": "Fin opcional en el mismo formato; si se omite en 'create', se asume 1 hora de duración."
                    },
                    "description": {
                        "type": "string",
                        "description": "Descripción o notas opcionales del evento."
                    },
                    "location": {
                        "type": "string",
                        "description": "Ubicación física o enlace de reunión opcional."
                    },
                    "attendees": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Lista opcional de emails de invitados."
                    },
                    "all_day": {
                        "type": "boolean",
                        "description": "Si es true, crea el evento como todo el día."
                    }
                },
                "required": ["action"]
            }),
        }
    }

    fn manage_whatsapp() -> ToolDefinition {
        ToolDefinition {
            name: "manage_whatsapp",
            description: "Lee chats/mensajes, gestiona agenda de contactos/apodos y envía mensajes por el WhatsApp conectado del usuario, respetando el limitador anti-spam y el modo de confirmación previa.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": [
                            "status",
                            "list_chats",
                            "read_messages",
                            "send_message",
                            "confirm_send",
                            "cancel_pending",
                            "list_pending",
                            "set_confirmation",
                            "save_contact",
                            "list_contacts"
                        ],
                        "description": "'status' muestra conexión y reglas; 'list_chats' lista conversaciones y contactos recientes (usar search para filtrar por nombre o teléfono); 'read_messages' lee un chat por nombre, alias o número; 'send_message' prepara o envía según la confirmación activa; 'confirm_send' envía un borrador pendiente tras confirmación explícita; 'cancel_pending' cancela borradores; 'set_confirmation' activa o desactiva pedir confirmación antes de enviar; 'save_contact' guarda un apodo/alias asociado a un teléfono; 'list_contacts' lista apodos guardados y contactos sincronizados de WhatsApp."
                    },
                    "search": {
                        "type": "string",
                        "description": "Filtro opcional de nombre o teléfono para 'list_chats' o 'list_contacts'."
                    },
                    "chat": {
                        "type": "string",
                        "description": "Nombre del contacto/grupo, alias, teléfono o JID para 'read_messages'."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Cantidad de chats, contactos o mensajes a devolver (por defecto 20)."
                    },
                    "to": {
                        "type": "string",
                        "description": "Destinatario para 'send_message' (nombre, alias, teléfono con código de país o JID) o teléfono/JID para 'save_contact'."
                    },
                    "message": {
                        "type": "string",
                        "description": "Texto del mensaje para 'send_message'."
                    },
                    "pending_id": {
                        "type": "string",
                        "description": "ID del borrador pendiente (ej. 'wa_ab12cd') para 'confirm_send' o 'cancel_pending' (o 'all' para cancelar todos)."
                    },
                    "require_confirmation": {
                        "type": "boolean",
                        "description": "Usado con 'set_confirmation': false si el usuario pide enviar directo sin confirmar; true para volver a pedir confirmación."
                    },
                    "alias": {
                        "type": "string",
                        "description": "Nombre corto o apodo para 'save_contact' (ej. 'Ana', 'mi pareja', 'Juan')."
                    },
                    "note": {
                        "type": "string",
                        "description": "Nota o relación opcional para 'save_contact' (ej. 'Pareja / Ana')."
                    }
                },
                "required": ["action"]
            }),
        }
    }

    // --- CORE-275: Specialist filesystem tools ---

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

    // --- CORE-273: ProjectLedger tool ---

    fn add_ledger_entry() -> ToolDefinition {
        ToolDefinition {
            name: "add_ledger_entry",
            description: "Record something important in the project's permanent history. Use for design decisions, completed milestones, or relevant findings that the user should be able to consult later.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "What to record. Plain language, any domain."
                    }
                },
                "required": ["content"]
            }),
        }
    }

    // --- CORE-276: Approve external path ---

    fn approve_path() -> ToolDefinition {
        ToolDefinition {
            name: "approve_path",
            description: "Approve an external path for filesystem access by specialists. Only call this after the user has explicitly authorized access to that path via ask_user.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to approve."
                    }
                },
                "required": ["path"]
            }),
        }
    }

    // --- CORE-272: Project ledger read for ChatAgent ---

    fn get_project_ledger() -> ToolDefinition {
        ToolDefinition {
            name: "get_project_ledger",
            description: "Get the history of a project: entries recorded by supervisors and exchanges with the user. Use ONLY when the user explicitly asks about a project's progress, decisions, or what was discussed with supervisors. Do not use proactively.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "project_name": {
                        "type": "string",
                        "description": "Name of the project to query."
                    }
                },
                "required": ["project_name"]
            }),
        }
    }

    // --- CORE-289: Agent status for ChatAgent ---

    fn get_agent_status() -> ToolDefinition {
        ToolDefinition {
            name: "get_agent_status",
            description: "Get the current status of all agents working on a project. \
                          Use this before spawning a new supervisor to check if one \
                          already exists. Use when the user asks about project progress.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "project_name": {
                        "type": "string",
                        "description": "Name of the project to check. Leave empty to get all active projects."
                    }
                },
                "required": []
            }),
        }
    }

    // --- CORE-FIX: shell verification for specialists ---

    fn execute_command() -> ToolDefinition {
        ToolDefinition {
            name: "execute_command",
            description:
                "Run a shell command for VERIFICATION (build, test, lint, status checks). \
                          Only whitelisted programs are allowed: cargo, rustc, npm, pnpm, yarn, \
                          git, python, python3, pytest, node, deno, bun, go, gradle, mvn, make, \
                          ls, echo, pwd, cat, head, tail. \
                          60s timeout. Output truncated to 8KB per stream. \
                          Use this to confirm your work compiles/passes tests, not to install \
                          packages or modify external state.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Full command line, e.g. 'cargo check -p my-crate' or 'git status'."
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Working directory relative to the workspace. Defaults to workspace root."
                    }
                },
                "required": ["command"]
            }),
        }
    }

    // --- CORE-277: Web search for specialists ---

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specialist_gets_filesystem_tools() {
        let role = AgentRole::Specialist {
            scope: "leer archivo".into(),
        };
        let tools = ToolRegistry::tools_for(&role, &ProviderKind::Anthropic);
        // CORE-FIX: Specialist tiene 6 tools (report + 3 filesystem + execute_command + web_search).
        // Names are now read from t["function"]["name"] because the serializer
        // emits OpenAI shape for every provider (we go through OpenAI-compat
        // endpoints for all of them, including Anthropic via OpenRouter).
        assert_eq!(tools.len(), 6);
        let names: Vec<&str> = tools
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"report"));
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"write_file"));
        assert!(names.contains(&"list_files"));
        assert!(names.contains(&"execute_command"));
        assert!(names.contains(&"web_search"));
    }

    #[test]
    fn test_supervisor_gets_all_tools() {
        let role = AgentRole::Supervisor {
            name: "Kernel".into(),
            scope: "kernel modules".into(),
        };
        let tools = ToolRegistry::tools_for(&role, &ProviderKind::OpenAI);
        // CORE-276 / CORE-FIX: supervisores tienen 11 tools (incluyendo ejecución directa)
        assert_eq!(tools.len(), 11);
        let names: Vec<&str> = tools
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"spawn_agent"));
        assert!(names.contains(&"query_agent"));
        assert!(names.contains(&"report"));
        assert!(names.contains(&"ask_user"));
        assert!(names.contains(&"add_ledger_entry"));
        assert!(names.contains(&"approve_path"));
    }

    #[test]
    fn test_chat_agent_gets_only_spawn() {
        // CORE-243: ChatAgent no tiene agent_id, por lo que query_agent y report
        // fallarían en SyscallExecutor. Solo recibe spawn_agent, answer_supervisor (CORE-263),
        // get_project_ledger (CORE-272), get_agent_status (CORE-289) y las 4 tools personales (CORE-375).
        let role = AgentRole::ChatAgent;
        let tools = ToolRegistry::tools_for(&role, &ProviderKind::Groq);
        assert_eq!(tools.len(), 13);
        let names: Vec<&str> = tools
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"spawn_agent"));
        assert!(names.contains(&"answer_supervisor"));
        assert!(names.contains(&"send_to_antigravity"));
        assert!(names.contains(&"manage_ai_provider"));
        assert!(names.contains(&"manage_modules"));
        assert!(names.contains(&"get_project_ledger"));
        assert!(names.contains(&"get_agent_status"));
        assert!(names.contains(&"log_expense_or_entry"));
        assert!(names.contains(&"query_ledger"));
        assert!(names.contains(&"remember_fact"));
        assert!(names.contains(&"manage_reminder"));
        assert!(names.contains(&"manage_google_calendar"));
        assert!(names.contains(&"manage_whatsapp"));
        assert!(
            !names.contains(&"query_agent"),
            "ChatAgent no debe recibir query_agent"
        );

        let provider_tool = tools
            .iter()
            .find(|t| t["function"]["name"] == "manage_ai_provider")
            .unwrap();
        let actions = provider_tool["function"]["parameters"]["properties"]["action"]["enum"]
            .as_array()
            .unwrap();
        assert!(actions.iter().any(|a| a == "catalog"));

        let rem_tool = tools
            .iter()
            .find(|t| t["function"]["name"] == "manage_reminder")
            .unwrap();
        let rem_actions = rem_tool["function"]["parameters"]["properties"]["action"]["enum"]
            .as_array()
            .unwrap();
        assert!(rem_actions.iter().any(|a| a == "update"));
        assert!(rem_actions.iter().any(|a| a == "delete"));
        assert!(rem_tool["function"]["parameters"]["properties"]
            .get("recurrence")
            .is_some());
    }

    #[test]
    fn test_anthropic_uses_openai_shape() {
        // CORE-FIX: Anthropic is reached via OpenRouter (OpenAI-compat), so
        // the tool shape must be OpenAI's `{ type, function: { name, ... } }`,
        // NOT the native `{ name, description, input_schema }`. Previously
        // this test asserted the native shape and the codepath actually used
        // the native shape too — both were wrong relative to the URL.
        let role = AgentRole::Specialist {
            scope: "test".into(),
        };
        let tools = ToolRegistry::tools_for(&role, &ProviderKind::Anthropic);
        assert!(
            tools.iter().all(|t| t["type"].as_str() == Some("function")),
            "OpenAI shape requires top-level type=function"
        );
        assert!(
            tools.iter().all(|t| t["function"]["name"].is_string()),
            "OpenAI shape requires function.name"
        );
    }

    #[test]
    fn test_gemini_uses_openai_shape() {
        // CORE-FIX: Gemini is reached via /v1beta/openai/chat/completions, so
        // it expects OpenAI tools, not Google's native `functionDeclarations`.
        // The previous code sent functionDeclarations and the upstream
        // returned 400 on every tool call.
        let role = AgentRole::Specialist {
            scope: "test".into(),
        };
        let tools = ToolRegistry::tools_for(&role, &ProviderKind::Gemini);
        assert!(
            tools.iter().all(|t| t["type"].as_str() == Some("function")),
            "Gemini OpenAI-compat shape requires top-level type=function"
        );
        assert!(
            tools.iter().all(|t| t["function"]["name"].is_string()),
            "Gemini OpenAI-compat shape requires function.name"
        );
        assert!(
            tools
                .iter()
                .all(|t| t.get("functionDeclarations").is_none()),
            "Gemini OpenAI-compat must NOT use Google's native functionDeclarations"
        );
    }

    #[test]
    fn test_provider_kind_from_string() {
        assert_eq!(
            ProviderKind::from_string("anthropic"),
            ProviderKind::Anthropic
        );
        assert_eq!(ProviderKind::from_string("ollama"), ProviderKind::Ollama);
        assert_eq!(ProviderKind::from_string("GROQ"), ProviderKind::Groq);
        assert_eq!(
            ProviderKind::from_string("unknown_provider"),
            ProviderKind::OpenAI
        );
    }

    #[test]
    fn test_core_376_multi_provider_tool_serialization() {
        let def = ToolDefinition {
            name: "test_calc",
            description: "Calculator",
            parameters: json!({"type": "object", "properties": {"expr": {"type": "string"}}}),
        };

        // Native Anthropic
        let anthropic = ToolRegistry::serialize_for_driver(&def, &ProviderKind::Anthropic, true);
        assert_eq!(anthropic["name"], "test_calc");
        assert!(anthropic.get("input_schema").is_some());
        assert!(anthropic.get("type").is_none());

        // Native Gemini
        let gemini = ToolRegistry::serialize_for_driver(&def, &ProviderKind::Gemini, true);
        assert_eq!(gemini["name"], "test_calc");
        assert!(gemini.get("parameters").is_some());
        assert!(gemini.get("type").is_none());

        // OpenAI format
        let openai = ToolRegistry::serialize_for_driver(&def, &ProviderKind::OpenAI, false);
        assert_eq!(openai["type"], "function");
        assert_eq!(openai["function"]["name"], "test_calc");
    }

    #[test]
    fn test_core_376_tool_deserialization() {
        // OpenAI payload
        let openai_payload = json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "id": "call_123",
                        "type": "function",
                        "function": {
                            "name": "spawn_agent",
                            "arguments": "{\"role\":\"supervisor\",\"scope\":\"audit\"}"
                        }
                    }]
                }
            }]
        });
        let calls =
            ToolRegistry::deserialize_tool_calls(&ProviderKind::OpenAI, &openai_payload).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_123");
        assert_eq!(calls[0].name, "spawn_agent");
        assert_eq!(calls[0].arguments["role"], "supervisor");

        // Anthropic payload
        let anthropic_payload = json!({
            "content": [{
                "type": "tool_use",
                "id": "toolu_abc",
                "name": "report",
                "input": { "status": "completed", "summary": "done" }
            }]
        });
        let calls =
            ToolRegistry::deserialize_tool_calls(&ProviderKind::Anthropic, &anthropic_payload)
                .unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "toolu_abc");
        assert_eq!(calls[0].name, "report");
        assert_eq!(calls[0].arguments["status"], "completed");

        // Gemini payload
        let gemini_payload = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "ask_user",
                            "args": { "question": "Are you sure?" }
                        }
                    }]
                }
            }]
        });
        let calls =
            ToolRegistry::deserialize_tool_calls(&ProviderKind::Gemini, &gemini_payload).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "ask_user");
        assert_eq!(calls[0].arguments["question"], "Are you sure?");
    }
}

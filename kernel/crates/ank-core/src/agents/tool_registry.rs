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
    OpenRouter,
    Xai,
    Mistral,
    DeepSeek,
    Qwen,
}

impl ProviderKind {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "anthropic" => Self::Anthropic,
            "openai" => Self::OpenAI,
            "groq" => Self::Groq,
            "gemini" => Self::Gemini,
            "ollama" => Self::Ollama,
            "openrouter" => Self::OpenRouter,
            "xai" => Self::Xai,
            "mistral" => Self::Mistral,
            "deepseek" => Self::DeepSeek,
            "qwen" => Self::Qwen,
            _ => Self::OpenAI,
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

    fn definitions_for(role: &AgentRole) -> Vec<ToolDefinition> {
        match role {
            // CORE-243: ChatAgent no tiene agent_id — query_agent y report requieren uno.
            // Solo puede hacer spawn_agent (para crear un ProjectSupervisor) y answer_supervisor (CORE-263).
            AgentRole::ChatAgent => vec![Self::spawn_agent(), Self::answer_supervisor()],
            AgentRole::ProjectSupervisor { .. } | AgentRole::Supervisor { .. } => {
                vec![
                    Self::spawn_agent(),
                    Self::query_agent(),
                    Self::report(),
                    Self::ask_user(),
                    Self::add_ledger_entry(),
                    Self::approve_path(),
                ]
            }
            AgentRole::Specialist { .. } => vec![
                Self::report(),
                Self::read_file(),
                Self::write_file(),
                Self::list_files(),
                Self::web_search(),
            ],
        }
    }

    /// Serializa una ToolDefinition al formato del proveedor.
    pub fn serialize(def: &ToolDefinition, provider: &ProviderKind) -> Value {
        match provider {
            ProviderKind::Anthropic => json!({
                "name": def.name,
                "description": def.description,
                "input_schema": def.parameters,
            }),
            ProviderKind::Gemini => json!({
                "functionDeclarations": [{
                    "name": def.name,
                    "description": def.description,
                    "parameters": def.parameters,
                }]
            }),
            // OpenAI / Groq / xAI / OpenRouter / Ollama / Mistral / DeepSeek / Qwen
            _ => json!({
                "type": "function",
                "function": {
                    "name": def.name,
                    "description": def.description,
                    "parameters": def.parameters,
                }
            }),
        }
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
        // CORE-277: Specialist ahora tiene 5 tools (report + 3 filesystem + web_search)
        assert_eq!(tools.len(), 5);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"report"));
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"write_file"));
        assert!(names.contains(&"list_files"));
        assert!(names.contains(&"web_search"));
    }

    #[test]
    fn test_supervisor_gets_all_tools() {
        let role = AgentRole::Supervisor {
            name: "Kernel".into(),
            scope: "kernel modules".into(),
        };
        let tools = ToolRegistry::tools_for(&role, &ProviderKind::OpenAI);
        // CORE-276: supervisores ahora tienen 6 tools (+ approve_path)
        assert_eq!(tools.len(), 6);
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
        // fallarían en SyscallExecutor. Solo recibe spawn_agent y answer_supervisor (CORE-263).
        let role = AgentRole::ChatAgent;
        let tools = ToolRegistry::tools_for(&role, &ProviderKind::Groq);
        assert_eq!(tools.len(), 2);
        let names: Vec<&str> = tools
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"spawn_agent"));
        assert!(names.contains(&"answer_supervisor"));
        assert!(
            !names.contains(&"query_agent"),
            "ChatAgent no debe recibir query_agent"
        );
    }

    #[test]
    fn test_anthropic_format() {
        let role = AgentRole::Specialist {
            scope: "test".into(),
        };
        let tools = ToolRegistry::tools_for(&role, &ProviderKind::Anthropic);
        // All tools in Anthropic format use input_schema
        assert!(
            tools.iter().all(|t| t.get("input_schema").is_some()),
            "Anthropic format must use input_schema"
        );
        assert!(
            tools.iter().all(|t| t.get("function").is_none()),
            "Anthropic format must not use function wrapper"
        );
    }

    #[test]
    fn test_gemini_format() {
        let role = AgentRole::Specialist {
            scope: "test".into(),
        };
        let tools = ToolRegistry::tools_for(&role, &ProviderKind::Gemini);
        assert!(
            tools[0].get("functionDeclarations").is_some(),
            "Gemini format must use functionDeclarations"
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
}

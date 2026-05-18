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
            AgentRole::ChatAgent => vec![
                Self::spawn_agent(),
                Self::answer_supervisor(),
                Self::get_project_ledger(),
                Self::get_agent_status(), // CORE-289
            ],
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
        // fallarían en SyscallExecutor. Solo recibe spawn_agent, answer_supervisor (CORE-263),
        // get_project_ledger (CORE-272) y get_agent_status (CORE-289).
        let role = AgentRole::ChatAgent;
        let tools = ToolRegistry::tools_for(&role, &ProviderKind::Groq);
        assert_eq!(tools.len(), 4);
        let names: Vec<&str> = tools
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"spawn_agent"));
        assert!(names.contains(&"answer_supervisor"));
        assert!(names.contains(&"get_project_ledger"));
        assert!(names.contains(&"get_agent_status"));
        assert!(
            !names.contains(&"query_agent"),
            "ChatAgent no debe recibir query_agent"
        );
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
}

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
/// - ChatAgent            → [spawn_agent]  (query_agent requiere agent_id — ver CORE-243)
/// - ProjectSupervisor    → [spawn_agent, query_agent, report]
/// - Supervisor           → [spawn_agent, query_agent, report]
/// - Specialist           → [report]
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
                ]
            }
            AgentRole::Specialist { .. } => vec![Self::report()],
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specialist_only_gets_report() {
        let role = AgentRole::Specialist {
            scope: "leer archivo".into(),
        };
        let tools = ToolRegistry::tools_for(&role, &ProviderKind::Anthropic);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "report");
    }

    #[test]
    fn test_supervisor_gets_all_tools() {
        let role = AgentRole::Supervisor {
            name: "Kernel".into(),
            scope: "kernel modules".into(),
        };
        let tools = ToolRegistry::tools_for(&role, &ProviderKind::OpenAI);
        // CORE-263: supervisores ahora tienen 4 tools (+ ask_user)
        assert_eq!(tools.len(), 4);
        let names: Vec<&str> = tools
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"spawn_agent"));
        assert!(names.contains(&"query_agent"));
        assert!(names.contains(&"report"));
        assert!(names.contains(&"ask_user"));
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
        assert!(
            tools[0].get("input_schema").is_some(),
            "Anthropic format must use input_schema"
        );
        assert!(
            tools[0].get("function").is_none(),
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

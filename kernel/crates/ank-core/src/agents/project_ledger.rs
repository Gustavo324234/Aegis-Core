use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Registro de avance persistente de un proyecto.
/// Sobrevive a la muerte de cualquier supervisor.
/// Ruta en disco: users/{tenant}/projects/{project_id}/project.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLedger {
    pub project_id: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Entradas en texto libre — sin categorías. El LLM sintetiza al leer.
    pub entries: Vec<LedgerEntry>,
    /// Intercambios usuario ↔ supervisores.
    pub user_exchanges: Vec<UserExchange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub id: Uuid,
    pub content: String,
    pub author: String,
    pub source_agent_role: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserExchange {
    pub id: Uuid,
    pub question: String,
    pub answer: Option<String>,
    pub context: Option<String>,
    pub agent_role: String,
    pub asked_at: DateTime<Utc>,
    pub answered_at: Option<DateTime<Utc>>,
}

impl ProjectLedger {
    pub fn new(project_id: String, display_name: String) -> Self {
        let now = Utc::now();
        Self {
            project_id,
            display_name,
            created_at: now,
            updated_at: now,
            entries: Vec::new(),
            user_exchanges: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, content: String, author: String, source_agent_role: String) {
        self.entries.push(LedgerEntry {
            id: Uuid::new_v4(),
            content,
            author,
            source_agent_role,
            timestamp: Utc::now(),
        });
        self.updated_at = Utc::now();
    }

    /// Formatea las entradas para inyectar en el system prompt del ProjectSupervisor.
    pub fn format_for_prompt(&self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }
        let lines: Vec<String> = self
            .entries
            .iter()
            .map(|e| format!("[{}] {}", e.timestamp.format("%Y-%m-%d %H:%M"), e.content))
            .collect();
        format!("[PROJECT HISTORY]\n{}", lines.join("\n"))
    }
}

/// Chat Agent context limiter — ventana deslizante + resumen automático (CORE-201).
///
/// El historial de conversación del Chat Agent es potencialmente infinito.
/// Este módulo mantiene una ventana deslizante de los últimos N tokens
/// y genera un resumen cuando se acerca al límite (ADR-CAA-002).
use serde::{Deserialize, Serialize};

/// Un turno de la conversación del Chat Agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub role: TurnRole,
    pub content: String,
    /// Estimación de tokens (content.len() / 4 como aproximación)
    pub estimated_tokens: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TurnRole {
    User,
    Assistant,
    System,
}

impl ConversationTurn {
    pub fn new(role: TurnRole, content: impl Into<String>) -> Self {
        let content = content.into();
        let estimated_tokens = content.len() / 4;
        Self {
            role,
            content,
            estimated_tokens,
        }
    }
}

/// Ventana deslizante del historial de conversación del Chat Agent.
///
/// Cuando el total de tokens supera `warn_threshold`, el caller debe
/// solicitar un resumen al LLM y llamar a `compact_with_summary()`.
#[derive(Debug)]
pub struct ChatContextWindow {
    turns: Vec<ConversationTurn>,
    /// Límite máximo de tokens para el historial (sin el system prompt).
    max_tokens: usize,
    /// Threshold a partir del cual se recomienda compactar (80% del max).
    warn_threshold: usize,
    /// Resumen compactado de turnos anteriores, si existe.
    compacted_summary: Option<String>,
}

impl ChatContextWindow {
    pub fn new(max_tokens: usize) -> Self {
        let warn_threshold = (max_tokens * 8) / 10;
        Self {
            turns: Vec::new(),
            max_tokens,
            warn_threshold,
            compacted_summary: None,
        }
    }

    /// Presupuesto por defecto para el Chat Agent (ADR-CAA-002).
    pub fn default_budget() -> Self {
        Self::new(4096)
    }

    pub fn push(&mut self, turn: ConversationTurn) {
        self.turns.push(turn);
    }

    /// Total de tokens estimados en la ventana actual.
    pub fn total_tokens(&self) -> usize {
        let summary_tokens = self.compacted_summary.as_deref().map(|s| s.len() / 4).unwrap_or(0);
        let turns_tokens: usize = self.turns.iter().map(|t| t.estimated_tokens).sum();
        summary_tokens + turns_tokens
    }

    /// true si se está acercando al límite y debería compactarse.
    pub fn should_compact(&self) -> bool {
        self.total_tokens() >= self.warn_threshold
    }

    /// true si supera el límite absoluto.
    pub fn is_over_budget(&self) -> bool {
        self.total_tokens() > self.max_tokens
    }

    /// Compacta los turnos más antiguos reemplazándolos por un resumen generado externamente.
    /// El caller es responsable de llamar al LLM para generar el `summary`.
    /// Se mantienen siempre los últimos `keep_recent` turnos sin compactar.
    pub fn compact_with_summary(&mut self, summary: String, keep_recent: usize) {
        let keep = keep_recent.min(self.turns.len());
        let _compacted_turns = &self.turns[..self.turns.len() - keep];

        // Combinar el resumen anterior con el nuevo si existe
        let new_summary = if let Some(prev) = self.compacted_summary.take() {
            format!("{}\n\n{}", prev, summary)
        } else {
            summary
        };

        self.compacted_summary = Some(new_summary);
        self.turns = self.turns[self.turns.len() - keep..].to_vec();
    }

    /// Retorna los turnos actuales listos para inyectar en el contexto del LLM.
    /// Si existe un resumen compactado, lo antepone como turno de sistema.
    pub fn build_context(&self) -> Vec<ConversationTurn> {
        let mut ctx = Vec::new();
        if let Some(summary) = &self.compacted_summary {
            ctx.push(ConversationTurn::new(
                TurnRole::System,
                format!("[Resumen de conversación anterior]\n{}", summary),
            ));
        }
        ctx.extend(self.turns.clone());
        ctx
    }

    pub fn turns(&self) -> &[ConversationTurn] {
        &self.turns
    }

    pub fn compacted_summary(&self) -> Option<&str> {
        self.compacted_summary.as_deref()
    }

    pub fn len(&self) -> usize {
        self.turns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }
}

/// Información que el Chat Agent debe preservar al compactar.
/// Puntos clave que no deben perderse en el resumen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionHints {
    /// Proyectos mencionados en la conversación.
    pub active_projects: Vec<String>,
    /// Tareas en curso al momento de compactar.
    pub tasks_in_progress: Vec<String>,
    /// Preferencias explícitas del usuario mencionadas.
    pub user_preferences: Vec<String>,
    /// Información personal relevante que el usuario compartió.
    pub user_context: Vec<String>,
}

impl CompactionHints {
    pub fn to_prompt_instruction(&self) -> String {
        let mut parts = Vec::new();
        if !self.active_projects.is_empty() {
            parts.push(format!("Proyectos activos: {}", self.active_projects.join(", ")));
        }
        if !self.tasks_in_progress.is_empty() {
            parts.push(format!("Tareas en curso: {}", self.tasks_in_progress.join(", ")));
        }
        if !self.user_preferences.is_empty() {
            parts.push(format!("Preferencias del usuario: {}", self.user_preferences.join("; ")));
        }
        if !self.user_context.is_empty() {
            parts.push(format!("Contexto del usuario: {}", self.user_context.join("; ")));
        }
        format!(
            "Resume la conversación anterior preservando obligatoriamente: {}",
            parts.join(". ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_token_counting() {
        let mut window = ChatContextWindow::new(1000);
        // 400 chars ≈ 100 tokens
        window.push(ConversationTurn::new(TurnRole::User, "a".repeat(400)));
        assert_eq!(window.total_tokens(), 100);
        assert!(!window.should_compact());
    }

    #[test]
    fn test_should_compact_at_80_pct() {
        let mut window = ChatContextWindow::new(100);
        // Agregar 81 tokens (> 80% de 100)
        window.push(ConversationTurn::new(TurnRole::User, "a".repeat(81 * 4)));
        assert!(window.should_compact());
    }

    #[test]
    fn test_compact_keeps_recent_turns() {
        let mut window = ChatContextWindow::new(1000);
        for i in 0..10 {
            window.push(ConversationTurn::new(TurnRole::User, format!("turn {}", i)));
        }
        window.compact_with_summary("Resumen de los primeros turnos.".to_string(), 3);
        assert_eq!(window.turns().len(), 3);
        assert!(window.compacted_summary().is_some());
    }

    #[test]
    fn test_build_context_includes_summary() {
        let mut window = ChatContextWindow::new(1000);
        window.push(ConversationTurn::new(TurnRole::User, "hola".to_string()));
        window.compact_with_summary("Resumen anterior.".to_string(), 1);
        window.push(ConversationTurn::new(TurnRole::Assistant, "respuesta".to_string()));

        let ctx = window.build_context();
        assert_eq!(ctx[0].role, TurnRole::System);
        assert!(ctx[0].content.contains("Resumen anterior"));
    }
}

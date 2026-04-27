use crate::agents::node::{AgentId, AgentRole};
use crate::pcb::TaskType;

/// Presupuesto de tokens por AgentNode (ADR-CAA-009).
/// El presupuesto no es global por tenant sino individual por nodo,
/// calibrado según el rol y la naturaleza cognitiva del trabajo.
#[derive(Debug, Clone)]
pub struct ContextBudget {
    /// Tokens máximos para el contexto de entrada del agente.
    pub max_tokens: usize,
    /// Tokens reservados para el system prompt + instrucciones de rol.
    pub system_reserve: usize,
    /// Tokens disponibles para archivos, memoria y reportes de hijos.
    pub available: usize,
}

impl ContextBudget {
    pub fn new(max_tokens: usize, system_reserve: usize) -> Self {
        let available = max_tokens.saturating_sub(system_reserve);
        Self {
            max_tokens,
            system_reserve,
            available,
        }
    }

    /// Presupuesto por defecto según el rol del agente.
    ///
    /// El Chat Agent tiene el presupuesto más bajo — su contexto es potencialmente infinito
    /// y debe usar ventana deslizante.
    /// Los Specialists tienen el presupuesto más alto — son efímeros y tienen trabajo concreto.
    pub fn for_role(role: &AgentRole, task_type: TaskType) -> Self {
        let (max, reserve) = match (role, task_type) {
            (AgentRole::ChatAgent, _) => (4096, 512),
            (AgentRole::ProjectSupervisor { .. }, _) => (16384, 1024),
            (AgentRole::Supervisor { .. }, TaskType::Code) => (32768, 1024),
            (AgentRole::Supervisor { .. }, _) => (16384, 1024),
            (AgentRole::Specialist { .. }, TaskType::Code) => (65536, 2048),
            (AgentRole::Specialist { .. }, _) => (32768, 2048),
        };
        Self::new(max, reserve)
    }

    /// Calcula cuántos tokens quedan después de descontar el uso actual.
    pub fn remaining(&self, used: usize) -> usize {
        self.available.saturating_sub(used)
    }

    pub fn is_within_budget(&self, used: usize) -> bool {
        used <= self.available
    }
}

/// Resumen de contexto de un AgentNode — lo que el orquestador expone hacia la UI
/// y usa el Chat Agent para responder preguntas sin lanzar una Query.
#[derive(Debug, Clone)]
pub struct AgentContextSummary {
    pub agent_id: AgentId,
    pub role_label: String,
    pub budget: ContextBudget,
    pub tokens_used: usize,
}

impl AgentContextSummary {
    pub fn utilization_pct(&self) -> f32 {
        if self.budget.available == 0 {
            return 100.0;
        }
        (self.tokens_used as f32 / self.budget.available as f32 * 100.0).min(100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_available() {
        let b = ContextBudget::new(8192, 512);
        assert_eq!(b.available, 7680);
        assert!(b.is_within_budget(7000));
        assert!(!b.is_within_budget(8000));
    }

    #[test]
    fn test_budget_for_role_ordering() {
        let chat = ContextBudget::for_role(&AgentRole::ChatAgent, TaskType::Chat);
        let spec =
            ContextBudget::for_role(&AgentRole::Specialist { scope: "x".into() }, TaskType::Code);
        // Specialist de código tiene más presupuesto que el Chat Agent
        assert!(spec.max_tokens > chat.max_tokens);
    }

    #[test]
    fn test_remaining() {
        let b = ContextBudget::new(8192, 512);
        assert_eq!(b.remaining(1000), 6680);
        assert_eq!(b.remaining(10000), 0); // saturating
    }
}

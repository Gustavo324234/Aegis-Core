    /// Ejecuta un AgentToolCall recibido del LLM via tool use (EPIC 47 — CORE-235).
    /// Retorna el resultado como JSON string para incluir en el historial como `tool_result`.
    pub async fn execute_agent_tool_call(
        &self,
        pcb: &crate::pcb::PCB,
        call: crate::agents::message::AgentToolCall,
    ) -> Result<String, SyscallError> {
        use crate::agents::message::AgentToolCall;
        use crate::agents::node::AgentRole;

        let orchestrator = self.agent_orchestrator.as_ref().ok_or_else(|| {
            SyscallError::InternalError(
                "AgentOrchestrator not configured — AgentToolCall unavailable".to_string(),
            )
        })?;

        match &call {
            AgentToolCall::Spawn { role, name, scope, task_type } => {
                match (pcb.agent_id.as_ref(), role) {
                    // Chat Agent (sin agent_id) crea ProjectSupervisor — caso raíz válido
                    (None, AgentRole::ProjectSupervisor) => {
                        let project_name = name
                            .clone()
                            .unwrap_or_else(|| scope.chars().take(40).collect());

                        orchestrator
                            .create_project(
                                project_name.clone(),
                                scope.clone(),
                                task_type.clone(),
                                pcb.tenant_id.clone(),
                            )
                            .await
                            .map_err(|e| SyscallError::InternalError(e.to_string()))?;

                        return Ok(format!(
                            "{{\"status\":\"spawned\",\"project\":\"{}\"}}",
                            project_name
                        ));
                    }

                    // Agente del árbol crea hijo — caso normal válido
                    (Some(caller_id), AgentRole::Supervisor | AgentRole::Specialist) => {
                        orchestrator
                            .handle_tool_call(*caller_id, call)
                            .await
                            .map_err(|e| SyscallError::InternalError(e.to_string()))
                    }

                    // Chat Agent intentando crear Supervisor/Specialist directamente — inválido
                    (None, _) => Err(SyscallError::InternalError(
                        "Chat Agent can only spawn ProjectSupervisors — use role=\"project_supervisor\"".to_string(),
                    )),

                    // Agente del árbol intentando crear ProjectSupervisor — inválido
                    (Some(_), AgentRole::ProjectSupervisor) => Err(SyscallError::InternalError(
                        "Only the Chat Agent can create ProjectSupervisors".to_string(),
                    )),
                }
            }

            // Query y Report: requieren agent_id (no aplican al Chat Agent)
            AgentToolCall::Query { .. } | AgentToolCall::Report { .. } => {
                let caller_id = pcb.agent_id.ok_or_else(|| {
                    SyscallError::InternalError(
                        "Query and Report require an active agent_id in PCB".to_string(),
                    )
                })?;

                orchestrator
                    .handle_tool_call(caller_id, call)
                    .await
                    .map_err(|e| SyscallError::InternalError(e.to_string()))
            }
        }
    }
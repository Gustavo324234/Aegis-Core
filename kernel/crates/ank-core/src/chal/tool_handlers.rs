use super::*;

impl CognitiveHAL {
    // --- CORE-275: Filesystem helpers ---

    pub(super) fn get_tenant_workspace(pcb: &PCB) -> std::path::PathBuf {
        let data_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
        let tenant = pcb.tenant_id.as_deref().unwrap_or("default");
        std::path::Path::new(&data_dir)
            .join("users")
            .join(tenant)
            .join("workspace")
    }

    pub(super) fn valid_pcb_session_key(pcb: &PCB) -> Option<&str> {
        pcb.session_key
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty() && *k != "default")
    }

    pub(super) async fn get_approved_paths(pcb: &PCB) -> Vec<String> {
        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
        let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
            return vec![];
        };
        match crate::enclave::TenantDB::open(tenant_id, session_key) {
            Ok(db) => db.get_approved_paths().unwrap_or_default(),
            Err(_) => vec![],
        }
    }

    /// CORE-276 (autonomous mode): whether the project the calling agent belongs
    /// to is marked autonomous — i.e. the user opted this project into skipping
    /// the external-path approval gate (full filesystem access, no per-path
    /// prompts). Resolves the agent's project via the orchestrator tree, then
    /// checks the tenant enclave. Defaults to false (locked down) on any miss.
    pub(super) async fn is_project_autonomous(&self, pcb: &PCB) -> bool {
        let agent_id = match pcb.agent_id {
            Some(id) => id,
            None => return false,
        };
        let orchestrator = match self.agent_orchestrator.read().await.clone() {
            Some(o) => o,
            None => return false,
        };
        let project_id = {
            let tree = orchestrator.tree.read().await;
            match tree.get(&agent_id) {
                Some(node) => node.project_id.clone(),
                None => return false,
            }
        };
        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
        let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
            return false;
        };
        match crate::enclave::TenantDB::open(tenant_id, session_key) {
            Ok(db) => db.is_project_autonomous(&project_id),
            Err(_) => false,
        }
    }

    pub(super) fn strip_unc_prefix(path: &std::path::Path) -> std::path::PathBuf {
        let path_str = path.to_string_lossy();
        if let Some(stripped) = path_str.strip_prefix(r"\\?\") {
            if let Some(unc_stripped) = stripped.strip_prefix("UNC\\") {
                std::path::PathBuf::from(format!(r"\\{}", unc_stripped))
            } else {
                std::path::PathBuf::from(stripped)
            }
        } else {
            path.to_path_buf()
        }
    }

    pub(super) fn resolve_path(
        workspace: &std::path::Path,
        input_path: &str,
        approved_paths: &[String],
        autonomous: bool,
    ) -> Result<std::path::PathBuf, String> {
        let candidate = if std::path::Path::new(input_path).is_absolute() {
            std::path::PathBuf::from(input_path)
        } else {
            workspace.join(input_path)
        };

        // Canonicalizar para resolver .. y symlinks.
        // Si el archivo no existe aún (write_file), canonicalizar el padre.
        let canonical = if candidate.exists() {
            candidate
                .canonicalize()
                .map_err(|e| format!("{{\"error\":\"invalid_path\",\"detail\":\"{}\"}}", e))?
        } else {
            let parent = candidate.parent().ok_or_else(|| {
                "{\"error\":\"invalid_path\",\"detail\":\"no parent\"}".to_string()
            })?;
            let canonical_parent = parent
                .canonicalize()
                .unwrap_or_else(|_| parent.to_path_buf());
            canonical_parent.join(candidate.file_name().unwrap_or_default())
        };

        // CORE-FIX: canonicalize workspace BEFORE comparing. Without this, a
        // workspace that contains a symlink anywhere in its path (e.g. /var/lib
        // is a symlink to /mnt/data/lib on the host) produces a canonical
        // candidate under /mnt/data/lib/... that does NOT start_with the
        // original /var/lib/... — and we'd incorrectly reject legitimate paths.
        let workspace_canonical = workspace
            .canonicalize()
            .unwrap_or_else(|_| workspace.to_path_buf());

        let canonical_clean = Self::strip_unc_prefix(&canonical);
        let workspace_canonical_clean = Self::strip_unc_prefix(&workspace_canonical);

        if canonical_clean.starts_with(&workspace_canonical_clean) {
            return Ok(canonical);
        }

        // Autonomous project: the user opted this project into full filesystem
        // access, so external paths are allowed without per-path approval.
        if autonomous {
            return Ok(canonical);
        }

        // Path externo — verificar aprobaciones
        let canonical_str = canonical_clean.to_string_lossy().to_string();
        let approved = approved_paths.iter().any(|a| {
            let approved_clean = Self::strip_unc_prefix(std::path::Path::new(a));
            canonical_clean.starts_with(&approved_clean)
        });

        if approved {
            Ok(canonical)
        } else {
            Err(format!(
                "{{\"error\":\"path_requires_approval\",\"path\":\"{}\",\"message\":\"Este path está fuera del workspace. El usuario debe aprobarlo primero.\"}}",
                canonical_str
            ))
        }
    }

    /// CORE-261: Ejecuta un tool call interno y retorna el resultado como String.
    pub(super) async fn execute_tool_call_internal(
        &self,
        tc: &ToolCallRecord,
        pid: &str,
        pcb: &PCB,
    ) -> String {
        self.execute_tool_call_with_progress(tc, pid, pcb, None)
            .await
    }

    pub(super) async fn execute_tool_call_with_progress(
        &self,
        tc: &ToolCallRecord,
        _pid: &str,
        pcb: &PCB,
        progress_tx: Option<&tokio::sync::mpsc::UnboundedSender<Result<String, ExecutionError>>>,
    ) -> String {
        let mut args: serde_json::Value =
            serde_json::from_str(&tc.function.arguments).unwrap_or_default();

        match tc.function.name.as_str() {
            "spawn_agent" => {
                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                match orchestrator_opt {
                    None => "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                    Some(orchestrator) => {
                        let name = args["name"].as_str().map(String::from);
                        let scope = args["scope"].as_str().unwrap_or("").to_string();
                        let task_type_str = args["task_type"].as_str();
                        let task_type = task_type_str
                            .map(crate::syscalls::parse_task_type)
                            .or_else(|| {
                                if !scope.trim().is_empty() {
                                    Some(crate::pcb::infer_task_type(&scope))
                                } else {
                                    Some(crate::pcb::TaskType::Planning)
                                }
                            });

                        if let Some(caller_id) = pcb.agent_id {
                            let role_str = args["role"].as_str().unwrap_or("").to_string();
                            let agent_role = match role_str.to_lowercase().as_str() {
                                "project_supervisor" => {
                                    crate::agents::node::AgentRole::ProjectSupervisor {
                                        name: name.clone().unwrap_or_else(|| scope.clone()),
                                        description: scope.clone(),
                                    }
                                }
                                "supervisor" => crate::agents::node::AgentRole::Supervisor {
                                    name: name.clone().unwrap_or_else(|| scope.clone()),
                                    scope: scope.clone(),
                                },
                                _ => crate::agents::node::AgentRole::Specialist {
                                    scope: scope.clone(),
                                },
                            };

                            let call = crate::agents::message::AgentToolCall::Spawn {
                                role: agent_role,
                                name,
                                scope,
                                task_type,
                            };

                            match orchestrator.handle_tool_call(caller_id, call).await {
                                Ok(result) => result,
                                Err(e) => format!("{{\"error\":\"{}\"}}", e),
                            }
                        } else {
                            let project_name = name
                                .clone()
                                .unwrap_or_else(|| scope.chars().take(40).collect());

                            match orchestrator
                                .create_project(
                                    project_name.clone(),
                                    scope.clone(),
                                    task_type,
                                    pcb.tenant_id.clone(),
                                )
                                .await
                            {
                                Ok(agent_id) => {
                                    let task = if !scope.trim().is_empty() {
                                        scope.clone()
                                    } else {
                                        pcb.memory_pointers.l1_instruction.clone()
                                    };
                                    if !task.is_empty() {
                                        if let Err(e) =
                                            orchestrator.dispatch(agent_id, task, vec![]).await
                                        {
                                            tracing::warn!(
                                                agent = %agent_id,
                                                "CORE-264: dispatch post-spawn falló: {}",
                                                e
                                            );
                                        } else {
                                            tracing::info!(
                                                agent = %agent_id,
                                                project = %project_name,
                                                "CORE-264: Dispatch automático enviado al supervisor recién creado."
                                            );
                                        }
                                    }
                                    // CORE-EMIT: Actualizar inmediatamente el árbol en la UI para que aparezca en el Dashboard
                                    let tenant = pcb.tenant_id.as_deref().unwrap_or("default");
                                    let nodes = orchestrator.tree_snapshot_for_tenant(tenant).await;
                                    orchestrator.emit_event(
                                        crate::agents::event::AgentEvent::TreeSnapshot { nodes },
                                    );

                                    format!(
                                        "{{\"status\":\"spawned\",\"project\":\"{}\",\"agent_id\":\"{}\"}}",
                                        project_name,
                                        agent_id
                                    )
                                }
                                Err(e) => format!("{{\"error\":\"{}\"}}", e),
                            }
                        }
                    }
                }
            }

            "query_agent" => {
                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                match orchestrator_opt {
                    None => "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                    Some(orchestrator) => {
                        let caller_id = match pcb.agent_id {
                            Some(id) => id,
                            None => {
                                return "{\"error\":\"query_agent requiere agent_id en PCB\"}"
                                    .to_string()
                            }
                        };
                        let project = args["project"].as_str().unwrap_or("").to_string();
                        let question = args["question"].as_str().unwrap_or("").to_string();
                        let call =
                            crate::agents::message::AgentToolCall::Query { project, question };
                        match orchestrator.handle_tool_call(caller_id, call).await {
                            Ok(result) => result,
                            Err(e) => format!("{{\"error\":\"{}\"}}", e),
                        }
                    }
                }
            }

            "call_plugin" => {
                let plugin_name = args["plugin_name"].as_str().unwrap_or("").to_string();
                let plugin_args = args["args"]
                    .as_object()
                    .map(|o| serde_json::Value::Object(o.clone()).to_string())
                    .unwrap_or_else(|| "{}".to_string());
                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                let pm = self.plugin_manager.read().await;
                match pm
                    .execute_plugin(
                        tenant_id,
                        &plugin_name,
                        &plugin_args,
                        pcb.session_key.as_deref(),
                    )
                    .await
                {
                    Ok(result) => result,
                    // CORE-253: mensaje legible al usuario cuando el plugin no existe.
                    Err(crate::plugins::PluginError::FunctionNotFound(_)) => format!(
                        "El plugin '{}' no está instalado o no está activo en este tenant. \
                         Podés activarlo desde Configuración → Plugins.",
                        plugin_name
                    ),
                    Err(e) => format!("{{\"error\":\"{}\"}}", e),
                }
            }

            // CORE-263: supervisor pausa su ejecución esperando respuesta del usuario
            "ask_user" => {
                let question = args["question"].as_str().unwrap_or("").to_string();
                let context = args
                    .get("context")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                let agent_uuid = match pcb.agent_id {
                    Some(id) => id,
                    None => return "{\"error\":\"ask_user requiere agent_id en PCB\"}".to_string(),
                };

                // Autonomous project: don't pause for the user. Auto-approve so the
                // supervisor proceeds without per-question permission prompts — the
                // behavioural half of "omitir permisos por proyecto" (the gate
                // bypass in resolve_path is the filesystem half).
                if self.is_project_autonomous(pcb).await {
                    tracing::info!(
                        agent = %agent_uuid,
                        question = %question,
                        "ask_user auto-approved — project is in autonomous mode"
                    );
                    return serde_json::json!({
                        "user_answer": "Modo autónomo activo en este proyecto: tenés acceso completo, procedé sin pedir permiso."
                    })
                    .to_string();
                }

                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                let orchestrator = match orchestrator_opt {
                    Some(o) => o,
                    None => return "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                };

                let (reply_tx, reply_rx) = tokio::sync::oneshot::channel::<String>();
                orchestrator.register_user_reply(agent_uuid, reply_tx).await;

                let project_name = {
                    let tree = orchestrator.tree.read().await;
                    tree.get(&agent_uuid)
                        .map(|n| n.project_id.clone())
                        .unwrap_or_else(|| "proyecto".to_string())
                };

                {
                    let mut tree = orchestrator.tree.write().await;
                    if let Some(node) = tree.get_mut(&agent_uuid) {
                        node.set_state(crate::agents::node::AgentState::WaitingUser);
                    }
                }

                tracing::info!(
                    agent = %agent_uuid,
                    question = %question,
                    "CORE-263: supervisor pausado esperando respuesta del usuario"
                );

                // CORE-268: notificar a la UI que el supervisor necesita respuesta.
                // Guardamos también el evento para re-enviarlo si el usuario
                // reconecta el WebSocket mientras el supervisor sigue esperando.
                let question_event = crate::agents::event::AgentEvent::SupervisorQuestion {
                    agent_id: agent_uuid,
                    project_name: project_name.clone(),
                    question: question.clone(),
                    context: context.clone(),
                    timestamp: chrono::Utc::now(),
                };
                orchestrator
                    .set_pending_question(agent_uuid, question_event.clone())
                    .await;
                orchestrator.emit_event(question_event);

                match tokio::time::timeout(std::time::Duration::from_secs(600), reply_rx).await {
                    Ok(Ok(answer)) => {
                        orchestrator.clear_pending_question(&agent_uuid).await;
                        {
                            let mut tree = orchestrator.tree.write().await;
                            if let Some(node) = tree.get_mut(&agent_uuid) {
                                node.set_state(crate::agents::node::AgentState::Running);
                            }
                        }
                        orchestrator.emit_event(
                            crate::agents::event::AgentEvent::SupervisorResumed {
                                agent_id: agent_uuid,
                            },
                        );
                        format!("{{\"user_answer\": {}}}", serde_json::json!(answer))
                    }
                    _ => {
                        orchestrator.clear_pending_question(&agent_uuid).await;
                        {
                            let mut tree = orchestrator.tree.write().await;
                            if let Some(node) = tree.get_mut(&agent_uuid) {
                                node.set_state(crate::agents::node::AgentState::Running);
                            }
                        }
                        orchestrator.emit_event(
                            crate::agents::event::AgentEvent::SupervisorTimedOut {
                                agent_id: agent_uuid,
                                project_name,
                            },
                        );
                        "{\"user_answer\": null, \"reason\": \"timeout\"}".to_string()
                    }
                }
            }

            // CORE-276: supervisor aprueba un path externo tras recibir OK del usuario
            "approve_path" => {
                let path = args["path"].as_str().unwrap_or("").to_string();

                if path.is_empty() {
                    return "{\"error\":\"empty_path\"}".to_string();
                }

                if !std::path::Path::new(&path).exists() {
                    return format!("{{\"error\":\"path_not_found\",\"path\":\"{}\"}}", path);
                }

                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };
                let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                    return "{\"error\":\"missing_session_key\"}".to_string();
                };

                match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                    Ok(db) => match db.add_approved_path(&path) {
                        Ok(_) => serde_json::json!({
                            "status": "approved",
                            "path": path
                        })
                        .to_string(),
                        Err(e) => {
                            format!("{{\"error\":\"persist_failed\",\"detail\":\"{}\"}}", e)
                        }
                    },
                    Err(e) => {
                        format!("{{\"error\":\"enclave_open_failed\",\"detail\":\"{}\"}}", e)
                    }
                }
            }

            // CORE-277: búsqueda web para specialists
            "web_search" => {
                let query = args["query"].as_str().unwrap_or("").to_string();
                let max_results = args
                    .get("max_results")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(5)
                    .min(10) as usize;

                if query.is_empty() {
                    return "{\"error\":\"empty_query\"}".to_string();
                }

                let api_key = match std::env::var("BRAVE_SEARCH_API_KEY").ok() {
                    Some(k) if !k.is_empty() => k,
                    _ => {
                        return "{\"error\":\"web_search_not_configured\",\"detail\":\"BRAVE_SEARCH_API_KEY not set\"}".to_string();
                    }
                };

                match self
                    .http_client
                    .get("https://api.search.brave.com/res/v1/web/search")
                    .header("X-Subscription-Token", api_key)
                    .header("Accept", "application/json")
                    .query(&[("q", &query), ("count", &max_results.to_string())])
                    .send()
                    .await
                {
                    Ok(resp) => match resp.json::<serde_json::Value>().await {
                        Ok(data) => {
                            let empty = vec![];
                            let raw = data["web"]["results"].as_array().unwrap_or(&empty);
                            let results: Vec<serde_json::Value> = raw
                                .iter()
                                .take(max_results)
                                .map(|r| {
                                    serde_json::json!({
                                        "title": r["title"],
                                        "url": r["url"],
                                        "snippet": r["description"]
                                    })
                                })
                                .collect();
                            let count = results.len();
                            serde_json::json!({
                                "results": results,
                                "query": query,
                                "count": count
                            })
                            .to_string()
                        }
                        Err(e) => {
                            format!("{{\"error\":\"parse_failed\",\"detail\":\"{}\"}}", e)
                        }
                    },
                    Err(e) => format!("{{\"error\":\"search_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            // CORE-263: Chat Agent entrega la respuesta del usuario al supervisor pausado
            "answer_supervisor" => {
                let agent_id_str = args["agent_id"].as_str().unwrap_or("");
                let answer = args["answer"].as_str().unwrap_or("").to_string();

                let agent_uuid = match agent_id_str.parse::<uuid::Uuid>() {
                    Ok(id) => id,
                    Err(_) => return "{\"error\":\"agent_id inválido\"}".to_string(),
                };

                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                match orchestrator_opt {
                    None => "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                    Some(orchestrator) => {
                        if orchestrator.answer_user_question(agent_uuid, answer).await {
                            "{\"status\":\"answer_delivered\"}".to_string()
                        } else {
                            "{\"status\":\"no_supervisor_waiting\"}".to_string()
                        }
                    }
                }
            }

            // CORE-275: Specialist filesystem tools
            "read_file" => {
                let input_path = args["path"].as_str().unwrap_or("").to_string();
                let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let length = args.get("length").and_then(|v| v.as_u64()).unwrap_or(200) as usize;

                let workspace = Self::get_tenant_workspace(pcb);
                let approved = Self::get_approved_paths(pcb).await;
                let autonomous = self.is_project_autonomous(pcb).await;

                let resolved =
                    match Self::resolve_path(&workspace, &input_path, &approved, autonomous) {
                        Ok(p) => p,
                        Err(e) => return e,
                    };

                match tokio::fs::read_to_string(&resolved).await {
                    Ok(content) => {
                        let lines: Vec<&str> = content.lines().collect();
                        let total = lines.len();
                        let start = offset.min(total);
                        let end = (offset + length).min(total);
                        let slice = &lines[start..end];
                        serde_json::json!({
                            "content": slice.join("\n"),
                            "total_lines": total,
                            "offset": offset,
                            "returned_lines": slice.len()
                        })
                        .to_string()
                    }
                    Err(e) => format!("{{\"error\":\"read_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            "write_file" => {
                use tokio::io::AsyncWriteExt;

                let input_path = args["path"].as_str().unwrap_or("").to_string();
                let content = args["content"].as_str().unwrap_or("").to_string();
                let mode = args
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("rewrite");

                let workspace = Self::get_tenant_workspace(pcb);
                let autonomous = self.is_project_autonomous(pcb).await;

                // write_file stays inside the workspace — unless the project is
                // autonomous, in which case external writes are allowed too.
                let resolved = match Self::resolve_path(&workspace, &input_path, &[], autonomous) {
                    Ok(p) => p,
                    Err(_) => {
                        return "{\"error\":\"write_outside_workspace\",\"message\":\"write_file solo puede escribir dentro del workspace del tenant.\"}".to_string();
                    }
                };

                // Crear directorios intermedios si es necesario
                if let Some(parent) = resolved.parent() {
                    if let Err(e) = tokio::fs::create_dir_all(parent).await {
                        return format!("{{\"error\":\"mkdir_failed\",\"detail\":\"{}\"}}", e);
                    }
                }

                let result = if mode == "append" {
                    match tokio::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&resolved)
                        .await
                    {
                        Ok(mut f) => f.write_all(content.as_bytes()).await,
                        Err(e) => Err(e),
                    }
                } else {
                    tokio::fs::write(&resolved, content.as_bytes()).await
                };

                match result {
                    Ok(_) => serde_json::json!({
                        "status": "written",
                        "path": resolved.to_string_lossy()
                    })
                    .to_string(),
                    Err(e) => format!("{{\"error\":\"write_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            "list_files" => {
                let input_path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let depth = args
                    .get("depth")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(2)
                    .min(4) as usize;

                let workspace = Self::get_tenant_workspace(pcb);
                let approved = Self::get_approved_paths(pcb).await;
                let autonomous = self.is_project_autonomous(pcb).await;

                let resolved =
                    match Self::resolve_path(&workspace, input_path, &approved, autonomous) {
                        Ok(p) => p,
                        Err(e) => return e,
                    };

                fn walk(dir: &std::path::Path, max_depth: usize, current: usize) -> Vec<String> {
                    if current > max_depth {
                        return vec![];
                    }
                    let mut entries = vec![];
                    if let Ok(rd) = std::fs::read_dir(dir) {
                        let mut items: Vec<_> = rd.flatten().collect();
                        items.sort_by_key(|e| e.file_name());
                        for entry in items {
                            let path = entry.path();
                            let name = path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            if name.starts_with('.') || name == "target" || name == "node_modules" {
                                continue;
                            }
                            if path.is_dir() {
                                entries.push(format!("[DIR]  {}", name));
                                for s in walk(&path, max_depth, current + 1) {
                                    entries.push(format!("  {}", s));
                                }
                            } else {
                                entries.push(format!("[FILE] {}", name));
                            }
                        }
                    }
                    entries
                }

                let listing = walk(&resolved, depth, 0);
                serde_json::json!({
                    "path": resolved.to_string_lossy(),
                    "entries": listing
                })
                .to_string()
            }

            // CORE-272: Chat Agent consulta el ledger de un proyecto por nombre
            "get_project_ledger" => {
                let project_name = args["project_name"].as_str().unwrap_or("").to_string();

                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                let orchestrator = match orchestrator_opt {
                    Some(o) => o,
                    None => return "{\"error\":\"orchestrator_not_configured\"}".to_string(),
                };

                let project_id = {
                    let tree = orchestrator.tree.read().await;
                    tree.all_nodes()
                        .iter()
                        .filter_map(|n| {
                            if let crate::agents::node::AgentRole::ProjectSupervisor {
                                name, ..
                            } = &n.role
                            {
                                if name.to_lowercase().contains(&project_name.to_lowercase()) {
                                    return Some(n.project_id.clone());
                                }
                            }
                            None
                        })
                        .next()
                };

                let project_id = match project_id {
                    Some(id) => id,
                    None => {
                        return format!(
                            "{{\"error\":\"project_not_found\",\"project\":\"{}\"}}",
                            project_name
                        )
                    }
                };

                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };

                let persistence = crate::agents::persistence::AgentPersistence::from_env();
                match persistence.load_ledger(&tenant_id, &project_id) {
                    Ok(Some(ledger)) => serde_json::to_string(&ledger)
                        .unwrap_or_else(|_| "{\"error\":\"serialization_error\"}".to_string()),
                    Ok(None) => format!(
                        "{{\"error\":\"no_ledger\",\"project_id\":\"{}\"}}",
                        project_id
                    ),
                    Err(e) => format!("{{\"error\":\"load_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            // CORE-273: Supervisores escriben al ledger del proyecto
            "add_ledger_entry" => {
                let content = args["content"].as_str().unwrap_or("").to_string();
                if content.is_empty() {
                    return "{\"error\":\"content is required\"}".to_string();
                }

                let agent_uuid = match pcb.agent_id {
                    Some(id) => id,
                    None => {
                        return "{\"error\":\"add_ledger_entry requiere agent_id en PCB\"}"
                            .to_string()
                    }
                };

                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                let orchestrator = match orchestrator_opt {
                    Some(o) => o,
                    None => return "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                };

                let tenant_id = pcb
                    .tenant_id
                    .clone()
                    .unwrap_or_else(|| "default".to_string());

                match orchestrator
                    .add_project_ledger_entry(&tenant_id, agent_uuid, content)
                    .await
                {
                    Ok(_) => "{\"status\":\"recorded\"}".to_string(),
                    Err(e) => format!("{{\"error\":\"{}\"}}", e),
                }
            }

            // CORE-289: Chat Agent consulta el estado actual del árbol de agentes
            // CORE-300: filtrar por tenant_id del PCB — aislamiento cross-tenant.
            "get_agent_status" => {
                let project_name = args
                    .get("project_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                let orchestrator = match orchestrator_opt {
                    Some(o) => o,
                    None => return "{\"error\":\"orchestrator_not_configured\"}".to_string(),
                };

                let tenant_id = pcb
                    .tenant_id
                    .clone()
                    .unwrap_or_else(|| "default".to_string());
                let snapshot = orchestrator.tree_snapshot_for_tenant(&tenant_id).await;

                let filtered: Vec<_> = if project_name.is_empty() {
                    snapshot
                } else {
                    snapshot
                        .into_iter()
                        .filter(|n| {
                            n.project_id
                                .to_lowercase()
                                .contains(&project_name.to_lowercase())
                                || n.role_label
                                    .to_lowercase()
                                    .contains(&project_name.to_lowercase())
                        })
                        .collect()
                };

                if filtered.is_empty() {
                    return serde_json::json!({
                        "status": "no_active_agents",
                        "project": project_name,
                        "message": "No active agents for this project. You can spawn a new supervisor."
                    })
                    .to_string();
                }

                serde_json::to_string(&serde_json::json!({
                    "agents": filtered.iter().map(|n| serde_json::json!({
                        "role": n.role_label,
                        "state": n.state,
                        "project": n.project_id,
                        "last_report": n.last_report,
                        "degraded": n.degraded,
                    })).collect::<Vec<_>>()
                }))
                .unwrap_or_else(|_| "{\"error\":\"serialization_error\"}".to_string())
            }

            // CORE-FIX: Antes no existía handler para `report` — la tool definida en
            // ToolRegistry caía en el fallback "Unknown tool: report" y los agentes
            // del árbol no podían reportar status=error|blocked al supervisor padre.
            // Ahora delegamos a AgentOrchestrator::handle_tool_call que ya tiene la
            // lógica para setear AgentState y last_report en el nodo.
            "report" => {
                let caller_id = match pcb.agent_id {
                    Some(id) => id,
                    None => {
                        return "{\"error\":\"report requires agent_id in PCB (only tree agents can report)\"}".to_string();
                    }
                };
                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                let orchestrator = match orchestrator_opt {
                    Some(o) => o,
                    None => return "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                };

                let status_str = args["status"].as_str().unwrap_or("completed");
                let status = match status_str.to_lowercase().as_str() {
                    "error" => crate::agents::message::ToolCallReportStatus::Error,
                    "blocked" => crate::agents::message::ToolCallReportStatus::Blocked,
                    _ => crate::agents::message::ToolCallReportStatus::Completed,
                };
                let summary = args["summary"].as_str().unwrap_or("").to_string();
                let observations = args
                    .get("observations")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                let call = crate::agents::message::AgentToolCall::Report {
                    status,
                    summary,
                    observations,
                };
                match orchestrator.handle_tool_call(caller_id, call).await {
                    Ok(result) => result,
                    Err(e) => format!("{{\"error\":\"{}\"}}", e),
                }
            }

            // CORE-FIX: Specialists declaran "Verify the result (build, test, lint)"
            // en su prompt pero antes no tenían ninguna tool para hacerlo. Esta tool
            // permite ejecutar comandos de verificación con una whitelist estricta,
            // timeout de 60s y output truncado a 8KB. No es un shell general.
            "execute_command" => {
                const ALLOWED_PROGRAMS: &[&str] = &[
                    "cargo", "rustc", "npm", "pnpm", "yarn", "git", "python", "python3", "pytest",
                    "node", "deno", "bun", "go", "gradle", "mvn", "make", "ls", "echo", "pwd",
                    "cat", "head", "tail", "protoc",
                ];
                let timeout_secs = std::env::var("AEGIS_COMMAND_TIMEOUT")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(300); // Generous 5-minute default
                const MAX_OUTPUT_BYTES: usize = 8 * 1024;

                let command = args["command"].as_str().unwrap_or("").trim().to_string();
                if command.is_empty() {
                    return "{\"error\":\"empty_command\"}".to_string();
                }

                let program = command.split_whitespace().next().unwrap_or("");
                if !ALLOWED_PROGRAMS.contains(&program) {
                    return format!(
                        "{{\"error\":\"command_not_allowed\",\"detail\":\"'{}' is not whitelisted\",\"allowed\":{}}}",
                        program,
                        serde_json::to_string(ALLOWED_PROGRAMS).unwrap_or_else(|_| "[]".to_string())
                    );
                }

                let mut cmd_str = command.clone();
                if cfg!(windows) {
                    // Quick translation of common Unix shell commands to Windows CMD builtins
                    if cmd_str.starts_with("ls ") || cmd_str == "ls" {
                        cmd_str = cmd_str.replace("ls", "dir");
                    }
                    if cmd_str.starts_with("cat ") {
                        cmd_str = cmd_str.replace("cat", "type");
                    }
                    if cmd_str.starts_with("rm -rf ") {
                        let target = cmd_str.strip_prefix("rm -rf ").unwrap_or("").trim();
                        cmd_str = format!("rmdir /s /q {}", target);
                    }
                    if cmd_str == "pwd" {
                        cmd_str = "cd".to_string();
                    }
                    if cmd_str == "clear" {
                        cmd_str = "cls".to_string();
                    }
                }

                // --- Auto-installation check on Linux ---
                if cfg!(target_os = "linux") {
                    fn check_tool_exists(prog: &str) -> bool {
                        std::process::Command::new("sh")
                            .args(["-c", &format!("command -v {}", prog)])
                            .output()
                            .map(|o| o.status.success())
                            .unwrap_or_else(|_| {
                                std::process::Command::new("which")
                                    .arg(prog)
                                    .output()
                                    .map(|o| o.status.success())
                                    .unwrap_or(false)
                            })
                    }

                    fn install_tool_on_linux(prog: &str) {
                        let package = match prog {
                            "git" => "git",
                            "protoc" | "protobuf" => "protobuf-compiler",
                            "node" | "npm" => "nodejs npm",
                            "cargo" | "rustc" => "rustc cargo",
                            "make" | "gcc" => "build-essential pkg-config libssl-dev",
                            _ => return,
                        };

                        println!("[AEGIS] La herramienta '{}' no esta instalada. Intentando instalar el paquete '{}' de forma rapida...", prog, package);

                        // Try fast apt-get install directly first (saves 10-20 seconds!)
                        let fast_cmd = format!("apt-get install -y {}", package);
                        let output = std::process::Command::new("sh")
                            .args(["-c", &fast_cmd])
                            .output();

                        if let Ok(ref out) = output {
                            if out.status.success() {
                                println!(
                                    "[AEGIS] Instale con exito '{}' de forma rapida.",
                                    package
                                );
                                return;
                            }
                        }

                        // Fallback: update package list and try again
                        println!("[AEGIS] Intento rapido fallido. Actualizando repositorio e instalando '{}'...", package);
                        let cmd = format!("apt-get update && apt-get install -y {}", package);
                        let output = std::process::Command::new("sh").args(["-c", &cmd]).output();

                        if let Ok(ref out) = output {
                            if out.status.success() {
                                println!(
                                    "[AEGIS] Instale con exito '{}' via apt-get con actualizacion.",
                                    package
                                );
                                return;
                            }
                        }

                        // Try with sudo if previous attempts failed
                        let sudo_cmd =
                            format!("sudo apt-get update && sudo apt-get install -y {}", package);
                        let _ = std::process::Command::new("sh")
                            .args(["-c", &sudo_cmd])
                            .output();
                    }

                    // Check primary program
                    let exists = if program == "cargo" || program == "rustc" {
                        check_tool_exists("cargo") && check_tool_exists("rustc")
                    } else {
                        check_tool_exists(program)
                    };

                    if !exists {
                        install_tool_on_linux(program);
                    }

                    // Check protobuf compiler if program is cargo or make
                    if (program == "cargo" || program == "make") && !check_tool_exists("protoc") {
                        install_tool_on_linux("protoc");
                    }

                    // Check gcc/make compiler tools if program is cargo or make
                    if (program == "cargo" || program == "make")
                        && (!check_tool_exists("gcc") || !check_tool_exists("make"))
                    {
                        install_tool_on_linux("make");
                    }
                }

                let workspace = Self::get_tenant_workspace(pcb);
                // CORE-FIX: make sure the workspace exists before we try to run
                // anything in it — otherwise a specialist's first command
                // (typically `git clone …` on a brand-new tenant) fails at
                // spawn time with "No such file or directory" because
                // current_dir() points at a path that was never created.
                if let Err(e) = tokio::fs::create_dir_all(&workspace).await {
                    return format!(
                        "{{\"error\":\"workspace_unavailable\",\"detail\":\"{}\"}}",
                        e
                    );
                }
                let cwd_arg = args.get("cwd").and_then(|v| v.as_str()).unwrap_or(".");
                let cwd = match Self::resolve_path(&workspace, cwd_arg, &[], false) {
                    Ok(p) if p.is_dir() => p,
                    _ => workspace.clone(),
                };

                // OPTIMIZATION: Configure Shared Cargo Target Directory cache to speed up subagent rust checks from minutes to seconds
                let central_target = workspace
                    .parent()
                    .unwrap_or(&workspace)
                    .join("cargo_target_cache");

                let output_result = if cfg!(windows) {
                    tokio::time::timeout(
                        std::time::Duration::from_secs(timeout_secs),
                        tokio::process::Command::new("cmd")
                            .args(["/C", cmd_str.as_str()])
                            .current_dir(&cwd)
                            .env("CARGO_TARGET_DIR", &central_target)
                            .output(),
                    )
                    .await
                } else {
                    tokio::time::timeout(
                        std::time::Duration::from_secs(timeout_secs),
                        tokio::process::Command::new("sh")
                            .args(["-c", cmd_str.as_str()])
                            .current_dir(&cwd)
                            .env("CARGO_TARGET_DIR", &central_target)
                            .output(),
                    )
                    .await
                };

                fn truncate_output(s: String, max: usize) -> String {
                    if s.len() > max {
                        let half = max / 2;
                        let head: String = s.chars().take(half).collect();
                        let tail: String = s
                            .chars()
                            .skip(s.chars().count().saturating_sub(half))
                            .collect();
                        format!("{}\n\n[...truncated...]\n\n{}", head, tail)
                    } else {
                        s
                    }
                }

                match output_result {
                    Ok(Ok(output)) => {
                        let stdout = truncate_output(
                            String::from_utf8_lossy(&output.stdout).to_string(),
                            MAX_OUTPUT_BYTES,
                        );
                        let stderr = truncate_output(
                            String::from_utf8_lossy(&output.stderr).to_string(),
                            MAX_OUTPUT_BYTES,
                        );
                        serde_json::json!({
                            "exit_code": output.status.code(),
                            "stdout": stdout,
                            "stderr": stderr,
                            "cwd": cwd.to_string_lossy(),
                        })
                        .to_string()
                    }
                    Ok(Err(e)) => {
                        format!("{{\"error\":\"spawn_failed\",\"detail\":\"{}\"}}", e)
                    }
                    Err(_) => format!(
                        "{{\"error\":\"timeout\",\"detail\":\"command exceeded {}s\"}}",
                        timeout_secs
                    ),
                }
            }

            // CORE-375: Herramientas personales instantáneas del Chat Agent
            "log_expense_or_entry" => {
                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };
                let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                    return "{\"error\":\"missing_session_key\"}".to_string();
                };

                let raw_category = args["category"].as_str().unwrap_or("general");
                let description = args["description"].as_str().unwrap_or("");
                let amount = args["amount"].as_f64().unwrap_or(0.0);

                let norm_category =
                    crate::enclave::normalize_expense_category(raw_category, description);

                match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                    Ok(db) => {
                        if let Err(e) = db.add_expense(amount, description, Some(&norm_category)) {
                            return format!(
                                "{{\"error\":\"add_expense_failed\",\"detail\":\"{}\"}}",
                                e
                            );
                        }
                        match db.query_expenses_summary(Some(&norm_category)) {
                            Ok(summary) => summary.to_string(),
                            Err(e) => {
                                format!("{{\"error\":\"summary_failed\",\"detail\":\"{}\"}}", e)
                            }
                        }
                    }
                    Err(e) => format!("{{\"error\":\"enclave_open_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            "query_ledger" => {
                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };
                let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                    return "{\"error\":\"missing_session_key\"}".to_string();
                };

                let category_opt = args["category"].as_str();

                match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                    Ok(db) => match db.query_expenses_summary(category_opt) {
                        Ok(summary) => summary.to_string(),
                        Err(e) => format!("{{\"error\":\"summary_failed\",\"detail\":\"{}\"}}", e),
                    },
                    Err(e) => format!("{{\"error\":\"enclave_open_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            "remember_fact" => {
                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };
                let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                    return "{\"error\":\"missing_session_key\"}".to_string();
                };

                let action = args["action"]
                    .as_str()
                    .unwrap_or("upsert")
                    .trim()
                    .to_lowercase();
                let id_opt = args.get("id").and_then(|v| {
                    v.as_i64().or_else(|| {
                        v.as_str()
                            .map(|s| s.trim().trim_start_matches('#').trim())
                            .and_then(|s| s.parse::<i64>().ok())
                    })
                });
                let category = args["category"].as_str().unwrap_or("general");
                let fact = args["fact"].as_str().unwrap_or("").trim();

                match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                    Ok(db) => match action.as_str() {
                        "delete" => {
                            let Some(fid) = id_opt else {
                                return "{\"error\":\"missing_id\",\"detail\":\"Indicá el id de la nota a eliminar.\"}".to_string();
                            };
                            match db.delete_personal_fact(fid) {
                                Ok(true) => serde_json::json!({
                                    "status": "deleted",
                                    "id": fid
                                })
                                .to_string(),
                                Ok(false) => serde_json::json!({
                                    "status": "not_found",
                                    "id": fid
                                })
                                .to_string(),
                                Err(e) => format!(
                                    "{{\"error\":\"remember_fact_delete_failed\",\"detail\":\"{}\"}}",
                                    e
                                ),
                            }
                        }
                        _ => {
                            if fact.is_empty() {
                                return "{\"error\":\"empty_fact\",\"detail\":\"El hecho a guardar no puede estar vacío.\"}".to_string();
                            }
                            match db.upsert_personal_fact(id_opt, category, fact, "chat") {
                                Ok(crate::enclave::PersonalFactUpsertOutcome::Inserted { id }) => {
                                    serde_json::json!({
                                        "status": "remembered",
                                        "id": id,
                                        "fact": fact,
                                        "category": category
                                    })
                                    .to_string()
                                }
                                Ok(crate::enclave::PersonalFactUpsertOutcome::Updated {
                                    id,
                                    previous_fact,
                                }) => serde_json::json!({
                                    "status": "updated",
                                    "id": id,
                                    "fact": fact,
                                    "previous_fact": previous_fact,
                                    "category": category
                                })
                                .to_string(),
                                Ok(crate::enclave::PersonalFactUpsertOutcome::AlreadyKnown {
                                    id,
                                }) => serde_json::json!({
                                    "status": "already_known",
                                    "id": id,
                                    "fact": fact,
                                    "category": category
                                })
                                .to_string(),
                                Err(e) => format!(
                                    "{{\"error\":\"remember_fact_failed\",\"detail\":\"{}\"}}",
                                    e
                                ),
                            }
                        }
                    },
                    Err(e) => format!("{{\"error\":\"enclave_open_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            "manage_reminder" => {
                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };
                let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                    return "{\"error\":\"missing_session_key\"}".to_string();
                };

                let action = args["action"].as_str().unwrap_or("list").trim();
                let id_arg = args.get("id").and_then(|v| {
                    v.as_i64()
                        .map(|n| n.to_string())
                        .or_else(|| v.as_str().map(|s| s.trim().to_string()))
                        .filter(|s| !s.is_empty())
                });
                let text = args["text"].as_str().unwrap_or("").trim();
                let time_hint = args["time_hint"].as_str().unwrap_or("").trim();
                let raw_rec = args["recurrence"].as_str().unwrap_or("").trim();

                let mut norm_rec = crate::enclave::normalize_recurrence(raw_rec);
                let th_lc = time_hint.to_lowercase();
                if norm_rec.is_empty()
                    && (th_lc.contains("todos los")
                        || th_lc.contains("cada ")
                        || th_lc.contains("diari")
                        || th_lc.contains("semanal")
                        || th_lc.contains("mensual"))
                {
                    norm_rec = crate::enclave::normalize_recurrence(time_hint);
                }

                let now_local = chrono::Local::now();
                let remind_at =
                    crate::enclave::normalize_remind_at(time_hint, &norm_rec, now_local);

                match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                    Ok(db) => match action {
                        "add" => {
                            if text.is_empty() {
                                return "{\"error\":\"missing_text\",\"detail\":\"Falta la descripción del recordatorio.\"}".to_string();
                            }
                            match db.add_reminder_with_recurrence(&remind_at, text, Some(&norm_rec))
                            {
                                Ok(new_id) => serde_json::json!({
                                    "status": "added",
                                    "id": new_id,
                                    "description": text,
                                    "remind_at": remind_at,
                                    "recurrence": norm_rec
                                })
                                .to_string(),
                                Err(e) => format!(
                                    "{{\"error\":\"add_reminder_failed\",\"detail\":\"{}\"}}",
                                    e
                                ),
                            }
                        }
                        "update" => {
                            let target = id_arg.as_deref().unwrap_or(text);
                            if target.is_empty() {
                                return "{\"error\":\"missing_target\",\"detail\":\"Indicá 'id' o 'text' para actualizar el recordatorio.\"}".to_string();
                            }
                            let new_remind_at =
                                (!time_hint.is_empty()).then_some(remind_at.as_str());
                            let new_desc = (id_arg.is_some() && !text.is_empty()).then_some(text);
                            let new_rec = args
                                .get("recurrence")
                                .is_some()
                                .then_some(norm_rec.as_str())
                                .or_else(|| (!norm_rec.is_empty()).then_some(norm_rec.as_str()));
                            match db.update_reminder(target, new_remind_at, new_desc, new_rec) {
                                Ok(count) => {
                                    let reminders = db.get_reminders(30).unwrap_or_default();
                                    serde_json::json!({
                                        "status": "updated",
                                        "updated_count": count,
                                        "target": target,
                                        "reminders": reminders
                                    })
                                    .to_string()
                                }
                                Err(e) => format!(
                                    "{{\"error\":\"update_reminder_failed\",\"detail\":\"{}\"}}",
                                    e
                                ),
                            }
                        }
                        "delete" | "remove" => {
                            let target = id_arg.as_deref().unwrap_or(text);
                            if target.is_empty() {
                                return "{\"error\":\"missing_target\",\"detail\":\"Indicá 'id' o 'text' del recordatorio a eliminar.\"}".to_string();
                            }
                            match db.delete_reminder(target) {
                                Ok(count) => serde_json::json!({
                                    "status": "deleted",
                                    "deleted_count": count,
                                    "target": target
                                })
                                .to_string(),
                                Err(e) => format!(
                                    "{{\"error\":\"delete_reminder_failed\",\"detail\":\"{}\"}}",
                                    e
                                ),
                            }
                        }
                        "complete" => {
                            let target = id_arg.as_deref().unwrap_or(text);
                            if target.is_empty() {
                                return "{\"error\":\"missing_target\",\"detail\":\"Indicá 'id' o 'text' del recordatorio a completar.\"}".to_string();
                            }
                            match db.complete_reminder(target) {
                                Ok(count) => serde_json::json!({
                                    "status": "completed",
                                    "matched_count": count,
                                    "target": target
                                })
                                .to_string(),
                                Err(e) => format!(
                                    "{{\"error\":\"complete_reminder_failed\",\"detail\":\"{}\"}}",
                                    e
                                ),
                            }
                        }
                        _ => match db.get_reminders(30) {
                            Ok(reminders) => serde_json::json!({
                                "status": "ok",
                                "reminders": reminders
                            })
                            .to_string(),
                            Err(e) => format!(
                                "{{\"error\":\"list_reminders_failed\",\"detail\":\"{}\"}}",
                                e
                            ),
                        },
                    },
                    Err(e) => format!("{{\"error\":\"enclave_open_failed\",\"detail\":\"{}\"}}", e),
                }
            }

            // CORE-385: Puente directo hacia Google Antigravity (SDK + IDE Outbox)
            "send_to_antigravity" => {
                let instruction = args["instruction"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if instruction.is_empty() {
                    return "{\"error\":\"instruction is required\"}".to_string();
                }
                let project_name = args
                    .get("project_name")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("Antigravity")
                    .to_string();

                let orchestrator_opt = self.agent_orchestrator.read().await.clone();
                match orchestrator_opt {
                    None => "{\"error\":\"AgentOrchestrator not configured\"}".to_string(),
                    Some(orchestrator) => {
                        // 1. Encolar siempre en el outbox particionado por tenant para el hook del IDE Antigravity
                        let outbox_tenant = pcb.tenant_id.as_deref().unwrap_or("default");
                        orchestrator
                            .agy_runner
                            .push_ide_outbox(outbox_tenant, instruction.clone())
                            .await;

                        // 2. Si el SDK google-antigravity está habilitado, despachar también al supervisor
                        if orchestrator.agy_runner.is_enabled() {
                            if let Ok(agent_id) = orchestrator
                                .create_project(
                                    project_name.clone(),
                                    instruction.clone(),
                                    Some(crate::pcb::TaskType::Code),
                                    pcb.tenant_id.clone(),
                                )
                                .await
                            {
                                let _ = orchestrator
                                    .dispatch(agent_id, instruction.clone(), vec![])
                                    .await;
                                let tenant = pcb.tenant_id.as_deref().unwrap_or("default");
                                let nodes = orchestrator.tree_snapshot_for_tenant(tenant).await;
                                orchestrator.emit_event(
                                    crate::agents::event::AgentEvent::TreeSnapshot { nodes },
                                );
                                return serde_json::json!({
                                    "status": "dispatched_to_antigravity_sdk_and_ide_outbox",
                                    "project": project_name,
                                    "agent_id": agent_id.to_string(),
                                })
                                .to_string();
                            }
                        }

                        serde_json::json!({
                            "status": "queued_for_antigravity_ide",
                            "project": project_name,
                            "message": "Instrucción encolada para Google Antigravity."
                        })
                        .to_string()
                    }
                }
            }

            // Gestión conversacional de proveedores de IA, claves API y modelos desde el chat
            "manage_ai_provider" => {
                let action = args["action"]
                    .as_str()
                    .unwrap_or("list")
                    .trim()
                    .to_lowercase();
                let raw_provider = args["provider"].as_str().unwrap_or("").trim();
                let mut raw_key = args["api_key"].as_str().unwrap_or("").trim().to_string();
                let raw_model = args["model"].as_str().unwrap_or("").trim();
                let raw_url = args["api_url"].as_str().unwrap_or("").trim();
                let set_primary = args["set_primary"].as_bool().unwrap_or(false);

                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                let router_opt = self.router_ref.read().await.clone();
                let Some(router_rw) = router_opt else {
                    return "{\"error\":\"router_not_available\"}".to_string();
                };

                if raw_key.is_empty() {
                    if let Some(extracted) =
                        Self::extract_api_key_from_text(&pcb.memory_pointers.l1_instruction)
                    {
                        raw_key = extracted;
                    }
                }

                let provider = Self::infer_provider_from_key_or_text(
                    raw_provider,
                    &raw_key,
                    &pcb.memory_pointers.l1_instruction,
                );

                match action.as_str() {
                    "list" => {
                        let router = router_rw.read().await;
                        let global_keys = router.list_global_keys().await;
                        let tenant_keys = router.list_tenant_keys(tenant_id).await;
                        let catalog_models = router.list_models_for_catalog().await;
                        let providers_summary = summarize_configured_providers(
                            tenant_keys.into_iter().chain(global_keys),
                            &catalog_models,
                        );

                        let agy_enabled = match self.agent_orchestrator.read().await.clone() {
                            Some(orch) => orch.agy_runner.is_enabled(),
                            None => false,
                        };

                        serde_json::json!({
                            "status": "ok",
                            "configured_providers": providers_summary,
                            "total_catalog_models": catalog_models.len(),
                            "antigravity_sdk_enabled": agy_enabled,
                            "hint": "Para ver la lista completa o modelos gratuitos de un proveedor (ej. openrouter), llamá a manage_ai_provider con action='catalog'."
                        })
                        .to_string()
                    }

                    "catalog" => {
                        let free_only = args["free_only"].as_bool().unwrap_or(false);
                        let limit = args["limit"].as_u64().unwrap_or(30).clamp(1, 100) as usize;
                        let filter_provider = if raw_provider.eq_ignore_ascii_case("openroute")
                            || provider.eq_ignore_ascii_case("openroute")
                        {
                            "openrouter"
                        } else if !raw_provider.is_empty() {
                            raw_provider
                        } else {
                            provider.as_str()
                        };

                        let router = router_rw.read().await;
                        let mut matches: Vec<_> = router
                            .list_models_for_catalog()
                            .await
                            .into_iter()
                            .filter(|m| {
                                if !filter_provider.is_empty() {
                                    if filter_provider.eq_ignore_ascii_case("openrouter") {
                                        if !(m.provider == "openrouter"
                                            || (!m.is_local && m.model_id.contains('/')))
                                        {
                                            return false;
                                        }
                                    } else if !m.provider.eq_ignore_ascii_case(filter_provider) {
                                        return false;
                                    }
                                }
                                if free_only {
                                    let is_free = m.free_tier_eligible
                                        || m.model_id.ends_with(":free")
                                        || (m.cost_input_per_mtok == 0.0
                                            && m.cost_output_per_mtok == 0.0
                                            && m.model_id != "openrouter/auto");
                                    if !is_free {
                                        return false;
                                    }
                                }
                                true
                            })
                            .collect();

                        matches.sort_by(|a, b| {
                            let a_free = a.free_tier_eligible || a.model_id.ends_with(":free");
                            let b_free = b.free_tier_eligible || b.model_id.ends_with(":free");
                            b_free
                                .cmp(&a_free)
                                .then_with(|| {
                                    a.cost_input_per_mtok.total_cmp(&b.cost_input_per_mtok)
                                })
                                .then_with(|| a.model_id.cmp(&b.model_id))
                        });

                        let total_matching = matches.len();
                        let models: Vec<serde_json::Value> = matches
                            .into_iter()
                            .take(limit)
                            .map(|m| {
                                let is_free = m.free_tier_eligible
                                    || m.model_id.ends_with(":free")
                                    || (m.cost_input_per_mtok == 0.0
                                        && m.cost_output_per_mtok == 0.0
                                        && m.model_id != "openrouter/auto");
                                serde_json::json!({
                                    "model_id": m.model_id,
                                    "display_name": m.display_name,
                                    "provider": m.provider,
                                    "free": is_free,
                                    "cost_input_per_mtok": m.cost_input_per_mtok,
                                    "cost_output_per_mtok": m.cost_output_per_mtok,
                                    "context_window": m.context_window,
                                    "supports_tools": m.supports_tools,
                                })
                            })
                            .collect();

                        serde_json::json!({
                            "status": "ok",
                            "provider_filter": if filter_provider.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(filter_provider.to_string()) },
                            "free_only": free_only,
                            "total_matching": total_matching,
                            "returned": models.len(),
                            "models": models,
                        })
                        .to_string()
                    }

                    "add" => {
                        let is_free_prompt = pcb
                            .memory_pointers
                            .l1_instruction
                            .to_lowercase()
                            .contains("gratis")
                            || pcb
                                .memory_pointers
                                .l1_instruction
                                .to_lowercase()
                                .contains("free")
                            || pcb
                                .memory_pointers
                                .l1_instruction
                                .to_lowercase()
                                .contains("gratuito")
                            || pcb
                                .memory_pointers
                                .l1_instruction
                                .to_lowercase()
                                .contains("cuenta gratuita");

                        let all_detected = Self::extract_all_api_keys_from_text(
                            &pcb.memory_pointers.l1_instruction,
                        );
                        if all_detected.len() > 1 {
                            let mut added_list = Vec::new();
                            let router = router_rw.read().await;
                            for (p, k) in all_detected {
                                if p.is_empty() {
                                    continue;
                                }
                                if p == "elevenlabs" {
                                    let masked = Self::mask_single_key(&k);
                                    if let Some(session_key) = Self::valid_pcb_session_key(pcb) {
                                        if let Ok(db) =
                                            crate::enclave::TenantDB::open(tenant_id, session_key)
                                        {
                                            let _ = db.set_kv("elevenlabs_api_key", &k);
                                        }
                                    }
                                    added_list.push(serde_json::json!({
                                        "provider": "elevenlabs",
                                        "service": "Siren Voice (TTS)",
                                        "masked_key": masked,
                                    }));
                                    continue;
                                }

                                let (def_m, def_u) = Self::default_model_and_url_for_provider(&p);
                                let discovered =
                                    match crate::router::discovery::fetch_provider_models(
                                        &p,
                                        Some(def_u),
                                        &k,
                                    )
                                    .await
                                    {
                                        Ok(list) if !list.is_empty() => {
                                            list.into_iter().map(|d| d.model_id).collect()
                                        }
                                        _ => vec![def_m.to_string()],
                                    };
                                let masked = Self::mask_single_key(&k);
                                let entry = crate::router::key_pool::ApiKeyEntry {
                                    key_id: uuid::Uuid::new_v4().to_string(),
                                    provider: p.clone(),
                                    api_key: k.clone(),
                                    api_url: Some(def_u.to_string()),
                                    label: Some(format!("Chat ({})", p)),
                                    is_active: true,
                                    rate_limited_until: None,
                                    active_models: Some(discovered.clone()),
                                    is_free_tier: is_free_prompt,
                                };
                                let _ = router.add_tenant_key(tenant_id, entry.clone()).await;
                                let _ = router.add_global_key(entry.clone()).await;
                                let _ = crate::router::syncer::register_provider_models(
                                    &p,
                                    &discovered,
                                    &router.catalog_ref(),
                                )
                                .await;
                                added_list.push(serde_json::json!({
                                    "provider": p,
                                    "models": discovered,
                                    "masked_key": masked,
                                }));
                            }
                            router.invalidate_all_sticky().await;
                            return serde_json::json!({
                                "status": "batch_added",
                                "count": added_list.len(),
                                "providers": added_list,
                                "message": format!("Se guardaron {} proveedores con éxito en el enclave cifrado.", added_list.len()),
                            }).to_string();
                        }

                        if provider == "elevenlabs" {
                            let masked = Self::mask_single_key(&raw_key);
                            if let Some(session_key) = Self::valid_pcb_session_key(pcb) {
                                if let Ok(db) =
                                    crate::enclave::TenantDB::open(tenant_id, session_key)
                                {
                                    let _ = db.set_kv("elevenlabs_api_key", &raw_key);
                                }
                            }
                            return serde_json::json!({
                                "status": "added",
                                "provider": "elevenlabs",
                                "service": "Siren Voice (TTS)",
                                "masked_key": masked,
                                "message": "Clave de ElevenLabs configurada en el motor de voz Siren."
                            }).to_string();
                        }

                        if provider.is_empty() {
                            return serde_json::json!({
                                "error": "provider_unknown",
                                "message": "No pude identificar el proveedor. Indicá por ejemplo: Groq, Gemini, OpenRouter, OpenAI, Anthropic, DeepSeek u Ollama."
                            })
                            .to_string();
                        }

                        if provider != "ollama" && raw_key.is_empty() {
                            return serde_json::json!({
                                "error": "missing_api_key",
                                "provider": provider,
                                "message": format!("Falta la clave API para {}. Pasame la API key para guardarla en el enclave.", provider)
                            })
                            .to_string();
                        }

                        let (default_model, default_url) =
                            Self::default_model_and_url_for_provider(&provider);
                        let effective_model = if !raw_model.is_empty() {
                            raw_model.to_string()
                        } else {
                            default_model.to_string()
                        };
                        let effective_url = if !raw_url.is_empty() {
                            raw_url.to_string()
                        } else {
                            default_url.to_string()
                        };

                        let masked = Self::mask_single_key(&raw_key);
                        let discovered = if !raw_model.is_empty() {
                            vec![raw_model.to_string()]
                        } else {
                            match crate::router::discovery::fetch_provider_models(
                                &provider,
                                Some(&effective_url),
                                &raw_key,
                            )
                            .await
                            {
                                Ok(list) if !list.is_empty() => {
                                    list.into_iter().map(|d| d.model_id).collect()
                                }
                                _ => vec![effective_model.clone()],
                            }
                        };

                        let entry = crate::router::key_pool::ApiKeyEntry {
                            key_id: uuid::Uuid::new_v4().to_string(),
                            provider: provider.clone(),
                            api_key: raw_key.clone(),
                            api_url: Some(effective_url.clone()),
                            label: Some(format!("Chat ({})", provider)),
                            is_active: true,
                            rate_limited_until: None,
                            active_models: Some(discovered.clone()),
                            is_free_tier: is_free_prompt,
                        };

                        {
                            let router = router_rw.read().await;
                            let _ = router.add_tenant_key(tenant_id, entry.clone()).await;
                            let _ = router.add_global_key(entry.clone()).await;
                            let _ = crate::router::syncer::register_provider_models(
                                &provider,
                                &discovered,
                                &router.catalog_ref(),
                            )
                            .await;
                            router.invalidate_all_sticky().await;
                        }

                        if set_primary {
                            self.update_cloud_credentials(
                                effective_url,
                                effective_model.clone(),
                                raw_key,
                            )
                            .await;
                        }

                        serde_json::json!({
                            "status": "added",
                            "provider": provider,
                            "model": effective_model,
                            "masked_key": masked,
                            "set_primary": set_primary,
                            "message": "Proveedor y clave guardados de forma cifrada en el enclave. Nunca repitas la clave completa en el chat."
                        })
                        .to_string()
                    }

                    "add_model" => {
                        if provider.is_empty() || raw_model.is_empty() {
                            return serde_json::json!({
                                "error": "missing_params",
                                "message": "Indicá tanto el proveedor (ej: gemini, groq, openrouter, ollama) como el nombre del modelo a sumar."
                            })
                            .to_string();
                        }

                        let router = router_rw.read().await;
                        let existing = router
                            .get_raw_key_for_provider(&provider, Some(tenant_id))
                            .await;

                        let Some(mut entry) = existing else {
                            return serde_json::json!({
                                "error": "provider_not_configured",
                                "provider": provider,
                                "message": format!("Todavía no tenés una clave configurada para {}. Pasame primero su API key.", provider)
                            })
                            .to_string();
                        };

                        let mut models = entry.active_models.take().unwrap_or_default();
                        if !models.iter().any(|m| m == raw_model) {
                            models.push(raw_model.to_string());
                        }
                        entry.active_models = Some(models.clone());

                        let _ = router.add_tenant_key(tenant_id, entry.clone()).await;
                        let _ = router.add_global_key(entry).await;
                        let _ = crate::router::syncer::register_provider_models(
                            &provider,
                            &models,
                            &router.catalog_ref(),
                        )
                        .await;
                        router.invalidate_all_sticky().await;

                        serde_json::json!({
                            "status": "model_added",
                            "provider": provider,
                            "added_model": raw_model,
                            "active_models": models,
                        })
                        .to_string()
                    }

                    "remove" => {
                        if provider.is_empty() {
                            return "{\"error\":\"provider is required to remove\"}".to_string();
                        }
                        let router = router_rw.read().await;
                        let mut removed = 0usize;
                        for k in router.list_tenant_keys(tenant_id).await {
                            if k.provider == provider
                                && router.delete_key(&k.key_id, Some(tenant_id)).await.is_ok()
                            {
                                removed += 1;
                            }
                        }
                        // CORE-400: Solo root puede eliminar claves globales desde chat.
                        if tenant_id == "root" {
                            for k in router.list_global_keys().await {
                                if k.provider == provider
                                    && router.delete_key(&k.key_id, None).await.is_ok()
                                {
                                    removed += 1;
                                }
                            }
                            router.invalidate_all_sticky().await;
                        } else {
                            router.invalidate_sticky(tenant_id).await;
                        }
                        serde_json::json!({
                            "status": "removed",
                            "provider": provider,
                            "removed_keys_count": removed
                        })
                        .to_string()
                    }

                    _ => "{\"error\":\"invalid_action\"}".to_string(),
                }
            }

            // Inspección y habilitación/deshabilitación de módulos del microkernel y plugins cargados
            "manage_modules" => {
                let action = args["action"]
                    .as_str()
                    .unwrap_or("list")
                    .trim()
                    .to_lowercase();
                let module_id = args["module_id"].as_str().unwrap_or("").trim();
                let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

                let router_opt = self.router_ref.read().await.clone();
                let Some(router_rw) = router_opt else {
                    return "{\"error\":\"router_not_available\"}".to_string();
                };

                let router = router_rw.read().await;
                let modules_map = router.modules.read().await;
                let db_opt = Self::valid_pcb_session_key(pcb)
                    .and_then(|skey| crate::enclave::TenantDB::open(tenant_id, skey).ok());

                match action.as_str() {
                    "list" => {
                        let mut manifests: Vec<_> = modules_map.values().collect();
                        manifests.sort_by(|a, b| a.module_id.cmp(&b.module_id));

                        let microkernel_modules: Vec<serde_json::Value> = manifests
                            .into_iter()
                            .map(|m| {
                                let enabled = db_opt
                                    .as_ref()
                                    .and_then(|db| {
                                        db.get_kv(&format!("module_active:{}", m.module_id)).ok()
                                    })
                                    .flatten()
                                    .map(|v| v == "true")
                                    .unwrap_or(false);
                                let tools: Vec<serde_json::Value> = m
                                    .exposed_tools
                                    .iter()
                                    .map(|t| {
                                        serde_json::json!({
                                            "name": t.name,
                                            "description": t.description
                                        })
                                    })
                                    .collect();
                                serde_json::json!({
                                    "module_id": m.module_id,
                                    "display_name": m.display_name,
                                    "version": m.version,
                                    "enabled": enabled,
                                    "tools": tools,
                                })
                            })
                            .collect();

                        let loaded_plugins: Vec<serde_json::Value> = self
                            .plugin_manager
                            .read()
                            .await
                            .list_plugin_summaries()
                            .into_iter()
                            .map(|(name, description, kind)| {
                                serde_json::json!({
                                    "name": name,
                                    "description": description,
                                    "kind": kind,
                                    "active": true,
                                })
                            })
                            .collect();

                        serde_json::json!({
                            "status": "ok",
                            "microkernel_modules": microkernel_modules,
                            "loaded_plugins": loaded_plugins,
                            "builtin_capabilities": ["maker (JS sandbox)"],
                        })
                        .to_string()
                    }
                    "enable" | "disable" => {
                        if module_id.is_empty() {
                            return "{\"error\":\"module_id_required\"}".to_string();
                        }
                        if !modules_map.contains_key(module_id) {
                            let available: Vec<&str> =
                                modules_map.keys().map(String::as_str).collect();
                            return serde_json::json!({
                                "error": "module_not_found",
                                "module_id": module_id,
                                "available_modules": available,
                            })
                            .to_string();
                        }
                        let Some(ref db) = db_opt else {
                            return "{\"error\":\"tenant_db_unavailable\"}".to_string();
                        };
                        let enabled = action == "enable";
                        if let Err(e) = db.set_kv(
                            &format!("module_active:{}", module_id),
                            if enabled { "true" } else { "false" },
                        ) {
                            return serde_json::json!({
                                "error": "db_write_failed",
                                "detail": e.to_string()
                            })
                            .to_string();
                        }
                        serde_json::json!({
                            "status": if enabled { "enabled" } else { "disabled" },
                            "module_id": module_id,
                            "enabled": enabled,
                        })
                        .to_string()
                    }
                    _ => "{\"error\":\"invalid_action\"}".to_string(),
                }
            }

            "manage_google_calendar" => {
                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };
                let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                    return "{\"error\":\"missing_session_key\"}".to_string();
                };
                let action = args["action"]
                    .as_str()
                    .unwrap_or("list")
                    .trim()
                    .to_lowercase();

                if action == "status" {
                    return match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                        Ok(db) => {
                            let connected = db.is_oauth_connected("google").unwrap_or(false);
                            let email = db.get_oauth_email("google").ok().flatten();
                            let scope = db.get_oauth_scope("google").ok().flatten();
                            let creds = crate::oauth::resolve_client_credentials(&db, "google");
                            serde_json::json!({
                                "status": "ok",
                                "connected": connected,
                                "email": email,
                                "scope": scope,
                                "client_configured": creds.client_id.is_some(),
                            })
                            .to_string()
                        }
                        Err(e) => serde_json::json!({
                            "error": "enclave_open_failed",
                            "detail": e.to_string()
                        })
                        .to_string(),
                    };
                }

                let connected = crate::enclave::TenantDB::open(&tenant_id, session_key)
                    .map(|db| db.is_oauth_connected("google").unwrap_or(false))
                    .unwrap_or(false);
                if !connected {
                    return serde_json::json!({
                        "error": "google_not_connected",
                        "detail": "La cuenta de Google no está vinculada. El usuario debe conectarla en Settings → Cuentas Conectadas."
                    })
                    .to_string();
                }

                let res = match action.as_str() {
                    "list" => {
                        let days = args["days"].as_u64().unwrap_or(7).clamp(1, 60) as u8;
                        let max_results =
                            args["max_results"].as_u64().unwrap_or(20).clamp(1, 50) as u8;
                        let query = args["query"].as_str();
                        crate::oauth::google_calendar::list_events(
                            &self.http_client,
                            &tenant_id,
                            session_key,
                            days,
                            max_results,
                            query,
                        )
                        .await
                    }
                    "create" => {
                        let summary = args["summary"].as_str().unwrap_or("").to_string();
                        let start = args["start"].as_str().unwrap_or("").to_string();
                        let end = args["end"].as_str().map(String::from);
                        let description = args["description"].as_str().map(String::from);
                        let location = args["location"].as_str().map(String::from);
                        let all_day = args["all_day"].as_bool();
                        let attendees = args["attendees"].as_array().map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        });
                        crate::oauth::google_calendar::create_event(
                            &self.http_client,
                            &tenant_id,
                            session_key,
                            crate::oauth::google_calendar::CalendarEventInput {
                                summary,
                                start,
                                end,
                                description,
                                location,
                                attendees,
                                all_day,
                            },
                        )
                        .await
                    }
                    "update" => {
                        let event_id = args["event_id"].as_str().unwrap_or("");
                        crate::oauth::google_calendar::update_event(
                            &self.http_client,
                            &tenant_id,
                            session_key,
                            event_id,
                            args["summary"].as_str(),
                            args["start"].as_str(),
                            args["end"].as_str(),
                            args["description"].as_str(),
                            args["location"].as_str(),
                        )
                        .await
                    }
                    "delete" => {
                        let event_id = args["event_id"].as_str().unwrap_or("");
                        crate::oauth::google_calendar::delete_event(
                            &self.http_client,
                            &tenant_id,
                            session_key,
                            event_id,
                        )
                        .await
                    }
                    _ => Err(anyhow::anyhow!(
                        "Acción de calendario desconocida: {}",
                        action
                    )),
                };

                match res {
                    Ok(val) => val.to_string(),
                    Err(e) => serde_json::json!({
                        "error": "google_calendar_failed",
                        "detail": e.to_string()
                    })
                    .to_string(),
                }
            }

            "manage_whatsapp" => {
                let tenant_id = match &pcb.tenant_id {
                    Some(t) => t.clone(),
                    None => return "{\"error\":\"no_tenant_id\"}".to_string(),
                };
                let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                    return "{\"error\":\"missing_session_key\"}".to_string();
                };
                let action = args["action"]
                    .as_str()
                    .unwrap_or("status")
                    .trim()
                    .to_lowercase();

                if action == "set_confirmation" {
                    let enabled = args["require_confirmation"].as_bool().unwrap_or(true);
                    return match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                        Ok(db) => {
                            if let Err(e) = crate::whatsapp::set_require_confirmation(&db, enabled)
                            {
                                serde_json::json!({
                                    "error": "whatsapp_settings_failed",
                                    "detail": e.to_string()
                                })
                                .to_string()
                            } else {
                                serde_json::json!({
                                    "status": "updated",
                                    "require_confirmation": enabled,
                                    "message": if enabled {
                                        "Confirmación previa ACTIVADA: voy a pedirte OK antes de enviar cada mensaje."
                                    } else {
                                        "Confirmación previa DESACTIVADA: enviaré los mensajes directamente (siempre bajo el limitador anti-spam) hasta que me digas que vuelva a confirmar."
                                    }
                                })
                                .to_string()
                            }
                        }
                        Err(e) => serde_json::json!({
                            "error": "enclave_open_failed",
                            "detail": e.to_string()
                        })
                        .to_string(),
                    };
                }

                if action == "list_pending" {
                    return match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                        Ok(db) => {
                            let pending = crate::whatsapp::list_pending_messages(&db);
                            serde_json::json!({
                                "status": "ok",
                                "count": pending.len(),
                                "pending": pending,
                            })
                            .to_string()
                        }
                        Err(e) => serde_json::json!({
                            "error": "enclave_open_failed",
                            "detail": e.to_string()
                        })
                        .to_string(),
                    };
                }

                if action == "cancel_pending" {
                    let target = args["pending_id"].as_str();
                    return match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                        Ok(db) => match crate::whatsapp::cancel_pending_messages(&db, target) {
                            Ok(removed) => serde_json::json!({
                                "status": "cancelled",
                                "removed_count": removed,
                                "remaining": crate::whatsapp::list_pending_messages(&db),
                            })
                            .to_string(),
                            Err(e) => serde_json::json!({
                                "error": "cancel_failed",
                                "detail": e.to_string()
                            })
                            .to_string(),
                        },
                        Err(e) => serde_json::json!({
                            "error": "enclave_open_failed",
                            "detail": e.to_string()
                        })
                        .to_string(),
                    };
                }

                if action == "save_contact" {
                    let alias = args["alias"]
                        .as_str()
                        .or_else(|| args["name"].as_str())
                        .or_else(|| args["search"].as_str())
                        .unwrap_or("");
                    let target = args["to"]
                        .as_str()
                        .or_else(|| args["phone"].as_str())
                        .or_else(|| args["chat"].as_str())
                        .unwrap_or("");
                    let note = args["note"].as_str().or_else(|| args["message"].as_str());
                    return match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                        Ok(db) => match crate::whatsapp::save_contact_alias(
                            &db, alias, target, note,
                        ) {
                            Ok(entry) => serde_json::json!({
                                "status": "saved",
                                "contact": entry,
                                "total_aliases": crate::whatsapp::list_contact_aliases(&db).len(),
                            })
                            .to_string(),
                            Err(e) => serde_json::json!({
                                "error": "save_contact_failed",
                                "detail": e.to_string(),
                            })
                            .to_string(),
                        },
                        Err(e) => serde_json::json!({
                            "error": "enclave_open_failed",
                            "detail": e.to_string(),
                        })
                        .to_string(),
                    };
                }

                if action == "list_contacts" {
                    let search = args["search"].as_str();
                    let limit = args["limit"].as_u64().unwrap_or(50) as usize;
                    let wa_opt = self.whatsapp.read().await.clone();
                    if let Some(wa) = wa_opt {
                        return match wa
                            .list_contacts(&tenant_id, session_key, search, limit)
                            .await
                        {
                            Ok(val) => val.to_string(),
                            Err(e) => serde_json::json!({
                                "error": "whatsapp_failed",
                                "detail": e.to_string(),
                            })
                            .to_string(),
                        };
                    }
                    return match crate::enclave::TenantDB::open(&tenant_id, session_key) {
                        Ok(db) => serde_json::json!({
                            "saved_aliases": crate::whatsapp::list_contact_aliases(&db),
                            "bridge_connected": false,
                            "bridge_contacts": [],
                        })
                        .to_string(),
                        Err(e) => serde_json::json!({
                            "error": "enclave_open_failed",
                            "detail": e.to_string(),
                        })
                        .to_string(),
                    };
                }

                let wa_opt = self.whatsapp.read().await.clone();
                let Some(wa) = wa_opt else {
                    return "{\"error\":\"whatsapp_manager_unavailable\",\"detail\":\"El gestor de WhatsApp no está inicializado en el kernel.\"}".to_string();
                };

                let res = match action.as_str() {
                    "status" => wa.get_status(&tenant_id, session_key, true).await,
                    "list_chats" => {
                        let search = args["search"].as_str();
                        let limit = args["limit"].as_u64().unwrap_or(20) as usize;
                        wa.list_chats(&tenant_id, search, limit).await
                    }
                    "read_messages" => {
                        let chat = args["chat"]
                            .as_str()
                            .or_else(|| args["to"].as_str())
                            .unwrap_or("");
                        let limit = args["limit"].as_u64().unwrap_or(20) as usize;
                        wa.read_messages_with_aliases(&tenant_id, session_key, chat, limit)
                            .await
                    }
                    "send_message" => {
                        let to = args["to"].as_str().unwrap_or("");
                        let msg = args["message"].as_str().unwrap_or("");
                        wa.send_or_queue_message(&tenant_id, session_key, to, msg)
                            .await
                    }
                    "confirm_send" => {
                        let pid = args["pending_id"].as_str();
                        wa.confirm_pending_and_send(&tenant_id, session_key, pid)
                            .await
                    }
                    _ => Err(anyhow::anyhow!(
                        "Acción de WhatsApp desconocida: {}",
                        action
                    )),
                };

                match res {
                    Ok(val) => val.to_string(),
                    Err(e) => serde_json::json!({
                        "error": "whatsapp_failed",
                        "detail": e.to_string()
                    })
                    .to_string(),
                }
            }

            other => {
                // Módulos de dominio del microkernel (Aegis Creator Studio, Video Studio, Dev Studio, Business & POS, etc.)
                if let Some(router_rw) = self.router_ref.read().await.clone() {
                    let (manifest_opt, key_pool_arc) = {
                        let router_guard = router_rw.read().await;
                        (
                            router_guard.find_module_for_tool(other).await,
                            router_guard.key_pool_arc(),
                        )
                    };
                    if let Some(manifest) = manifest_opt {
                        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");
                        let Some(session_key) = Self::valid_pcb_session_key(pcb) else {
                            return "{\"error\":\"missing_session_key\"}".to_string();
                        };
                        if let Ok(db) = crate::enclave::TenantDB::open(tenant_id, session_key) {
                            let active_key = format!("module_active:{}", manifest.module_id);
                            if db.get_kv(&active_key).unwrap_or(None).as_deref() != Some("true") {
                                let _ = db.set_kv(&active_key, "true");
                            }
                        }
                        let mut video_ctx = crate::video_studio::VideoExecContext {
                            key_pool: Some(key_pool_arc),
                            router: Some(router_rw.clone()),
                            ..Default::default()
                        };
                        if let Some(ptx) = progress_tx {
                            let tx_clone = ptx.clone();
                            let cb: crate::video_studio::types::ProgressCallback =
                                Arc::new(move |evt| {
                                    if let Ok(payload) = serde_json::to_string(&evt) {
                                        let _ =
                                            tx_clone.send(Ok(format!("__PROGRESS__{}", payload)));
                                    }
                                });
                            video_ctx.progress_cb = Some(cb);
                        }
                        if pcb.agent_id.is_none()
                            && other == "video_create_project"
                            && args.get("auto_render").is_none()
                            && !args
                                .get("draft_only")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false)
                        {
                            if let Some(obj) = args.as_object_mut() {
                                obj.insert("auto_render".to_string(), serde_json::json!(true));
                            }
                        }
                        let args_str = args.to_string();
                        return match crate::router::modules::execute_module_tool_with_context(
                            &manifest,
                            other,
                            &args_str,
                            tenant_id,
                            session_key,
                            Some(&video_ctx),
                        )
                        .await
                        {
                            Ok(res) => res,
                            Err(e) => serde_json::json!({
                                "error": "module_tool_failed",
                                "module_id": manifest.module_id,
                                "detail": e,
                            })
                            .to_string(),
                        };
                    }
                }

                // CORE-377: Check if the tool is registered in the MCP Registry
                if self.mcp_registry.has_tool(other).await {
                    // CORE-377 Criterion 4: Aislamiento de permisos contra políticas de tenant / Citadel
                    if let Some(tenant_id) = &pcb.tenant_id {
                        if let Some(session_key) = Self::valid_pcb_session_key(pcb) {
                            if let Ok(db) = crate::enclave::TenantDB::open(tenant_id, session_key) {
                                let tool_policy_key = format!("mcp_tool_policy:{}", other);
                                if let Ok(Some(policy)) = db.get_kv(&tool_policy_key) {
                                    if policy == "deny" {
                                        tracing::warn!(
                                            pid = %_pid,
                                            tool = %other,
                                            tenant = %tenant_id,
                                            "CORE-377: MCP tool call denied by tenant policy"
                                        );
                                        return format!(
                                            "{{\"error\": \"Permission denied: tool '{}' is restricted for this tenant.\"}}",
                                            other
                                        );
                                    }
                                }
                            }
                        }
                    }

                    match ank_mcp::registry::McpToolDispatcher::execute(
                        &self.mcp_registry,
                        other,
                        args,
                    )
                    .await
                    {
                        Ok(val) => {
                            tracing::info!(
                                pid = %_pid,
                                tool = %other,
                                "CORE-377: MCP tool successfully executed"
                            );
                            val.to_string()
                        }
                        Err(e) => {
                            tracing::error!(
                                pid = %_pid,
                                tool = %other,
                                error = %e,
                                "CORE-377: MCP tool execution failed"
                            );
                            format!(
                                "{{\"error\":\"mcp_execution_failed\",\"detail\":\"{}\"}}",
                                e
                            )
                        }
                    }
                } else {
                    format!("{{\"error\":\"Unknown tool: {}\"}}", other)
                }
            }
        }
    }
}

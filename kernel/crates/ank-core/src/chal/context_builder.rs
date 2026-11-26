use super::*;
use crate::agents::node::AgentRole;
use crate::chal::{ChatMessage, ChatRole, CognitiveHAL};
use crate::pcb::PCB;

/// Selects the minimal domain-relevant subset of `ChatAgent` tool names for a turn (CORE-408).
///
/// Inspects the current prompt plus the last 2 messages in `history` so short
/// follow-ups ("sí, borralo", "dale", "a las 18hs") inherit the active domain.
pub(crate) fn select_chat_tool_names(prompt: &str, history: &[ChatMessage]) -> Vec<&'static str> {
    let lower = prompt.trim().to_lowercase();
    if lower.is_empty() {
        return Vec::new();
    }

    // Build a small rolling text window: current prompt + last 2 turns (if short follow-up).
    let mut recent_window = lower.clone();
    let is_short_followup = prompt.trim().chars().count() < 55 && !history.is_empty();
    if is_short_followup {
        for msg in history.iter().rev().take(2) {
            if let Some(ref c) = msg.content {
                recent_window.push(' ');
                recent_window.push_str(&c.to_lowercase());
            }
        }
    }

    let has_any = |haystack: &str, kws: &[&str]| kws.iter().any(|k| haystack.contains(k));

    let mut selected: Vec<&'static str> = Vec::new();
    let mut push_unique = |name: &'static str| {
        if !selected.contains(&name) {
            selected.push(name);
        }
    };

    // 1. Finance / Expense ledger
    let finance_kws = [
        "$",
        "gast",
        "compr",
        "pagu",
        "pague",
        "cobr",
        "sueldo",
        "ingres",
        "súper",
        "super",
        "carnicer",
        "nafta",
        "alquiler",
        "expensas",
        "farmacia",
        "verdul",
        "panader",
        "uber",
        "taxi",
        "sube",
        "cuánto llev",
        "cuanto llev",
        "cuánto gast",
        "cuanto gast",
        "resumen de",
        "finanza",
        "plata",
        "dinero",
        "ledger",
        "pesos",
        "usd",
        "dólar",
        "dolar",
    ];
    if has_any(&recent_window, &finance_kws) {
        push_unique("log_expense_or_entry");
        push_unique("query_ledger");
    }

    // 2. Reminders & Google Calendar
    let schedule_kws = [
        "recorda",
        "recordá",
        "recordatorio",
        "alarma",
        "avisame",
        "agenda",
        "agendá",
        "agendar",
        "calendario",
        "calendar",
        "evento",
        "reunión",
        "reunion",
        "turno",
        "cita",
        "mañana",
        "manana",
        "lunes",
        "martes",
        "miércoles",
        "miercoles",
        "jueves",
        "viernes",
        "sábado",
        "sabado",
        "domingo",
        "hs",
        "horas",
        "todos los",
        "reprogram",
        "horario",
        "clase",
        "pendiente",
    ];
    if has_any(&recent_window, &schedule_kws) {
        push_unique("manage_reminder");
        push_unique("manage_google_calendar");
    }

    // 3. WhatsApp & contacts
    let whatsapp_kws = [
        "whatsapp",
        "wsp",
        "wasap",
        "mensaje",
        "mandale",
        "escribile",
        "enviale",
        "enviá",
        "envia",
        "chat de",
        "contacto",
        "apodo",
        "alias",
        "teléfono",
        "telefono",
        "wa_",
        "confirmation_required",
        "confirmar envío",
        "confirmar envio",
        "envialo",
        "borrador",
    ];
    if has_any(&recent_window, &whatsapp_kws) {
        push_unique("manage_whatsapp");
    }

    // 4. AI Providers, models, API keys & microkernel modules
    let sys_admin_kws = [
        "modelo",
        "modelos",
        "provider",
        "proveedor",
        "api key",
        "apikey",
        "openrouter",
        "gemini",
        "groq",
        "claude",
        "anthropic",
        "openai",
        "deepseek",
        "ollama",
        "qwen",
        "gsk_",
        "aiza",
        "sk-",
        "módulo",
        "modulo",
        "módulos",
        "modulos",
        "plugin",
        "plugins",
        "catálogo",
        "catalogo",
        "gratis",
        "free",
    ];
    if has_any(&recent_window, &sys_admin_kws)
        || !CognitiveHAL::extract_all_api_keys_from_text(prompt).is_empty()
    {
        push_unique("manage_ai_provider");
        push_unique("manage_modules");
    }

    // 5. Code, projects, background supervisors & Antigravity
    let agent_kws = [
        "proyecto",
        "supervisor",
        "agente",
        "agentes",
        "background",
        "segundo plano",
        "avance",
        "progreso",
        "antigravity",
        "pregunta del supervisor",
    ];
    if CognitiveHAL::is_technical_request(prompt)
        || CognitiveHAL::has_technical_context(prompt, history)
        || CognitiveHAL::is_status_query(prompt)
        || has_any(&recent_window, &agent_kws)
    {
        push_unique("spawn_agent");
        push_unique("answer_supervisor");
        push_unique("get_agent_status");
        push_unique("get_project_ledger");
        push_unique("send_to_antigravity");
    }

    // 6. Long-term personal memory facts
    let memory_kws = [
        "recordá que",
        "recorda que",
        "acordate",
        "no te olvides",
        "guardá en memoria",
        "guarda en memoria",
        "me llamo",
        "soy ",
        "mi pareja",
        "mi esposa",
        "mi esposo",
        "mi hijo",
        "mi hija",
        "mi trabajo",
        "prefiero",
        "intoleran",
        "alergi",
        "entreno",
        "qué sabés de mí",
        "que sabes de mi",
        "mis datos",
        "mis notas",
        "memoria",
        "olvidá",
        "olvida",
    ];
    if has_any(&recent_window, &memory_kws) {
        push_unique("remember_fact");
    }

    // 7. Short affirmative/follow-up continuity: preserve tool names invoked in the last assistant turn.
    if is_short_followup {
        if let Some(last_assistant) = history
            .iter()
            .rev()
            .find(|m| matches!(m.role, ChatRole::Assistant) && m.tool_calls.is_some())
        {
            if let Some(ref tcs) = last_assistant.tool_calls {
                for tc in tcs {
                    match tc.function.name.as_str() {
                        "spawn_agent" => push_unique("spawn_agent"),
                        "answer_supervisor" => push_unique("answer_supervisor"),
                        "send_to_antigravity" => push_unique("send_to_antigravity"),
                        "manage_ai_provider" => push_unique("manage_ai_provider"),
                        "manage_modules" => push_unique("manage_modules"),
                        "get_project_ledger" => push_unique("get_project_ledger"),
                        "get_agent_status" => push_unique("get_agent_status"),
                        "log_expense_or_entry" => push_unique("log_expense_or_entry"),
                        "query_ledger" => push_unique("query_ledger"),
                        "remember_fact" => push_unique("remember_fact"),
                        "manage_reminder" => push_unique("manage_reminder"),
                        "manage_google_calendar" => push_unique("manage_google_calendar"),
                        "manage_whatsapp" => push_unique("manage_whatsapp"),
                        _ => {}
                    }
                }
            }
        }
    }

    selected
}

/// Compacts stale `ChatRole::Tool` outputs from earlier ReAct iterations so long
/// multi-step loops don't replay megabytes of raw file reads or search dumps (CORE-408).
pub(crate) fn prune_stale_tool_outputs(
    messages: &mut [ChatMessage],
    keep_recent_tool_msgs: usize,
    max_old_chars: usize,
) {
    let tool_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter_map(|(idx, m)| matches!(m.role, ChatRole::Tool).then_some(idx))
        .collect();

    if tool_indices.len() <= keep_recent_tool_msgs {
        return;
    }

    let cutoff = tool_indices.len() - keep_recent_tool_msgs;
    for &idx in &tool_indices[..cutoff] {
        if let Some(ref mut content) = messages[idx].content {
            if content.chars().count() > max_old_chars {
                let head: String = content.chars().take(450).collect();
                let tail_rev: String = content.chars().rev().take(250).collect();
                let tail: String = tail_rev.chars().rev().collect();
                *content = format!(
                    "{}\n...[output compactado tras procesarse]...\n{}",
                    head, tail
                );
            }
        }
    }
}

impl CognitiveHAL {
    /// Construye los mensajes para el LLM usando VCM para ensamblar contexto.
    pub async fn build_messages(&self, pcb: &PCB, persona: Option<&str>) -> Vec<ChatMessage> {
        let (messages, _warnings) = self.build_messages_with_warnings(pcb, persona).await;
        messages
    }

    pub(crate) async fn get_dynamic_scripts_section(tenant_id: &str) -> String {
        let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
        let workspace_path = std::path::Path::new(&base_dir)
            .join("users")
            .join(tenant_id)
            .join("workspace");

        let mut scripts_list = String::new();

        if let Ok(entries) = std::fs::read_dir(workspace_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("js") {
                    if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                        let mut description = "No description available.".to_string();
                        let mut params_example = "{}".to_string();

                        if let Ok(content) = std::fs::read_to_string(&path) {
                            for line in content.lines().take(5) {
                                let trimmed = line.trim();
                                if trimmed.starts_with("// Description:")
                                    || trimmed.starts_with("// description:")
                                {
                                    description = trimmed
                                        .split_once(':')
                                        .map(|x| x.1)
                                        .unwrap_or("")
                                        .trim()
                                        .to_string();
                                } else if trimmed.starts_with("// Parameters:")
                                    || trimmed.starts_with("// parameters:")
                                    || trimmed.starts_with("// Example:")
                                {
                                    params_example = trimmed
                                        .split_once(':')
                                        .map(|x| x.1)
                                        .unwrap_or("")
                                        .trim()
                                        .to_string();
                                }
                            }
                        }

                        scripts_list.push_str(&format!(
                            "- **{}**: {}\n  Uso: [SYS_CALL_MAKER(\"js\", \"return eval(read_file('{}'))\", {})]\n",
                            filename, description, filename, params_example
                        ));
                    }
                }
            }
        }

        if scripts_list.is_empty() {
            String::new()
        } else {
            format!(
                "\n\nDYNAMIC SCRIPTS (HERRAMIENTAS DINÁMICAS GUARDADAS EN EL WORKSPACE):\n\
                 Podés reutilizar los siguientes scripts previamente creados y guardados en tu espacio de trabajo:\n\
                 {}\n",
                scripts_list
            )
        }
    }

    /// CORE-FIX (A3): variant that also returns any non-fatal warnings produced
    /// while assembling the context.
    pub async fn build_messages_with_warnings(
        &self,
        pcb: &PCB,
        persona: Option<&str>,
    ) -> (Vec<ChatMessage>, Vec<String>) {
        self.build_messages_internal(pcb, persona, false).await
    }

    /// Compact, cache-first chat-session variant used by `execute_with_decision`:
    /// keeps `messages[0]` (`System`) strictly static across turns and minutes so
    /// provider KV-caches hit, and omits redundant schema text already carried by `tools`.
    pub async fn build_messages_for_chat_with_warnings(
        &self,
        pcb: &PCB,
        persona: Option<&str>,
    ) -> (Vec<ChatMessage>, Vec<String>) {
        self.build_messages_internal(pcb, persona, true).await
    }

    async fn build_messages_internal(
        &self,
        pcb: &PCB,
        persona: Option<&str>,
        compact_chat: bool,
    ) -> (Vec<ChatMessage>, Vec<String>) {
        let mut warnings: Vec<String> = Vec::new();
        let casual_turn = compact_chat && is_casual_chat_turn(pcb);

        let skip_vcm = pcb.task_type == crate::pcb::TaskType::Chat
            && pcb.agent_id.is_none()
            && pcb.memory_pointers.l2_context_refs.is_empty()
            && pcb.memory_pointers.swap_refs.is_empty();

        // 1. Ensamblar contexto via VCM (L1 + L2 + L3) cuando aplique
        let assembled_context = if skip_vcm {
            pcb.memory_pointers.l1_instruction.clone()
        } else {
            self.vcm
                .assemble_context(
                    pcb,
                    &self.swap_manager,
                    self.embedding_driver.as_deref(),
                    4096,
                )
                .await
                .unwrap_or_else(|e| {
                    warn!(
                        "VCM assembly failed: {}. Falling back to raw instruction.",
                        e
                    );
                    warnings.push(format!(
                        "Context assembly failed; responding with reduced memory. Detail: {}",
                        e
                    ));
                    pcb.memory_pointers.l1_instruction.clone()
                })
        };

        let tool_prompt = if casual_turn {
            String::new()
        } else if compact_chat && pcb.agent_id.is_none() {
            // ledger & chronos are already exposed as native ChatAgent function tools
            self.plugin_manager
                .read()
                .await
                .get_available_tools_prompt_excluding(&["ledger", "chronos"])
        } else {
            self.plugin_manager
                .read()
                .await
                .get_available_tools_prompt()
        };

        // In compact_chat mode, MCP and domain module tools are serialized natively via JSON `tools`,
        // so duplicating their schemas as plain text inside `System` wastes tokens and breaks KV-cache.
        let mcp_tool_prompt = if compact_chat {
            String::new()
        } else {
            self.mcp_registry.generate_system_prompt().await
        };

        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

        let modules_tool_prompt = if compact_chat {
            String::new()
        } else if let Some(session_key) = Self::valid_pcb_session_key(pcb) {
            if let Some(ref router_lock) = *self.router.read().await {
                let router = router_lock.read().await;
                router
                    .generate_modules_prompt(
                        &pcb.memory_pointers.l1_instruction,
                        tenant_id,
                        session_key,
                    )
                    .await
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let persona_section = match persona {
            Some(p) if !p.trim().is_empty() => PERSONA_SECTION_TEMPLATE.replace("{persona}", p),
            _ => String::new(),
        };

        let has_music_plugin = !casual_turn
            && self
                .plugin_manager
                .read()
                .await
                .is_plugin_active("music_search");

        let music_section = if has_music_plugin {
            "\n\nMÚSICA — INSTRUCCIONES:\
             \n- Para reproducir: [SYS_CALL_PLUGIN(\"music_search\", {\"query\": \"<artista canción>\", \"max_results\": 1})] y luego [MUSIC_PLAY:youtube:<video_id>] (o [MUSIC_PLAY:spotify:<track_id>] si usas Spotify)\
             \n- Para pausar: responde brevemente y termina con [MUSIC_PAUSE]\
             \n- Para continuar: responde brevemente y termina con [MUSIC_RESUME]\
             \n- Para detener: responde brevemente y termina con [MUSIC_STOP]\
             \n- Para cambiar volumen: termina con [MUSIC_VOLUME:<0-100>]\
             \n- Para cambiar canción: haz una nueva búsqueda y usa [MUSIC_PLAY:youtube:<nuevo_video_id>]\
             \nNunca expliques estos tags al usuario. Solo úsalos.\n"
        } else {
            ""
        };

        let (role_instructions, instruction_source) = if pcb.agent_id.is_none() {
            let instructions = self
                .instruction_loader
                .write()
                .await
                .instructions_for(&AgentRole::ChatAgent);
            (instructions, "chat_agent.md")
        } else {
            (SYSTEM_PROMPT_MASTER.to_string(), "SYSTEM_PROMPT_MASTER")
        };

        info!(
            pid = %pcb.pid,
            source = instruction_source,
            chars = role_instructions.len(),
            "build_messages: role instructions loaded"
        );

        let has_maker_plugin =
            !casual_turn && self.plugin_manager.read().await.is_plugin_active("maker");

        let maker_section = if has_maker_plugin && pcb.agent_id.is_none() {
            let dynamic_scripts = Self::get_dynamic_scripts_section(tenant_id).await;
            if !compact_chat {
                format!("{}{}", MAKER_INSTRUCTIONS, dynamic_scripts)
            } else {
                let prompt_lower = pcb.memory_pointers.l1_instruction.to_lowercase();
                let wants_maker = !dynamic_scripts.trim().is_empty()
                    || prompt_lower.contains("script")
                    || prompt_lower.contains("javascript")
                    || prompt_lower.contains("maker")
                    || prompt_lower.contains("automatiz")
                    || prompt_lower.contains("boa");
                if wants_maker {
                    format!("{}{}", MAKER_INSTRUCTIONS, dynamic_scripts)
                } else {
                    String::new()
                }
            }
        } else {
            String::new()
        };

        let system_content = if tool_prompt.trim().is_empty()
            && mcp_tool_prompt.trim().is_empty()
            && modules_tool_prompt.trim().is_empty()
        {
            format!(
                "{}{}{}{}{}",
                maker_section,
                SPAWN_INSTRUCTIONS,
                role_instructions,
                persona_section,
                music_section
            )
        } else {
            format!(
                "{}{}{}{}{}\n\nHERRAMIENTAS DISPONIBLES:\n{}{}{}",
                maker_section,
                SPAWN_INSTRUCTIONS,
                role_instructions,
                persona_section,
                music_section,
                tool_prompt,
                mcp_tool_prompt,
                modules_tool_prompt
            )
        };

        // CORE-375 / CORE-408: Contextual personal memory (compact_chat) vs full block (legacy callers).
        let personal_memory_section = if !casual_turn && pcb.agent_id.is_none() {
            if let (Some(tid), Some(skey)) = (&pcb.tenant_id, &pcb.session_key) {
                if let Ok(db) = crate::enclave::TenantDB::open(tid, skey) {
                    if compact_chat {
                        db.build_contextual_personal_memory_block(
                            &pcb.memory_pointers.l1_instruction,
                        )
                        .unwrap_or_default()
                    } else {
                        db.build_personal_memory_prompt_block().unwrap_or_default()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let temporal_block = if pcb.agent_id.is_none() {
            use chrono::Datelike;
            let now_local = chrono::Local::now();
            let weekday_es = match now_local.weekday() {
                chrono::Weekday::Mon => "Lunes",
                chrono::Weekday::Tue => "Martes",
                chrono::Weekday::Wed => "Miércoles",
                chrono::Weekday::Thu => "Jueves",
                chrono::Weekday::Fri => "Viernes",
                chrono::Weekday::Sat => "Sábado",
                chrono::Weekday::Sun => "Domingo",
            };
            format!(
                "[CONTEXTO TEMPORAL ACTUAL]\n- Fecha y hora local del sistema: {} {}\n",
                weekday_es,
                now_local.format("%Y-%m-%d %H:%M (%:z)")
            )
        } else {
            String::new()
        };

        if compact_chat {
            // Cache-First: keep messages[0] completely invariant across turns and minutes.
            let mut messages = vec![ChatMessage {
                role: ChatRole::System,
                content: Some(system_content),
                ..Default::default()
            }];

            // CORE-260: Historial previo de la sesión
            messages.extend(pcb.message_history.clone());

            // Ephemeral per-turn operational context (current clock + selective personal memory)
            let mut dynamic_ctx = String::new();
            if !temporal_block.is_empty() {
                dynamic_ctx.push_str(&temporal_block);
            }
            if !personal_memory_section.is_empty() {
                dynamic_ctx.push_str(&personal_memory_section);
            }
            if !dynamic_ctx.is_empty() {
                messages.push(ChatMessage {
                    role: ChatRole::System,
                    content: Some(dynamic_ctx),
                    ..Default::default()
                });
            }

            messages.push(ChatMessage {
                role: ChatRole::User,
                content: Some(assembled_context),
                ..Default::default()
            });

            (messages, warnings)
        } else {
            let mut full_system_content = system_content;
            if !temporal_block.is_empty() {
                full_system_content.push_str("\n\n");
                full_system_content.push_str(&temporal_block);
            }
            if !personal_memory_section.is_empty() {
                full_system_content.push_str(&personal_memory_section);
            }

            let mut messages = vec![ChatMessage {
                role: ChatRole::System,
                content: Some(full_system_content),
                ..Default::default()
            }];

            messages.extend(pcb.message_history.clone());

            messages.push(ChatMessage {
                role: ChatRole::User,
                content: Some(assembled_context),
                ..Default::default()
            });

            (messages, warnings)
        }
    }
}

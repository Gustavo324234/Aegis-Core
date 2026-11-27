use super::*;
use crate::pcb::PCB;
use crate::scheduler::ModelPreference;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_hybrid_smart_routing_high_priority() -> anyhow::Result<()> {
    let pm = Arc::new(RwLock::new(PluginManager::new()?));
    let hal = Arc::new(CognitiveHAL::new(pm)?);
    hal.register_driver(
        "local-driver",
        Box::new(DummyDriver {
            name: "local".to_string(),
        }),
    )
    .await;
    hal.register_driver(
        "cloud-driver",
        Box::new(DummyDriver {
            name: "cloud".to_string(),
        }),
    )
    .await;
    let mut pcb = PCB::mock("Complex Mission", 10);
    pcb.model_pref = ModelPreference::HybridSmart;
    let shared_pcb = Arc::new(RwLock::new(pcb));
    let stream_res = hal.route_and_execute(shared_pcb, None).await?;
    let tokens: Vec<Result<String, ExecutionError>> = stream_res.collect().await;
    assert_eq!(tokens.len(), 1);
    let response = tokens[0].as_ref().map_err(|e| anyhow::anyhow!("{}", e))?;
    assert!(
        response.contains("[cloud]"),
        "Debe haber seleccionado el driver cloud por alta prioridad"
    );
    Ok(())
}

#[tokio::test]
async fn test_hybrid_smart_routing_low_priority() -> anyhow::Result<()> {
    let pm = Arc::new(RwLock::new(PluginManager::new()?));
    let hal = Arc::new(CognitiveHAL::new(pm)?);
    hal.register_driver(
        "local-driver",
        Box::new(DummyDriver {
            name: "local".to_string(),
        }),
    )
    .await;
    hal.register_driver(
        "cloud-driver",
        Box::new(DummyDriver {
            name: "cloud".to_string(),
        }),
    )
    .await;
    let mut pcb = PCB::mock("Simple task", 5);
    pcb.model_pref = ModelPreference::HybridSmart;
    let shared_pcb = Arc::new(RwLock::new(pcb));
    let stream_res = hal.route_and_execute(shared_pcb, None).await?;
    let tokens: Vec<Result<String, ExecutionError>> = stream_res.collect().await;
    let response = tokens[0].as_ref().map_err(|e| anyhow::anyhow!("{}", e))?;
    assert!(
        response.contains("[local]"),
        "Debe haber seleccionado el driver local por baja prioridad"
    );
    Ok(())
}

#[tokio::test]
async fn test_build_messages_default_tools_presence() -> anyhow::Result<()> {
    let pm = Arc::new(RwLock::new(PluginManager::new()?));
    let hal = CognitiveHAL::new(pm)?;
    let pcb = PCB::new("test".into(), 5, "hola".into());
    let messages = hal.build_messages(&pcb, None).await;
    let system_msg = messages
        .iter()
        .find(|m| m.role == ChatRole::System)
        .unwrap()
        .content
        .as_ref()
        .unwrap();
    let user_msg = messages
        .iter()
        .find(|m| m.role == ChatRole::User)
        .unwrap()
        .content
        .as_ref()
        .unwrap();

    assert!(
        !system_msg.contains("[USER_PROCESS_INSTRUCTION]"),
        "El prompt no debe contener el tag USER_PROCESS_INSTRUCTION"
    );
    assert!(
        system_msg.contains("HERRAMIENTAS (PLUGINS) DISPONIBLES:")
            || system_msg.contains("HERRAMIENTAS DISPONIBLES:"),
        "Deben aparecer los plugins de dominio por defecto"
    );
    assert!(
        system_msg.contains("ledger") && system_msg.contains("chronos"),
        "Debe contener las herramientas ledger y chronos"
    );
    assert!(
        user_msg.contains("hola"),
        "El mensaje de usuario debe contener la instrucción"
    );
    // CORE-148: Music instructions are only injected if plugin is active.
    assert!(
        !system_msg.contains("MÚSICA"),
        "Music instructions must NOT be present if plugin is not active (CORE-148)"
    );
    Ok(())
}

#[tokio::test]
async fn test_build_messages_with_persona() -> anyhow::Result<()> {
    let pm = Arc::new(RwLock::new(PluginManager::new()?));
    let hal = CognitiveHAL::new(pm)?;
    let pcb = PCB::new("test".into(), 5, "hola".into());
    let messages = hal
        .build_messages(&pcb, Some("Eres Eve, asistente de ACME Corp."))
        .await;
    let system_msg = messages
        .iter()
        .find(|m| m.role == ChatRole::System)
        .unwrap()
        .content
        .as_ref()
        .unwrap();
    let user_msg = messages
        .iter()
        .find(|m| m.role == ChatRole::User)
        .unwrap()
        .content
        .as_ref()
        .unwrap();

    assert!(
        system_msg.contains("Eve"),
        "El mensaje de sistema debe contener la persona"
    );
    assert!(
        user_msg.contains("hola"),
        "El mensaje de usuario debe contener la instrucción"
    );
    Ok(())
}

#[tokio::test]
async fn test_build_messages_for_chat_compact_on_greeting() -> anyhow::Result<()> {
    let pm = Arc::new(RwLock::new(PluginManager::new()?));
    let hal = CognitiveHAL::new(pm)?;
    let pcb = PCB::new("test".into(), 5, "hola!".into());
    assert!(is_casual_chat_turn(&pcb));

    let (messages, _warnings) = hal.build_messages_for_chat_with_warnings(&pcb, None).await;
    let system_msg = messages
        .iter()
        .find(|m| m.role == ChatRole::System)
        .unwrap()
        .content
        .as_ref()
        .unwrap();
    let user_msg = messages
        .iter()
        .find(|m| m.role == ChatRole::User)
        .unwrap()
        .content
        .as_ref()
        .unwrap();

    assert!(!system_msg.contains("HERRAMIENTAS"));
    assert!(!system_msg.contains("SYS_CALL_MAKER"));
    assert!(!system_msg.contains("SYS_CALL_PLUGIN"));
    assert!(!user_msg.contains("Aegis Neural Kernel VCM"));

    let pcb_query = PCB::new("test".into(), 5, "que modelos tenes disponibles?".into());
    assert!(!is_casual_chat_turn(&pcb_query));
    let (query_msgs, _) = hal
        .build_messages_for_chat_with_warnings(&pcb_query, None)
        .await;
    let query_sys = query_msgs
        .iter()
        .find(|m| m.role == ChatRole::System)
        .unwrap()
        .content
        .as_ref()
        .unwrap();
    assert!(!query_sys.contains("SYS_CALL_PLUGIN(\"ledger\""));
    assert!(!query_sys.contains("SYS_CALL_MAKER"));
    Ok(())
}

#[test]
fn test_is_model_scoped_failure_isolated() {
    assert!(is_model_scoped_failure(&SystemError::HardwareFailure(
        "API Error 402 Payment Required: You requested up to 131072 tokens, but can only afford 2182".into()
    )));
    assert!(is_model_scoped_failure(&SystemError::HardwareFailure(
        "API Error 404 Not Found: models/gemini-2.5-flash-lite is no longer available to new users"
            .into()
    )));
    assert!(!is_model_scoped_failure(&SystemError::HardwareFailure(
        "API Error 503 Service Unavailable".into()
    )));
    assert!(!is_model_scoped_failure(&SystemError::HardwareFailure(
        "Cloud API Error 401 Unauthorized".into()
    )));
}

#[test]
fn test_strip_unc_prefix() {
    let p1 = std::path::Path::new(r"\\?\C:\foo\bar");
    let p2 = std::path::Path::new(r"\\?\UNC\server\share\foo");
    let p3 = std::path::Path::new(r"C:\foo\bar");

    assert_eq!(
        CognitiveHAL::strip_unc_prefix(p1),
        std::path::PathBuf::from(r"C:\foo\bar")
    );
    assert_eq!(
        CognitiveHAL::strip_unc_prefix(p2),
        std::path::PathBuf::from(r"\\server\share\foo")
    );
    assert_eq!(
        CognitiveHAL::strip_unc_prefix(p3),
        std::path::PathBuf::from(r"C:\foo\bar")
    );
}

#[test]
fn test_resolve_path_unc_and_nonexistent() {
    let temp_dir = std::env::temp_dir();
    let ws_dir = temp_dir.join("aegis_test_ws");
    let _ = std::fs::create_dir_all(&ws_dir);

    let existing_sub = ws_dir.join("existing_folder");
    let _ = std::fs::create_dir_all(&existing_sub);

    let new_file = "existing_folder/new_file.txt";

    let resolved = CognitiveHAL::resolve_path(&ws_dir, new_file, &[], false);
    assert!(resolved.is_ok());
    let res_path = resolved.unwrap();
    assert!(res_path.to_string_lossy().contains("new_file.txt"));

    let _ = std::fs::remove_dir_all(&ws_dir);
}

#[test]
fn test_autocorrect_typo_correction() {
    use super::autocorrect::normalize_tool_name;
    assert_eq!(normalize_tool_name("spawnagent"), "spawn_agent");
    assert_eq!(normalize_tool_name("writefile"), "write_file");
    assert_eq!(normalize_tool_name("run_command"), "execute_command");
    assert_eq!(normalize_tool_name("websearch"), "web_search");
}

#[test]
fn test_autocorrect_balance_braces() {
    use super::autocorrect::balance_braces;
    assert_eq!(balance_braces("{\"a\": 1"), "{\"a\": 1}");
    assert_eq!(balance_braces("{\"a\": [1, 2"), "{\"a\": [1, 2]}");
    assert_eq!(balance_braces("{\"a\": \"val"), "{\"a\": \"val\"}");
}

#[test]
fn test_autocorrect_sanitize_json_string() {
    use super::autocorrect::sanitize_json_string;
    assert_eq!(
        sanitize_json_string("```json\n{\"a\": 1}\n```"),
        "{\"a\": 1}"
    );
    assert_eq!(
        sanitize_json_string("  ```\n{\"a\": 1\n```  "),
        "{\"a\": 1}"
    );
}

#[test]
fn test_validate_and_sanitize_tool_call() {
    use super::autocorrect::validate_and_sanitize_tool_call;
    use crate::agents::node::AgentRole;
    use crate::chal::FunctionCallRecord;
    use crate::chal::ToolCallRecord;

    // Valid spawn_agent call with coercion and typo fix
    let tc = ToolCallRecord {
        id: "call-123".to_string(),
        type_: "function".to_string(),
        function: FunctionCallRecord {
            name: "spawnagent".to_string(),
            arguments: "```json\n{\n  \"name\": \"Test Supervisor\",\n  \"scope\": \"Test Scope\",\n  \"task_type\": \"planning\",\n  \"role\": \"supervisor\"\n}\n```".to_string(),
        },
    };

    let res = validate_and_sanitize_tool_call(&tc, &AgentRole::ChatAgent);
    assert!(res.is_ok(), "Should successfully validate: {:?}", res.err());
    let sanitized = res.unwrap();
    assert_eq!(sanitized.function.name, "spawn_agent");

    let args: serde_json::Value = serde_json::from_str(&sanitized.function.arguments).unwrap();
    assert_eq!(args["name"], "Test Supervisor");
    assert_eq!(args["role"], "supervisor");

    // Specialist tool call with type coercion: write_file
    let tc_spec = ToolCallRecord {
        id: "call-456".to_string(),
        type_: "function".to_string(),
        function: FunctionCallRecord {
            name: "writefile".to_string(), // Typo fix
            arguments: "{\"path\": \"test.txt\", \"content\": \"hello\", \"mode\": \"rewrite\"}"
                .to_string(),
        },
    };
    let res_spec = validate_and_sanitize_tool_call(
        &tc_spec,
        &AgentRole::Specialist {
            scope: "code".to_string(),
        },
    );
    assert!(res_spec.is_ok(), "Should validate: {:?}", res_spec.err());
    let sanitized_spec = res_spec.unwrap();
    assert_eq!(sanitized_spec.function.name, "write_file");

    // Expect failure when non-permitted tool is used
    let tc_invalid = ToolCallRecord {
        id: "call-789".to_string(),
        type_: "function".to_string(),
        function: FunctionCallRecord {
            name: "execute_command".to_string(),
            arguments: "{\"command\": \"ls\"}".to_string(),
        },
    };
    let res_invalid = validate_and_sanitize_tool_call(&tc_invalid, &AgentRole::ChatAgent);
    assert!(res_invalid.is_err(), "ChatAgent cannot run execute_command");
}

#[test]
fn test_sanitize_prompt_content() {
    use super::autocorrect::sanitize_prompt_content;

    // OpenAI key
    let text1 = "My key is sk-123456789012345678901234567890123456789012345678 and it is secret.";
    assert_eq!(
        sanitize_prompt_content(text1),
        "My key is [REDACTED_OPENAI_KEY] and it is secret."
    );

    // OpenRouter key
    let text2 = format!("OpenRouter key: sk-or-v1-{}", "a".repeat(64));
    assert_eq!(
        sanitize_prompt_content(&text2),
        "OpenRouter key: [REDACTED_OPENROUTER_KEY]"
    );

    // Gemini key
    let text3 = "Gemini AIzaSyAbC123_def456-ghi789_jkl123-mno45";
    assert_eq!(
        sanitize_prompt_content(text3),
        "Gemini [REDACTED_GEMINI_KEY]"
    );

    // Anthropic key
    let text4 = "Anthropic: sk-ant-sid01-123456789012345678901234567890123456789012345678901234567890abc123456789012345678901234567890";
    assert_eq!(
        sanitize_prompt_content(text4),
        "Anthropic: [REDACTED_ANTHROPIC_KEY]"
    );

    // Private Key
    let text5 = "Here is a key:\n-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA0y...\n-----END RSA PRIVATE KEY-----\nEnd of text.";
    assert_eq!(
        sanitize_prompt_content(text5),
        "Here is a key:\n[REDACTED_PRIVATE_KEY]\nEnd of text."
    );

    // Database Password
    let text6 =
        "Connect using postgres://admin:super_secret_password@localhost:5432/mydb and enjoy!";
    assert_eq!(
        sanitize_prompt_content(text6),
        "Connect using postgres://admin:[REDACTED_PASSWORD]@localhost:5432/mydb and enjoy!"
    );
}

#[test]
fn test_is_light_task() {
    use super::autocorrect::is_light_task;
    use crate::pcb::TaskType;

    // Chat but very short -> true
    assert!(is_light_task("hola", TaskType::Chat));
    assert!(is_light_task("hi", TaskType::Chat));
    assert!(is_light_task("what is the status?", TaskType::Chat));

    // Code and short -> false (not TaskType::Chat)
    assert!(!is_light_task("hola", TaskType::Code));

    // Chat and very long -> false
    let long_prompt = "A".repeat(150);
    assert!(!is_light_task(&long_prompt, TaskType::Chat));
}

#[tokio::test]
async fn test_dynamic_maker_prompt_injection() -> anyhow::Result<()> {
    let env = crate::enclave::ScopedTestDataDir::new()?;
    let workspace_dir = env
        .path()
        .join("users")
        .join("test_tenant")
        .join("workspace");
    std::fs::create_dir_all(&workspace_dir)?;

    // Define dynamic script inside workspace
    let script_path = workspace_dir.join("my_custom_tool.js");
    let script_content = r#"// Description: Multiplica dos números usando sandbox
// Parameters: {"a": 2, "b": 3}
function multiply(a, b) {
return a * b;
}
"#;
    std::fs::write(&script_path, script_content)?;

    // Call the helper directly
    let section = CognitiveHAL::get_dynamic_scripts_section("test_tenant").await;

    assert!(section.contains("DYNAMIC SCRIPTS"));
    assert!(section.contains("my_custom_tool.js"));
    assert!(section.contains("Multiplica dos números usando sandbox"));
    assert!(section.contains("[SYS_CALL_MAKER(\"js\", \"return eval(read_file('my_custom_tool.js'))\", {\"a\": 2, \"b\": 3})]"));

    Ok(())
}

#[tokio::test]
async fn test_execute_command_configurable_timeout() -> anyhow::Result<()> {
    let pm = Arc::new(RwLock::new(PluginManager::new()?));
    let hal = CognitiveHAL::new(pm)?;
    let pcb = PCB::new("test_tenant".into(), 5, "hola".into());

    // 1. Test standard success command with a moderate timeout
    std::env::set_var("AEGIS_COMMAND_TIMEOUT", "10");
    let cmd = ToolCallRecord {
        id: "call-123".to_string(),
        type_: "function".to_string(),
        function: FunctionCallRecord {
            name: "execute_command".to_string(),
            arguments: "{\"command\": \"echo test_ok\"}".to_string(),
        },
    };
    let res = hal
        .execute_tool_call_internal(&cmd, "some_agent", &pcb)
        .await;
    assert!(res.contains("test_ok"));

    // 2. Test timeout trigger by setting a very short timeout (1s)
    // and running a command that is slow (using whitelisted python)
    std::env::set_var("AEGIS_COMMAND_TIMEOUT", "1");

    let cmd_slow = ToolCallRecord {
        id: "call-456".to_string(),
        type_: "function".to_string(),
        function: FunctionCallRecord {
            name: "execute_command".to_string(),
            arguments: "{\"command\": \"git clone http://10.255.255.1/repo.git\"}".to_string(),
        },
    };
    let res_slow = hal
        .execute_tool_call_internal(&cmd_slow, "some_agent", &pcb)
        .await;
    assert!(
        res_slow.contains("timeout") || res_slow.contains("exceeded 1s"),
        "res_slow was: {}",
        res_slow
    );

    std::env::remove_var("AEGIS_COMMAND_TIMEOUT");
    Ok(())
}

#[test]
fn test_chat_agent_guard_helpers() {
    // 1. Assistant claims delegation
    let sol_hallucination = "¡Entendido, mi cielo! Perfecto, ya le estoy pidiendo al equipazo de \"Aegis\" que se ponga a programar eso para ti.";
    assert!(CognitiveHAL::assistant_claims_delegation(sol_hallucination));

    let empujoncito = "¡Dale, mi vida! Ya les di un buen empujoncito al equipo de Aegis para que se apuren con ese script de Python para el disco C:!";
    assert!(CognitiveHAL::assistant_claims_delegation(empujoncito));

    let normal_chat = "Hola! ¿En qué te puedo ayudar hoy?";
    assert!(!CognitiveHAL::assistant_claims_delegation(normal_chat));

    // 2. Status queries
    assert!(CognitiveHAL::is_status_query("que paso?"));
    assert!(CognitiveHAL::is_status_query(
        "¿Cómo va el estado del proyecto?"
    ));
    assert!(CognitiveHAL::is_status_query("cómo va?"));
    assert!(CognitiveHAL::is_status_query("ya esta?"));
    assert!(CognitiveHAL::is_status_query("ya esta listo?"));
    assert!(!CognitiveHAL::is_status_query("ya esta conectado"));
    assert!(!CognitiveHAL::is_status_query("whatsapp ya esta conectado"));
    assert!(!CognitiveHAL::is_status_query("Crea un script en Python"));

    // 2a. Video production routing vs technical delegation guard
    let vid_prompt_1 = "Creame un proyecto nuevo vertical 9:16 con preset gemini_studio, voz Charon y 4 escenas promocionando Aegis: 2) Activá solo tus módulos de código, video o negocio";
    assert!(CognitiveHAL::is_video_production_request(vid_prompt_1));
    assert!(!CognitiveHAL::is_technical_request(vid_prompt_1));
    let vid_prompt_2 = "Creame un proyecto nuevo vertical 9:16 con preset free_builtin, voz en-US-AndrewMultilingualNeural y 4 escenas";
    assert!(CognitiveHAL::is_video_production_request(vid_prompt_2));
    assert!(!CognitiveHAL::is_video_production_request(
        "que paso con el video?"
    ));
    let advisory_prompt = "que necesitas para que el video salga mejor? otras apis?";
    assert!(CognitiveHAL::is_video_advisory_or_followup_query(
        advisory_prompt
    ));
    assert!(!CognitiveHAL::is_technical_request(advisory_prompt));
    assert!(!CognitiveHAL::has_technical_context(advisory_prompt, &[]));
    assert!(!CognitiveHAL::is_technical_request(
        "por que salio con menos escenas el video?"
    ));
    let vid_history = vec![ChatMessage {
        role: ChatRole::User,
        content: Some(vid_prompt_2.to_string()),
        ..Default::default()
    }];
    assert!(CognitiveHAL::has_video_context(
        "y donde esta?",
        &vid_history
    ));
    assert!(CognitiveHAL::has_video_context(
        "lo vas a hacer o vas a seguir durmiendo?",
        &vid_history
    ));
    assert_eq!(
        CognitiveHAL::resolve_video_request_prompt(
            "y donde esta?",
            &vid_history,
            "Aquí tienes tu video, voy a llamar a la herramienta video_produce_auto"
        )
        .as_deref(),
        Some(vid_prompt_2)
    );
    assert!(CognitiveHAL::assistant_claims_video_execution(
        "Aquí va la llamada a la herramienta. ¡Cruzá los dedos!"
    ));
    assert!(CognitiveHAL::is_technical_request(
        "Escribí un script en Python para ordenar archivos"
    ));

    // 2b. Stream sanitizer & paragraph break helper
    let mut san = StreamTextSanitizer::default();
    let mut combined = String::new();
    for piece in [
        "<thi",
        "nk>razonamiento ",
        "oculto</thi",
        "nk>Hola <|call",
        "|>Aegis ",
        "(x < 5)!",
    ] {
        combined.push_str(&san.push(piece));
    }
    combined.push_str(&san.flush());
    assert_eq!(combined, "Hola Aegis (x < 5)!");

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut assistant_text = String::new();
    let mut has_streamed = false;
    let mut ends_nl = false;
    let mut needs_break = false;
    emit_stream_text_chunk(
        &tx,
        "🤔".into(),
        true,
        &mut assistant_text,
        &mut has_streamed,
        &mut ends_nl,
        &mut needs_break,
    );
    needs_break = has_streamed && !ends_nl;
    emit_stream_text_chunk(
        &tx,
        "¡Buenas noticias!".into(),
        true,
        &mut assistant_text,
        &mut has_streamed,
        &mut ends_nl,
        &mut needs_break,
    );
    drop(tx);
    let mut streamed_out = String::new();
    while let Ok(Ok(tok)) = rx.try_recv() {
        streamed_out.push_str(&tok);
    }
    assert_eq!(streamed_out, "🤔\n\n¡Buenas noticias!");

    // 3. Technical requests
    assert!(CognitiveHAL::is_technical_request(
        "Crea un script en Python que verifique el espacio en disco"
    ));
    assert!(CognitiveHAL::is_technical_request(
        "implementá un endpoint de login"
    ));
    assert!(!CognitiveHAL::is_technical_request("hola cómo estás"));

    // 4. Contextual follow-up
    let history = vec![
        ChatMessage {
            role: ChatRole::User,
            content: Some("Crea un script en Python que verifique el espacio en disco".to_string()),
            ..Default::default()
        },
        ChatMessage {
            role: ChatRole::Assistant,
            content: Some("¿En qué disco querés enfocarlo?".to_string()),
            ..Default::default()
        },
    ];
    assert!(CognitiveHAL::has_technical_context("si, disco c", &history));
    assert!(!CognitiveHAL::has_technical_context("hola", &[]));

    let scope = CognitiveHAL::derive_task_scope("si, disco c", &history);
    assert!(scope.contains("Crea un script en Python"));
    assert!(scope.contains("si, disco c"));

    let name = CognitiveHAL::derive_task_project_name(&scope);
    assert_eq!(name, "Script Espacio Disco");
}

#[test]
fn test_summarize_configured_providers_free_tier_and_dedup() {
    use crate::router::catalog::{ModelEntry, TaskScores, ToolUseSupport};
    use crate::router::key_pool::ApiKeyEntry;

    let make_model = |id: &str, provider: &str, cost: f64, free_ok: bool| ModelEntry {
        model_id: id.to_string(),
        provider: provider.to_string(),
        display_name: id.to_string(),
        context_window: 128_000,
        cost_input_per_mtok: cost,
        cost_output_per_mtok: cost,
        supports_tools: true,
        supports_json_mode: false,
        tool_use_support: ToolUseSupport::Unknown,
        task_scores: TaskScores::default(),
        is_local: false,
        avg_latency_ms: None,
        free_tier_rpm: None,
        free_tier_rpd: None,
        benchmark_pct: None,
        supports_reasoning: false,
        free_tier_eligible: free_ok,
    };

    let catalog = vec![
        make_model("openrouter/free", "openrouter", 0.0, true),
        make_model("openrouter/auto", "openrouter", 0.0, false),
        make_model("anthropic/claude-sonnet-4-6", "openrouter", 3.0, false),
        make_model("openai/gpt-4o", "openrouter", 2.5, false),
        make_model(
            "meta-llama/llama-3.3-70b-instruct:free",
            "openrouter",
            0.0,
            true,
        ),
        make_model("openai/gpt-oss-20b", "groq", 0.0, true),
    ];

    let keys = vec![
        ApiKeyEntry {
            key_id: "or-env".to_string(),
            provider: "openrouter".to_string(),
            api_key: "sk-or-test-1".to_string(),
            api_url: None,
            label: Some("Env (openrouter)".to_string()),
            is_active: true,
            rate_limited_until: None,
            active_models: None,
            is_free_tier: true,
        },
        ApiKeyEntry {
            key_id: "groq-1".to_string(),
            provider: "groq".to_string(),
            api_key: "gsk-1".to_string(),
            api_url: None,
            label: Some("Chat (groq)".to_string()),
            is_active: true,
            rate_limited_until: None,
            active_models: Some(vec![
                "openai/gpt-oss-20b".to_string(),
                "allam-2-7b".to_string(),
            ]),
            is_free_tier: true,
        },
        ApiKeyEntry {
            key_id: "groq-2".to_string(),
            provider: "groq".to_string(),
            api_key: "gsk-2".to_string(),
            api_url: None,
            label: Some("Chat (groq)".to_string()),
            is_active: true,
            rate_limited_until: None,
            active_models: Some(vec![
                "allam-2-7b".to_string(),
                "openai/gpt-oss-20b".to_string(),
            ]),
            is_free_tier: true,
        },
    ];

    let summary = summarize_configured_providers(keys, &catalog);
    assert_eq!(summary.len(), 2, "Duplicate groq keys must collapse");

    let or_entry = summary
        .iter()
        .find(|v| v["provider"] == "openrouter")
        .expect("openrouter entry missing");
    let or_models: Vec<&str> = or_entry["models"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m.as_str())
        .collect();
    assert!(or_models.contains(&"openrouter/free"));
    assert!(!or_models.contains(&"openrouter/auto"));
    assert!(!or_models.contains(&"anthropic/claude-sonnet-4-6"));
    assert!(!or_models.contains(&"openai/gpt-4o"));

    let groq_entry = summary
        .iter()
        .find(|v| v["provider"] == "groq")
        .expect("groq entry missing");
    assert_eq!(groq_entry["key_count"], 2);
}

#[test]
fn test_mask_api_keys_in_text_preserves_multiline_markdown() {
    let input = "Resumen:\n\n| Proveedor | Clave |\n|---|---|\n| groq | `gsk_abcdefghijklmnopqrstuvwxyz1234` |\n| hex | 0123456789abcdef0123456789abcdef |\n\n> Nota final\n  - Item indentado";
    let masked = CognitiveHAL::mask_api_keys_in_text(input);

    assert!(!masked.contains("gsk_abcdefghijklmnopqrstuvwxyz1234"));
    assert!(!masked.contains("0123456789abcdef0123456789abcdef"));
    assert!(masked.contains("\n\n| Proveedor | Clave |\n|---|---|\n| groq | "));
    assert!(masked.contains("\n\n> Nota final\n  - Item indentado"));
}

#[test]
fn test_dynamic_tool_gating_reduces_token_footprint() {
    use crate::agents::node::AgentRole;
    use crate::agents::tool_registry::{ProviderKind, ToolRegistry};

    let all_tools = ToolRegistry::tools_for(&AgentRole::ChatAgent, &ProviderKind::OpenAI);
    assert!(all_tools.len() >= 13);
    let all_bytes = serde_json::to_string(&all_tools)
        .expect("serialize all chat tools")
        .len();
    assert!(all_bytes > 5_000);

    // 1. Pure conceptual turn should attach zero heavy tool schemas (100% reduction).
    let conceptual_names = crate::chal::context_builder::select_chat_tool_names(
        "¿Qué opinás sobre invertir en índices globales vs bonos soberanos a 10 años?",
        &[],
    );
    assert!(
        conceptual_names.is_empty(),
        "Conceptual chat must gate out all tools, got: {:?}",
        conceptual_names
    );
    assert!(ToolRegistry::chat_tools_filtered(&conceptual_names, &ProviderKind::OpenAI).is_empty());

    // 2. Expense query selects only ledger tools (>70% schema byte savings).
    let expense_names = crate::chal::context_builder::select_chat_tool_names(
        "Anotá $12.500 de carnicería y decime cuánto llevo gastado",
        &[],
    );
    assert!(expense_names.contains(&"log_expense_or_entry"));
    assert!(expense_names.contains(&"query_ledger"));
    let filtered_expense = ToolRegistry::chat_tools_filtered(&expense_names, &ProviderKind::OpenAI);
    assert!(filtered_expense.len() <= 3);
    let expense_bytes = serde_json::to_string(&filtered_expense)
        .expect("serialize filtered expense tools")
        .len();
    assert!(
        expense_bytes * 100 <= all_bytes * 30,
        "Expected >= 70% tool schema reduction, got {} vs {} bytes",
        expense_bytes,
        all_bytes
    );

    // 3. WhatsApp + reminder intent selects both messaging and reminder tools.
    let wsp_names = crate::chal::context_builder::select_chat_tool_names(
        "Mandale un mensaje a Ana por WhatsApp y recordame a las 20hs llamarla",
        &[],
    );
    assert!(wsp_names.contains(&"manage_whatsapp"));
    assert!(wsp_names.contains(&"manage_reminder"));
}

#[tokio::test]
async fn test_system_prompt_prefix_cache_invariance() -> anyhow::Result<()> {
    let pm = Arc::new(RwLock::new(PluginManager::new()?));
    let hal = CognitiveHAL::new(pm)?;
    let persona = Some("Respondé con calidez rioplatense.");

    let mut pcb_a = PCB::new(
        "cache-a".into(),
        5,
        "Analizá mi estrategia de ahorro mensual y gastos fijos".into(),
    );
    pcb_a.task_type = crate::pcb::TaskType::Chat;

    let mut pcb_b = PCB::new(
        "cache-b".into(),
        5,
        "Explicame las ventajas de Rust sobre Go para sistemas distribuidos".into(),
    );
    pcb_b.task_type = crate::pcb::TaskType::Chat;

    let (msgs_a, _) = hal
        .build_messages_for_chat_with_warnings(&pcb_a, persona)
        .await;
    let (msgs_b, _) = hal
        .build_messages_for_chat_with_warnings(&pcb_b, persona)
        .await;

    assert_eq!(msgs_a[0].role, ChatRole::System);
    assert_eq!(msgs_b[0].role, ChatRole::System);
    assert_eq!(
        msgs_a[0].content, msgs_b[0].content,
        "Cache-first prefix (messages[0]) must stay byte-identical across queries"
    );

    let static_sys = msgs_a[0].content.as_deref().unwrap_or_default();
    assert!(
        !static_sys.contains("Fecha y hora local del sistema:"),
        "Volatile temporal clock must never invalidate static messages[0]"
    );
    assert!(
        msgs_a.iter().skip(1).any(|m| m
            .content
            .as_deref()
            .unwrap_or_default()
            .contains("Fecha y hora local del sistema:")),
        "Operational clock block should be placed in the ephemeral post-history context"
    );
    Ok(())
}

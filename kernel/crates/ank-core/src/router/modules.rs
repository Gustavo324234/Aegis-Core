use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcTransport {
    pub protocol: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleDatabase {
    pub driver: String,
    pub encryption: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExposedTool {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleManifest {
    pub module_id: String,
    pub display_name: String,
    pub version: String,
    pub ipc_transport: IpcTransport,
    pub database: ModuleDatabase,
    pub exposed_tools: Vec<ExposedTool>,
    #[serde(default)]
    pub ui_views: Vec<Value>,
}

/// Recursively scans the given directory to find subfolders containing a `module.json` manifest.
/// Parsed manifests are validated and returned in a thread-safe registry map.
pub fn load_modules_from_dir<P: AsRef<Path>>(
    dir_path: P,
) -> anyhow::Result<HashMap<String, ModuleManifest>> {
    let mut modules = HashMap::new();
    let path = dir_path.as_ref();
    if !path.exists() {
        warn!(
            "Microkernel modules directory not found at: {}",
            path.display()
        );
        return Ok(modules);
    }

    if !path.is_dir() {
        return Err(anyhow::anyhow!(
            "Specified modules path is not a directory: {}",
            path.display()
        ));
    }

    // Scan subdirectories for module.json
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_dir() {
            let manifest_path = entry_path.join("module.json");
            if manifest_path.exists() && manifest_path.is_file() {
                match load_manifest(&manifest_path) {
                    Ok(manifest) => {
                        info!(
                            "Microkernel module '{}' ({}) loaded successfully with {} tools from {}",
                            manifest.display_name,
                            manifest.module_id,
                            manifest.exposed_tools.len(),
                            manifest_path.display()
                        );
                        modules.insert(manifest.module_id.clone(), manifest);
                    }
                    Err(e) => {
                        error!(
                            "Failed to parse microkernel module manifest at {}: {}",
                            manifest_path.display(),
                            e
                        );
                    }
                }
            }
        }
    }

    Ok(modules)
}

fn load_manifest(path: &Path) -> anyhow::Result<ModuleManifest> {
    let content = fs::read_to_string(path)?;
    let manifest: ModuleManifest = serde_json::from_str(&content)?;
    Ok(manifest)
}

/// A high-performance cognitive routing heuristic classifier that assesses the user prompt
/// against the module's target domains and keywords to decide if the tool schemas should be injected.
pub fn match_prompt_to_module(prompt: &str, manifest: &ModuleManifest) -> bool {
    let lower_prompt = prompt.to_lowercase();
    let module_id = manifest.module_id.to_lowercase();
    let display_name = manifest.display_name.to_lowercase();

    // 1. Heuristics for Business & Commerce domain (Aegis-Biz)
    if module_id.contains("business")
        || module_id.contains("biz")
        || display_name.contains("business")
        || display_name.contains("negocio")
        || display_name.contains("store")
        || display_name.contains("tienda")
    {
        let biz_keywords = &[
            "stock",
            "inventario",
            "facturación",
            "facturacion",
            "contabilidad",
            "venta",
            "ventas",
            "punto de venta",
            "pos",
            "catálogo",
            "catalogo",
            "código de barras",
            "codigo de barras",
            "mercadería",
            "mercaderia",
            "aegis-biz",
            "aegis.domain.business",
        ];
        let biz_phrases = &[
            "crear tienda",
            "crear negocio",
            "crear comercio",
            "crear store",
            "crear proyecto de tienda",
            "crear proyecto de la tienda",
            "proyecto de la tienda",
            "proyecto de tienda",
            "nueva tienda",
            "nuevo negocio",
            "registrar producto",
            "registrar venta",
            "vendí ",
            "vendi ",
            "tienda de",
            "negocio de",
        ];
        if biz_keywords.iter().any(|&kw| lower_prompt.contains(kw))
            || biz_phrases
                .iter()
                .any(|&phrase| lower_prompt.contains(phrase))
        {
            return true;
        }
    }

    // 2. Heuristics for Security & Auditing domain (Aegis-Sec)
    if module_id.contains("security")
        || module_id.contains("sec")
        || display_name.contains("security")
        || display_name.contains("cybersecurity")
        || display_name.contains("ciberseguridad")
    {
        let sec_keywords = &[
            "ciberseguridad",
            "cybersecurity",
            "pentest",
            "nmap",
            "vulnerabilidades",
            "hacking",
            "sandbox",
            "jailbreak",
            "firewall",
            "exploit",
            "aegis-sec",
        ];
        let sec_phrases = &[
            "escanear puertos",
            "auditoría de seguridad",
            "auditoria de seguridad",
            "seguridad informática",
            "seguridad informatica",
            "red team",
            "cyber attack",
        ];
        if sec_keywords.iter().any(|&kw| lower_prompt.contains(kw))
            || sec_phrases
                .iter()
                .any(|&phrase| lower_prompt.contains(phrase))
        {
            return true;
        }
    }

    // 3. Heuristics for Developer & Programming domain (Aegis-Dev)
    if module_id.contains("dev")
        || module_id.contains("programming")
        || display_name.contains("programming")
        || display_name.contains("desarrollo")
        || display_name.contains("código")
    {
        let dev_keywords = &[
            "compilar",
            "compiler",
            "depurar",
            "debugger",
            "snippet",
            "snippets",
            "kanban",
            "bug",
            "bugs",
            "refactor",
            "pull request",
            "repositorio",
            "runbook",
            "programación",
            "programacion",
            "aegis-dev",
            "aegis.domain.dev",
        ];
        let dev_phrases = &[
            "escribir código",
            "escribir codigo",
            "compilar proyecto",
            "ejecutar test",
            "ejecutar pruebas",
            "proyecto de programación",
            "proyecto de programacion",
            "tarea técnica",
            "tarea tecnica",
            "guardar snippet",
        ];
        if dev_keywords.iter().any(|&kw| lower_prompt.contains(kw))
            || dev_phrases
                .iter()
                .any(|&phrase| lower_prompt.contains(phrase))
        {
            return true;
        }
    }

    // 4. Heuristics for Social Media & Content Creation domain (Aegis Creator Studio)
    if module_id.contains("creator")
        || module_id.contains("social")
        || module_id.contains("content")
        || display_name.contains("creator")
        || display_name.contains("contenido")
    {
        let creator_keywords = &[
            "reel",
            "reels",
            "tiktok",
            "shorts",
            "youtube",
            "guion",
            "guión",
            "guiones",
            "hook",
            "hooks",
            "viral",
            "teleprompter",
            "calendario editorial",
            "instagram",
            "linkedin",
            "carrusel",
            "miniatura",
            "thumbnail",
            "creador",
            "aegis-creator",
            "aegis.domain.creator",
        ];
        let creator_phrases = &[
            "redes sociales",
            "crear contenido",
            "creación de contenido",
            "creacion de contenido",
            "post para",
            "hilo de x",
            "hilo para x",
            "guion para",
            "guión para",
            "banco de ideas",
            "idea de video",
            "ideas de video",
        ];
        if creator_keywords.iter().any(|&kw| lower_prompt.contains(kw))
            || creator_phrases
                .iter()
                .any(|&phrase| lower_prompt.contains(phrase))
        {
            return true;
        }
    }

    // 5. Heuristics for AI Video Production & Studio domain (Aegis Video Studio)
    if module_id.contains("video")
        || display_name.contains("video studio")
        || display_name.contains("video")
    {
        let video_keywords = &[
            "video",
            "videos",
            "video studio",
            "storyboard",
            "google veo",
            "edge tts",
            "edge-tts",
            "elevenlabs",
            "ffmpeg",
            "pexels",
            "locución",
            "locucion",
            "voiceover",
            "voz en off",
            "subtítulos",
            "subtitulos",
            "renderizar",
            "exportar mp4",
            "mp4",
            "9:16",
            "16:9",
            "escena",
            "escenas",
            "cortometraje",
            "preset",
            "free_builtin",
            "gemini_studio",
            "hybrid_pro",
            "charon",
            "andrewmultilingual",
            "reproductor",
            "publicitar",
            "promocionando",
            "promocional",
            "aegis-video",
            "aegis.domain.video",
        ];
        let video_phrases = &[
            "crear video",
            "crear un video",
            "creame un video",
            "proyecto vertical",
            "generar video",
            "generar un video",
            "video con ia",
            "video ia",
            "editar video",
            "editar un video",
            "hacer un video",
            "guion a video",
            "guión a video",
            "crear escenas",
        ];
        if video_keywords.iter().any(|&kw| lower_prompt.contains(kw))
            || video_phrases
                .iter()
                .any(|&phrase| lower_prompt.contains(phrase))
        {
            return true;
        }
    }

    // Fallback direct match: If prompt explicitly references module name or any tool name
    if lower_prompt.contains(&module_id) || lower_prompt.contains(&display_name) {
        return true;
    }

    for tool in &manifest.exposed_tools {
        if lower_prompt.contains(&tool.name.to_lowercase()) {
            return true;
        }
    }

    false
}

/// Returns native `ToolDefinition`s for every domain module currently active for `tenant_id`.
pub fn collect_enabled_module_tools(
    modules: &HashMap<String, ModuleManifest>,
    tenant_id: &str,
    session_key: &str,
) -> Vec<crate::pcb::ToolDefinition> {
    let Ok(db) = crate::enclave::TenantDB::open(tenant_id, session_key) else {
        return Vec::new();
    };
    let mut keys: Vec<&String> = modules.keys().collect();
    keys.sort();
    let mut defs = Vec::new();
    for mod_id in keys {
        let is_enabled = db
            .get_kv(&format!("module_active:{}", mod_id))
            .unwrap_or(None)
            .map(|v| v == "true")
            .unwrap_or(false);
        if !is_enabled {
            continue;
        }
        if let Some(manifest) = modules.get(mod_id) {
            for tool in &manifest.exposed_tools {
                defs.push(crate::pcb::ToolDefinition::new(
                    tool.name.clone(),
                    format!("[Módulo {}] {}", manifest.display_name, tool.description),
                    tool.parameters.clone(),
                ));
            }
        }
    }
    defs
}

/// Generates a markdown system prompt specifying active module schemas or available suggestions based on TenantDB activation.
pub fn generate_system_prompt_for_modules(
    modules: &HashMap<String, ModuleManifest>,
    prompt: &str,
    tenant_id: &str,
    session_key: &str,
) -> String {
    let mut relevant_enabled = Vec::new();
    let mut relevant_suggested = Vec::new();

    let db = crate::enclave::TenantDB::open(tenant_id, session_key).ok();

    for manifest in modules.values() {
        let is_enabled = if let Some(ref tenant_db) = db {
            tenant_db
                .get_kv(&format!("module_active:{}", manifest.module_id))
                .unwrap_or(None)
                .map(|v| v == "true")
                .unwrap_or(false)
        } else {
            false
        };

        if match_prompt_to_module(prompt, manifest) {
            if is_enabled {
                relevant_enabled.push(manifest);
            } else {
                relevant_suggested.push(manifest);
            }
        }
    }

    let mut system_prompt = String::new();

    // 1. Inject concise schemas for already enabled modules (avoid repeating full JSON Schema blobs already sent via native `tools`)
    if !relevant_enabled.is_empty() {
        system_prompt.push_str("\n## HERRAMIENTAS DE MÓDULOS DE DOMINIO (MICROKERNEL) ACTIVAS\n");
        system_prompt.push_str("Invocá directamente estas herramientas mediante Tool Calling nativo (o `[SYS_MCP_EXEC(tool_name, {args})]` como respaldo):\n\n");
        for manifest in relevant_enabled {
            system_prompt.push_str(&format!(
                "### {} ({})\n",
                manifest.display_name, manifest.version
            ));
            for tool in &manifest.exposed_tools {
                let param_names = tool
                    .parameters
                    .get("properties")
                    .and_then(Value::as_object)
                    .map(|props| {
                        let mut keys: Vec<&str> = props.keys().map(String::as_str).collect();
                        keys.sort();
                        keys.join(", ")
                    })
                    .unwrap_or_default();
                if param_names.is_empty() {
                    system_prompt.push_str(&format!("- **{}**: {}\n", tool.name, tool.description));
                } else {
                    system_prompt.push_str(&format!(
                        "- **{}** ({}): {}\n",
                        tool.name, param_names, tool.description
                    ));
                }
            }
            if manifest.module_id == "aegis.domain.video" {
                system_prompt.push_str(
                    "\nRegla Video Studio: Preferí llamar a `video_produce_auto` en UN SOLO PASO pasando SIEMPRE el arreglo explícito `scenes` (`[{title, narration_text, on_screen_text, visual_prompt, stock_keywords}]`) con una escena por cada bloque o tema que pidió el usuario y locución `narration_text` natural lista para leer en voz alta (nunca copies instrucciones técnicas en `narration_text`). La tool instala el motor si falta, genera voces y visuales con fallback automático, exporta el MP4 y devuelve `[AEGIS_VIDEO_CARD:<project_id>]` + diagnóstico `advisor`. Si algún motor pedido tuvo que adaptarse por límites de plan gratuito o hardware local, mencioná brevemente el aviso de `fallbacks_applied` y las recomendaciones de `advisor` (planes de API o modelos locales según la PC del usuario) junto con `[AEGIS_VIDEO_CARD:<project_id>]`. PROHIBIDO usar `spawn_agent` para pedidos de video: ejecutá siempre `video_*` directamente y nunca digas que no se puede descargar ni muestres rutas del sistema de archivos.\n",
                );
            }
            system_prompt.push('\n');
        }
    }

    // 2. Inject suggestion instructions for available but inactive modules
    if !relevant_suggested.is_empty() {
        system_prompt.push_str("\n## MÓDULOS DE DOMINIO DISPONIBLES (INSTALACIÓN BAJO DEMANDA)\n");
        system_prompt.push_str("Los siguientes módulos del microkernel son relevantes para el pedido del usuario pero aún NO están activos en su enclave.\n\
                                - Si el usuario simplemente pide hacer una tarea de ese dominio por primera vez, ofrecé activar el módulo correspondiente (explicando brevemente qué herramientas/Mini-App suma) antes de usar sus herramientas.\n\
                                - Si el usuario ya confirmó o pidió expresamente instalarlo/activarlo, ejecutá `manage_modules(action=\"enable\", module_id=\"...\")` (o `[SYS_ENABLE_MODULE(\"module_id\")]`) y luego respondé.\n\n");
        for manifest in relevant_suggested {
            system_prompt.push_str(&format!(
                "- **{}** (ID: `{}`): Expone herramientas especializadas para este dominio.\n",
                manifest.display_name, manifest.module_id
            ));
            system_prompt.push_str(&format!(
                "  Privileged Syscall to enable: `[SYS_ENABLE_MODULE(\"{}\")]`\n\n",
                manifest.module_id
            ));
        }
    }

    system_prompt
}

// ── Pool de Canales gRPC para Módulos del Microkernel (CORE-353) ─────────────

static MODULE_CHANNEL_POOL: std::sync::LazyLock<
    tokio::sync::RwLock<HashMap<String, tonic::transport::Channel>>,
> = std::sync::LazyLock::new(|| tokio::sync::RwLock::new(HashMap::new()));

/// Normaliza un endpoint de módulo garantizando esquema `http://` o `https://`.
pub fn normalize_module_endpoint(endpoint: &str) -> String {
    let trimmed = endpoint.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{}", trimmed)
    }
}

async fn get_or_connect_module_channel(
    channel_url: &str,
) -> Result<tonic::transport::Channel, String> {
    {
        let pool = MODULE_CHANNEL_POOL.read().await;
        if let Some(ch) = pool.get(channel_url) {
            return Ok(ch.clone());
        }
    }

    let endpoint_parsed = tonic::transport::Endpoint::from_shared(channel_url.to_string())
        .map_err(|e| format!("Invalid endpoint format: {}", e))?
        .timeout(std::time::Duration::from_secs(5))
        .connect_timeout(std::time::Duration::from_secs(5))
        .http2_keep_alive_interval(std::time::Duration::from_secs(30))
        .keep_alive_while_idle(true);

    let channel = endpoint_parsed
        .connect()
        .await
        .map_err(|e| format!("Failed to connect to microkernel module: {}", e))?;

    let mut pool = MODULE_CHANNEL_POOL.write().await;
    pool.insert(channel_url.to_string(), channel.clone());
    Ok(channel)
}

async fn evict_module_channel(channel_url: &str) {
    let mut pool = MODULE_CHANNEL_POOL.write().await;
    pool.remove(channel_url);
}

/// Invoca una herramienta de un módulo externo vía gRPC reutilizando el pool de canales HTTP/2 (CORE-353).
/// Si el canal cacheado falla por error de transporte (`Unavailable` / `Unknown`), se evicta y reintenta una vez.
pub async fn call_module_tool_pooled(
    endpoint: &str,
    tool_name: &str,
    args_json: &str,
    tenant_id: &str,
) -> Result<ank_proto::v1::ExecuteToolResponse, String> {
    let channel_url = normalize_module_endpoint(endpoint);
    let channel = get_or_connect_module_channel(&channel_url).await?;

    let mut client =
        ank_proto::v1::domain_module_service_client::DomainModuleServiceClient::new(channel);
    let request = tonic::Request::new(ank_proto::v1::ExecuteToolRequest {
        tool_name: tool_name.to_string(),
        arguments_json: args_json.to_string(),
        tenant_id: tenant_id.to_string(),
    });

    match client.execute_tool(request).await {
        Ok(resp) => Ok(resp.into_inner()),
        Err(status)
            if matches!(
                status.code(),
                tonic::Code::Unavailable | tonic::Code::Unknown | tonic::Code::Internal
            ) =>
        {
            warn!(
                endpoint = %channel_url,
                code = ?status.code(),
                "Pooled gRPC channel failed; evicting and retrying once"
            );
            evict_module_channel(&channel_url).await;
            let fresh_channel = get_or_connect_module_channel(&channel_url).await?;
            let mut retry_client =
                ank_proto::v1::domain_module_service_client::DomainModuleServiceClient::new(
                    fresh_channel,
                );
            let retry_req = tonic::Request::new(ank_proto::v1::ExecuteToolRequest {
                tool_name: tool_name.to_string(),
                arguments_json: args_json.to_string(),
                tenant_id: tenant_id.to_string(),
            });
            retry_client
                .execute_tool(retry_req)
                .await
                .map(|r| r.into_inner())
                .map_err(|e| format!("Module tool execution failed: {}", e))
        }
        Err(e) => Err(format!("Module tool execution failed: {}", e)),
    }
}

/// Executes a domain module tool with full kernel context (data_dir, KeyPool, Router, WS progress).
pub async fn execute_module_tool_with_context(
    manifest: &ModuleManifest,
    tool_name: &str,
    args_json: &str,
    tenant_id: &str,
    session_key: &str,
    video_ctx: Option<&crate::video_studio::VideoExecContext>,
) -> Result<String, String> {
    if manifest.module_id == "aegis.domain.video" {
        let default_ctx;
        let ctx_ref = match video_ctx {
            Some(c) => c,
            None => {
                default_ctx = crate::video_studio::VideoExecContext::default();
                &default_ctx
            }
        };
        return crate::video_studio::execute_video_tool(
            tool_name,
            args_json,
            tenant_id,
            session_key,
            ctx_ref,
        )
        .await;
    }

    let is_native = manifest
        .ipc_transport
        .protocol
        .eq_ignore_ascii_case("native")
        || manifest.ipc_transport.endpoint.starts_with("local://");

    if is_native {
        return super::module_runtime::execute_native_module_tool(
            &manifest.module_id,
            tool_name,
            args_json,
            tenant_id,
            session_key,
        );
    }

    match call_module_tool_pooled(
        &manifest.ipc_transport.endpoint,
        tool_name,
        args_json,
        tenant_id,
    )
    .await
    {
        Ok(resp) => {
            if resp.success {
                Ok(resp.result_json)
            } else {
                Err(resp.result_json)
            }
        }
        Err(grpc_err) if super::module_runtime::supports_native_runtime(&manifest.module_id) => {
            warn!(
                module_id = %manifest.module_id,
                endpoint = %manifest.ipc_transport.endpoint,
                error = %grpc_err,
                "External gRPC module endpoint unreachable — falling back to embedded TenantDB runtime"
            );
            super::module_runtime::execute_native_module_tool(
                &manifest.module_id,
                tool_name,
                args_json,
                tenant_id,
                session_key,
            )
        }
        Err(e) => Err(e),
    }
}

/// Executes a domain module tool using either the embedded native SQLCipher runtime
/// (`protocol: "native"` / `local://enclave`) or an external gRPC microservice (`protocol: "gRPC"`),
/// automatically falling back to the embedded runtime if a configured local runtime module's
/// external gRPC daemon is offline.
pub async fn execute_module_tool_for_tenant(
    manifest: &ModuleManifest,
    tool_name: &str,
    args_json: &str,
    tenant_id: &str,
    session_key: &str,
) -> Result<String, String> {
    execute_module_tool_with_context(manifest, tool_name, args_json, tenant_id, session_key, None)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_normalize_module_endpoint() {
        assert_eq!(
            normalize_module_endpoint("localhost:50071"),
            "http://localhost:50071"
        );
        assert_eq!(
            normalize_module_endpoint("http://127.0.0.1:50071"),
            "http://127.0.0.1:50071"
        );
        assert_eq!(
            normalize_module_endpoint("https://biz.aegis.local:50071"),
            "https://biz.aegis.local:50071"
        );
    }

    #[test]
    fn test_manifest_deserialization() {
        let json_data = r#"{
            "module_id": "aegis.domain.business",
            "display_name": "Aegis Business",
            "version": "1.0.0",
            "ipc_transport": {
                "protocol": "gRPC",
                "endpoint": "localhost:50071"
            },
            "database": {
                "driver": "sqlite",
                "encryption": true
            },
            "exposed_tools": [
                {
                    "name": "biz_add_product",
                    "description": "Add product",
                    "parameters": {
                        "type": "object",
                        "properties": {}
                    }
                }
            ]
        }"#;

        let manifest: Result<ModuleManifest, _> = serde_json::from_str(json_data);
        assert!(manifest.is_ok());
        let m = manifest.unwrap();
        assert_eq!(m.module_id, "aegis.domain.business");
        assert_eq!(m.ipc_transport.protocol, "gRPC");
        assert_eq!(m.database.driver, "sqlite");
        assert_eq!(m.exposed_tools.len(), 1);
    }

    #[test]
    fn test_match_prompt_heuristics() {
        let manifest = ModuleManifest {
            module_id: "aegis.domain.business".to_string(),
            display_name: "Aegis Business".to_string(),
            version: "1.0.0".to_string(),
            ipc_transport: IpcTransport {
                protocol: "gRPC".to_string(),
                endpoint: "localhost:50071".to_string(),
            },
            database: ModuleDatabase {
                driver: "sqlite".to_string(),
                encryption: true,
            },
            exposed_tools: vec![ExposedTool {
                name: "biz_add_product".to_string(),
                description: "Add product".to_string(),
                parameters: serde_json::json!({}),
            }],
            ui_views: vec![],
        };

        // Query with business keyword
        assert!(match_prompt_to_module(
            "Agrega 10 quesos al inventario",
            &manifest
        ));

        // Query explicitly calling the tool name
        assert!(match_prompt_to_module("Llama a biz_add_product", &manifest));

        // Unrelated query
        assert!(!match_prompt_to_module("Hola, cómo va todo?", &manifest));
    }

    #[test]
    fn test_modules_scanner_and_loader() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let module_dir = dir.path().join("aegis-biz");
        fs::create_dir_all(&module_dir)?;

        let manifest_path = module_dir.join("module.json");
        let mut file = File::create(&manifest_path)?;

        let json_data = r#"{
            "module_id": "aegis.domain.business",
            "display_name": "Aegis Business",
            "version": "1.0.0",
            "ipc_transport": {
                "protocol": "gRPC",
                "endpoint": "localhost:50071"
            },
            "database": {
                "driver": "sqlite",
                "encryption": true
            },
            "exposed_tools": []
        }"#;

        file.write_all(json_data.as_bytes())?;

        let loaded = load_modules_from_dir(dir.path())?;
        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("aegis.domain.business"));

        Ok(())
    }

    #[tokio::test]
    async fn test_call_module_tool_pooled_invalid_endpoint() {
        let res =
            call_module_tool_pooled("http://invalid uri with spaces", "ping", "{}", "default")
                .await;
        assert!(res.is_err());
        let err = res.err().unwrap_or_default();
        assert!(
            err.starts_with("Invalid endpoint format:"),
            "Unexpected error message: {err}"
        );
    }
}

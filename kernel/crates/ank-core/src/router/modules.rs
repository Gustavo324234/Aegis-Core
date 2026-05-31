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
        let dev_keywords = &["compilar", "compiler", "depurar", "debugger", "aegis-dev"];
        let dev_phrases = &[
            "escribir código",
            "escribir codigo",
            "compilar proyecto",
            "ejecutar test",
            "ejecutar pruebas",
        ];
        if dev_keywords.iter().any(|&kw| lower_prompt.contains(kw))
            || dev_phrases
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
        if match_prompt_to_module(prompt, manifest) {
            let is_enabled = if let Some(ref tenant_db) = db {
                tenant_db
                    .get_kv(&format!("module_active:{}", manifest.module_id))
                    .unwrap_or(None)
                    .map(|v| v == "true")
                    .unwrap_or(false)
            } else {
                false
            };

            if is_enabled {
                relevant_enabled.push(manifest);
            } else {
                relevant_suggested.push(manifest);
            }
        }
    }

    let mut system_prompt = String::new();

    // 1. Inject schemas for already enabled modules
    if !relevant_enabled.is_empty() {
        system_prompt.push_str("\n## HERRAMIENTAS DE MÓDULOS DE DOMINIO (MICROKERNEL) ACTIVAS\n");
        system_prompt.push_str("You can invoke active microkernel tools using the system call [SYS_MCP_EXEC(tool_name, {args})].\n\n");
        for manifest in relevant_enabled {
            system_prompt.push_str(&format!(
                "### {} ({})\n",
                manifest.display_name, manifest.version
            ));
            for tool in &manifest.exposed_tools {
                system_prompt.push_str(&format!("- **{}**: {}\n", tool.name, tool.description));
                system_prompt.push_str(&format!(
                    "  Args Schema: {}\n\n",
                    serde_json::to_string(&tool.parameters).unwrap_or_default()
                ));
            }
        }
    }

    // 2. Inject suggestion instructions for available but inactive modules
    if !relevant_suggested.is_empty() {
        system_prompt.push_str("\n## MÓDULOS DE DOMINIO DISPONIBLES (INSTALACIÓN BAJO DEMANDA)\n");
        system_prompt.push_str("The following microkernel modules are highly relevant to the user's specific request but NOT yet installed or active in their secure enclave database. \
                                You MUST NOT invoke their tools yet. Instead, you MUST ask the user if they want to install/enable the module to gain full control and specialized capabilities for this task. \
                                Be highly specific to the user's request (e.g., if they ask to create a store or business project, ask them if they want to install the Business module to have control over product listings, stock, inventory, and secure billing). \
                                If the user explicitly authorizes it, you MUST output the privileged system call [SYS_ENABLE_MODULE(\"module_id\")] in your response to perform the secure installation:\n\n");
        for manifest in relevant_suggested {
            system_prompt.push_str(&format!(
                "- **{}** (ID: `{}`): Exposes tools to manage this domain.\n",
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

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
}

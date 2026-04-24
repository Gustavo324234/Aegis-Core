use crate::chal::EmbeddingDriver;
use crate::pcb::PCB;
use crate::vcm::swap::LanceSwapManager;
use std::path::{Component, Path};
use thiserror::Error;
use tokio::process::Command;
use tracing::warn;

pub mod swap;

/// --- VCM ERROR SYSTEM ---
#[derive(Error, Debug, Clone)]
pub enum VCMError {
    #[error("Path Traversal Detected: attempt to access {0} outside sandbox")]
    PathTraversalDetected(String),
    #[error("Context Overflow: assembled context exceeds limit of {0} tokens")]
    ContextOverflow(usize),
    #[error("File Not Found: {0}")]
    FileNotFound(String),
    #[error("IO Error: {0}")]
    IOError(String),
    #[error("File too large: {0} exceeds {1} bytes")]
    FileTooLarge(String, u64),
}

const SYSTEM_INSTRUCTIONS: &str = "### SYSTEM: Aegis Neural Kernel VCM ###\nYou are an auxiliary cognitive module of the Aegis Neural Kernel. \
Use the provided context to fulfill the instruction accurately.";

/// Límite de seguridad para evitar cargar archivos masivos en la ventana de atención.
/// Archivos mayores a 2MB se consideran fuera de la capacidad de 'working memory' estándar.
const MAX_FILE_SIZE_BYTES: u64 = 2 * 1024 * 1024;

/// --- VIRTUAL CONTEXT MANAGER ---
/// El VCM es responsable de construir la "ventana de atención" (Context Window)
/// para el LLM, agregando instrucciones L1, referencias L2 y memoria swap L3.
#[derive(Clone, Copy)]
pub struct VirtualContextManager;

impl Default for VirtualContextManager {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualContextManager {
    pub fn new() -> Self {
        Self
    }

    /// Obtiene el estado actual de Git (rama, cambios, último commit).
    async fn fetch_git_state(&self) -> String {
        let branch = Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .await
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let status = Command::new("git")
            .args(["status", "--short"])
            .output()
            .await
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "error fetching status".to_string());

        let last_commit = Command::new("git")
            .args(["log", "-1", "--pretty=%B"])
            .output()
            .await
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "no commits found".to_string());

        format!(
            "[Git Branch]: {}\n[Git Status]:\n{}\n[Last Commit]: {}",
            branch, status, last_commit
        )
    }

    /// Obtiene el estado de los tickets desde TICKETS_MASTER.md.
    async fn fetch_governance_state(&self) -> String {
        let master_path = "governance/TICKETS_MASTER.md";
        match tokio::fs::read_to_string(master_path).await {
            Ok(content) => {
                let in_progress: Vec<&str> = content
                    .lines()
                    .filter(|l| l.contains("🚧 In Progress"))
                    .collect();

                if in_progress.is_empty() {
                    "[Governance]: No active tickets found in progress.".to_string()
                } else {
                    format!("[Governance - Active Tickets]:\n{}", in_progress.join("\n"))
                }
            }
            Err(_) => {
                "[Governance]: Error reading TICKETS_MASTER.md (file not found or unreadable)"
                    .to_string()
            }
        }
    }

    /// Ensambla el contexto final a partir de un PCB y acceso a la memoria L3.
    /// Resuelve las referencias de memoria y aplica límites de tokens.
    /// Estructura: [SYSTEM_INSTRUCTIONS] + \n + [L2_CONTEXT] + \n + [L3_MEMORY] + \n + [L1_INSTRUCTION]
    pub async fn assemble_context(
        &self,
        pcb: &PCB,
        swap_manager: &LanceSwapManager,
        embedding_driver: Option<&dyn EmbeddingDriver>,
        token_limit: usize,
    ) -> Result<String, VCMError> {
        // Enlazar dependencias para la heurística basada en .env si es CloudOnly
        let actual_token_limit = match pcb.model_pref {
            crate::scheduler::ModelPreference::CloudOnly => std::env::var("CLOUD_MAX_TOKENS")
                .unwrap_or_else(|_| "8192".to_string())
                .parse::<usize>()
                .unwrap_or(8192),
            _ => token_limit,
        };

        let l1_prompt = &pcb.memory_pointers.l1_instruction;
        let sys_tokens = estimate_tokens(SYSTEM_INSTRUCTIONS);
        let instr_tokens = estimate_tokens("\n## INSTRUCTION\n") + estimate_tokens(l1_prompt) + 2;

        // DAG Context (inlined_context) PRIORITY 1 (INTOCABLE)
        let mut inlined_str = String::new();
        if !pcb.inlined_context.is_empty() {
            inlined_str.push_str("\n## DAG CONTEXT (DEPENDENCIES)\n");
            for (node, out) in &pcb.inlined_context {
                inlined_str.push_str(&format!("[Node: {}]\n{}\n", node, out));
            }
        }
        let inlined_tokens = estimate_tokens(&inlined_str);

        let mandatory_tokens = sys_tokens + instr_tokens + inlined_tokens;
        if mandatory_tokens > actual_token_limit {
            return Err(VCMError::ContextOverflow(actual_token_limit));
        }
        let mut current_tokens = mandatory_tokens;

        // --- PROJECT CONTEXT INJECTION (CORE-151) ---
        // Project state is best-effort: omitted silently when budget is tight.
        let mut project_state_str = String::new();
        let git_state = self.fetch_git_state().await;
        let gov_state = self.fetch_governance_state().await;
        let mut candidate = String::new();
        candidate.push_str("\n## PROJECT STATE (GIT & GOVERNANCE)\n");
        candidate.push_str(&git_state);
        candidate.push_str("\n\n");
        candidate.push_str(&gov_state);
        candidate.push('\n');
        let project_tokens = estimate_tokens(&candidate);
        if current_tokens + project_tokens <= actual_token_limit {
            project_state_str = candidate;
            current_tokens += project_tokens;
        }

        // L3 SEMANTIC MEMORY - Medium Priority
        let mut l3_added = false;
        let mut l3_str = String::new();
        let tenant_id = pcb.tenant_id.as_deref().unwrap_or("default");

        if !pcb.memory_pointers.swap_refs.is_empty() {
            for _swap_query in &pcb.memory_pointers.swap_refs {
                let query_vector = if let Some(driver) = embedding_driver {
                    driver
                        .embed(&pcb.memory_pointers.l1_instruction)
                        .await
                        .unwrap_or_else(|_| vec![0.0; 128])
                } else {
                    vec![0.0; 128]
                };

                if let Ok(fragments) = swap_manager.search(tenant_id, query_vector, 5).await {
                    for fragment in fragments {
                        let fragment_text =
                            format!("[Memory ID: {}]\n{}\n", fragment.id, fragment.text);
                        let fragment_tokens = estimate_tokens(&fragment_text) + 10;
                        if current_tokens + fragment_tokens <= actual_token_limit {
                            if !l3_added {
                                l3_str.push_str("\n## L3 SEMANTIC MEMORY\n");
                                l3_added = true;
                            }
                            l3_str.push_str(&fragment_text);
                            current_tokens += fragment_tokens;
                        } else {
                            // Se reduce la cantidad de fragmentos devueltos al quedarse sin memoria
                            break;
                        }
                    }
                }
                if current_tokens >= actual_token_limit {
                    break;
                }
            }
        }

        let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
        let tenant_root = format!("{}/users/{}/workspace", base_dir, tenant_id);
        let mut has_l2 = false;
        let mut l2_str = String::new();

        for ref_uri in &pcb.memory_pointers.l2_context_refs {
            if let Some(path_part) = ref_uri.strip_prefix("file://") {
                if current_tokens >= actual_token_limit {
                    if !has_l2 {
                        l2_str.push_str("\n## ATTACHED CONTEXT\n");
                        has_l2 = true;
                    }
                    l2_str.push_str(&format!(
                        "[SYSTEM: {} omitido por falta de memoria]\n",
                        ref_uri
                    ));
                    continue;
                }

                if !is_safe_path(tenant_id, path_part) {
                    return Err(VCMError::PathTraversalDetected(path_part.to_string()));
                }

                let full_path = Path::new(&tenant_root).join(path_part);
                let metadata = match tokio::fs::metadata(&full_path).await {
                    Ok(m) => m,
                    Err(e) => return Err(VCMError::IOError(format!("{}: {}", path_part, e))),
                };

                if metadata.len() > MAX_FILE_SIZE_BYTES {
                    warn!(path = %path_part, size = %metadata.len(), "File too large for VCM, skipping.");
                    if !has_l2 {
                        l2_str.push_str("\n## ATTACHED CONTEXT\n");
                        has_l2 = true;
                    }
                    l2_str.push_str(&format!(
                        "[SYSTEM: {} omitido por tamaño excesivo]\n",
                        ref_uri
                    ));
                    continue;
                }

                let content = match tokio::fs::read_to_string(&full_path).await {
                    Ok(c) => c,
                    Err(e) => return Err(VCMError::IOError(format!("{}: {}", path_part, e))),
                };

                let prefix = format!("[File: {}]\n", path_part);
                let prefix_tokens = estimate_tokens(&prefix);

                let remaining =
                    actual_token_limit.saturating_sub(current_tokens + prefix_tokens + 5);
                if remaining == 0 {
                    if !has_l2 {
                        l2_str.push_str("\n## ATTACHED CONTEXT\n");
                        has_l2 = true;
                    }
                    l2_str.push_str(&format!(
                        "[SYSTEM: {} omitido por falta de memoria]\n",
                        ref_uri
                    ));
                    continue;
                }

                let mut content_to_add = &content[..];
                let content_tokens = estimate_tokens(content_to_add);

                if content_tokens > remaining {
                    // Truncar el archivo pero quedarse con la parte del final (mensajes más recientes).
                    let keep_chars = remaining * 4;
                    let trim_start = content.len().saturating_sub(keep_chars);
                    let mut actual_start = trim_start;
                    while actual_start < content.len() && !content.is_char_boundary(actual_start) {
                        actual_start += 1;
                    }
                    content_to_add = &content[actual_start..];

                    if !has_l2 {
                        l2_str.push_str("\n## ATTACHED CONTEXT\n");
                        has_l2 = true;
                    }
                    l2_str.push_str(&prefix);
                    l2_str.push_str("[...truncado por falta de memoria...]\n");
                    l2_str.push_str(content_to_add);
                    l2_str.push('\n');
                    current_tokens += estimate_tokens(content_to_add) + prefix_tokens + 5;
                } else {
                    if !has_l2 {
                        l2_str.push_str("\n## ATTACHED CONTEXT\n");
                        has_l2 = true;
                    }
                    l2_str.push_str(&prefix);
                    l2_str.push_str(content_to_add);
                    l2_str.push('\n');
                    current_tokens += content_tokens + prefix_tokens;
                }
            }
        }

        // Ensamblado final muy eficiente
        let mut final_context = String::with_capacity(actual_token_limit * 4);
        final_context.push_str(SYSTEM_INSTRUCTIONS);
        final_context.push('\n');

        if !inlined_str.is_empty() {
            final_context.push_str(&inlined_str);
        }
        if !project_state_str.is_empty() {
            final_context.push_str(&project_state_str);
        }
        if has_l2 {
            final_context.push_str(&l2_str);
        }
        if l3_added {
            final_context.push_str(&l3_str);
        }

        final_context.push_str("\n## INSTRUCTION\n");
        final_context.push_str(l1_prompt);
        final_context.push('\n');

        Ok(final_context)
    }
}

/// Heurística simple: 4 caracteres equivalen aproximadamente a 1 token.
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// Auditoría de Seguridad: Previene el acceso a archivos fuera del sandbox de trabajo.
/// Verifica que no existan retrocesos de directorio ("..") que escapen del root permitido.
pub fn is_safe_path(tenant_id: &str, path_str: &str) -> bool {
    // 1. Validar tenant_id para aislar namespace (prevenir Path Traversal via tenant_id)
    if tenant_id.is_empty()
        || !tenant_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return false;
    }

    let path = Path::new(path_str);

    // 2. Prohibir rutas absolutas por seguridad (aislamiento)
    if path.is_absolute() {
        return false;
    }

    // 3. Normalizar componentes y verificar profundidad interactuando solo con el path de entrada
    let mut depth: i32 = 0;
    for component in path.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return false; // Intento de salir del directorio base (Root Escape)
                }
            }
            Component::CurDir => continue,
            _ => return false, // No permitimos RootDir (ya cubierto), Prefix o similar.
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pcb::PCB;
    use anyhow::Context;
    use std::io::Write;

    #[tokio::test]
    async fn test_assemble_basic_context() -> anyhow::Result<()> {
        let vcm = VirtualContextManager::new();
        let swap = LanceSwapManager::new("./test_users"); // Mock
        let pcb = PCB::new("TestProcess".into(), 5, "Summarize this".into());

        // Límite generoso
        let context = vcm.assemble_context(&pcb, &swap, None, 1000).await?;

        assert!(context.contains("SYSTEM: Aegis Neural Kernel VCM"));
        assert!(context.contains("Summarize this"));
        // El orden es SYSTEM -> DAG -> L2 -> L3 -> L1
        Ok(())
    }

    #[tokio::test]
    async fn test_vcm_file_omission_on_overflow() -> anyhow::Result<()> {
        let vcm = VirtualContextManager::new();
        let swap = LanceSwapManager::new("./test_users");

        let tenant_id = format!(
            "test_tenant_vcm_overflow_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_else(|_| std::time::Duration::from_secs(0))
                .as_millis()
        );
        let workspace_path = format!("./users/{}/workspace", tenant_id);

        let mut retries = 5;
        while retries > 0 {
            if std::fs::create_dir_all(&workspace_path).is_ok() {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            retries -= 1;
        }
        std::fs::create_dir_all(&workspace_path).context("Failed to create workspace dir")?;

        // Crear un archivo temporal con ruta relativa dentro del workspace del tenant
        let file_name = "test_overflow_dummy.txt";
        let full_path = std::path::Path::new(&workspace_path).join(file_name);

        let mut file = std::fs::File::create(&full_path).context("Failed to create test file")?;
        let large_content = "X".repeat(2000); // ~500 tokens
        file.write_all(large_content.as_bytes())
            .context("Failed to write test content")?;

        let mut pcb = PCB::new("HeavyProc".into(), 5, "Small task".into());
        pcb.tenant_id = Some(tenant_id.clone());
        pcb.memory_pointers
            .l2_context_refs
            .push(format!("file://{}", file_name));

        // Límite que permite el contexto base (Project Context) pero no el archivo grande
        let context = vcm.assemble_context(&pcb, &swap, None, 250).await?;

        // Limpiar
        let _ = std::fs::remove_dir_all(format!("./users/{}", tenant_id));

        assert!(
            context.contains("omitido por falta de memoria")
                || context.contains("omitido por tamaño excesivo")
                || context.contains("truncado por falta de memoria")
        );
        assert!(!context.contains(&large_content));
        assert!(context.contains("Small task"));
        Ok(())
    }

    #[tokio::test]
    async fn test_vcm_l3_memory_injection() -> anyhow::Result<()> {
        let vcm = VirtualContextManager::new();
        let swap = LanceSwapManager::new("./test_users");
        // In a real test, we would add fragments to LanceDB.
        // For now, search returns an empty list since the DB is empty.

        let mut pcb = PCB::new("SwapProc".into(), 5, "Check memory".into());
        pcb.memory_pointers.swap_refs.push("vec:0.1,0.2".into());

        let context = vcm.assemble_context(&pcb, &swap, None, 1000).await?;

        // No debería fallar, aunque la lista esté vacía.
        assert!(context.contains("Check memory"));
        Ok(())
    }

    #[tokio::test]
    async fn test_vcm_dag_context_priority() -> anyhow::Result<()> {
        let vcm = VirtualContextManager::new();
        let swap = LanceSwapManager::new("./test_users");
        let mut pcb = PCB::new("DAGProc".into(), 5, "Task".into());
        pcb.inlined_context
            .insert("parent_node".into(), "parent_output".into());

        let context = vcm.assemble_context(&pcb, &swap, None, 1000).await?;
        assert!(context.contains("## DAG CONTEXT (DEPENDENCIES)"));
        assert!(context.contains("[Node: parent_node]"));
        assert!(context.contains("parent_output"));
        Ok(())
    }

    #[test]
    fn test_path_traversal_audit() {
        assert!(is_safe_path("tenant_1", "docs/contract.md"));
        assert!(!is_safe_path("tenant_1", "../etc/passwd"));
        assert!(!is_safe_path("tenant_1", "/absolute/path"));
        assert!(!is_safe_path("../tenant_2", "docs/contract.md"));
        assert!(!is_safe_path("tenant/1", "docs/contract.md"));
    }
}

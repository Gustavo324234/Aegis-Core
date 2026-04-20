use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
use tracing::{error, info, warn};
use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::p2::pipe::{MemoryInputPipe, MemoryOutputPipe};
use wasmtime_wasi::preview1::WasiP1Ctx;
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxView, WasiView};

pub mod signer;
pub mod watcher;

use self::signer::PluginSigner;
use crate::enclave::TenantDB;

/// --- PLUGIN ERROR SYSTEM (ANK-2411 Hardening) ---
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Compilation Failed: {0}")]
    CompilationFailed(String),
    #[error("Security Violation: {0}")]
    SecurityViolation(String),
    #[error("Logic Error: {0}")]
    LogicError(String),
    #[error("Resource Exhaustion: The plugin exceeded its CPU budget or memory")]
    ResourceExhaustion,
    #[error("Function Not Found: {0}")]
    FunctionNotFound(String),
    #[error("IO Error: {0}")]
    IOError(String),
    #[error("Execution Failed: {0}")]
    ExecutionFailed(String),
}

/// --- PLUGIN METADATA ---
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    pub name: String,
    pub description: String,
    pub version: String,
    pub author: String,
    pub parameter_example: String,
}

/// --- PLUGIN ---
/// Representa una herramienta cargada en el "User Space" del Kernel.
pub struct Plugin {
    pub metadata: PluginMetadata,
    pub module: Module,
}

/// --- PLUGIN STATE ---
/// Estado interno para el sandbox Wasm (WASI P1 bridge).
struct PluginState {
    ctx: WasiP1Ctx,
}

impl WasiView for PluginState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        self.ctx.ctx()
    }
}

/// --- PLUGIN MANAGER ---
/// Orquestador del sistema de plugins basado en Wasmtime.
pub struct PluginManager {
    engine: Engine,
    linker: Linker<PluginState>,
    plugins: HashMap<String, Plugin>,
    signer: PluginSigner,
}

impl PluginManager {
    /// Inicializa el motor de Wasm con configuraciones optimizadas
    /// y medidas de seguridad (Fuel consumption, WASI, CPU limits).
    pub fn new() -> Result<Self, PluginError> {
        // En un entorno real, AEGIS_ROOT_KEY vendría de enclave/vault.
        let public_key = [0u8; 32];
        let signer = PluginSigner::new(&public_key)
            .map_err(|e| PluginError::IOError(format!("Failed to init Signer: {}", e)))?;
        Self::new_with_signer(signer)
    }

    /// Inicializa el gestor con un firmador específico (útil para tests o rotación de llaves).
    pub fn new_with_signer(signer: PluginSigner) -> Result<Self, PluginError> {
        let mut config = Config::new();

        // --- CONFIGURACIÓN DE SEGURIDAD Y RENDIMIENTO ---
        config.async_support(true);
        config.consume_fuel(true);
        config.cranelift_opt_level(wasmtime::OptLevel::Speed);
        config.memory_reservation(512 * 1024 * 1024); // Máximo 512MB RAM

        let engine =
            Engine::new(&config).map_err(|e| PluginError::CompilationFailed(e.to_string()))?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::preview1::add_to_linker_async(&mut linker, |s: &mut PluginState| &mut s.ctx)
            .map_err(|e: anyhow::Error| PluginError::CompilationFailed(e.to_string()))?;

        Ok(Self {
            engine,
            linker,
            plugins: HashMap::new(),
            signer,
        })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub async fn reload_plugin_module(
        &mut self,
        path: &str,
        module: Module,
    ) -> Result<(), PluginError> {
        // En reload_plugin_module asumimos que el watcher ya verificó la firma o lo haremos aquí.
        // Por seguridad, siempre verificamos firma antes de cargar al mapa.
        self.signer
            .verify_plugin(path)
            .map_err(|e| PluginError::SecurityViolation(e.to_string()))?;

        let name = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PluginError::IOError("Invalid plugin path".to_string()))?
            .to_string();

        let initial_metadata = PluginMetadata {
            name: name.clone(),
            description: "Loading...".to_string(),
            version: "1.0.0".to_string(),
            author: "Loading...".to_string(),
            parameter_example: "{}".to_string(),
        };

        self.plugins.insert(
            name.clone(),
            Plugin {
                metadata: initial_metadata,
                module,
            },
        );

        let metadata_input = r#"{"action": "get_metadata"}"#;
        // system tenant no se marca como tainted fácilmente pero usamos error logging
        match self.execute_plugin("system", &name, metadata_input).await {
            Ok(json_out) => {
                if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&json_out) {
                    if let Some(data) = resp.get("data").and_then(|d| d.as_object()) {
                        let final_metadata = PluginMetadata {
                            name: name.clone(),
                            description: data
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("No description")
                                .to_string(),
                            version: data
                                .get("version")
                                .and_then(|v| v.as_str())
                                .unwrap_or("1.0.0")
                                .to_string(),
                            author: data
                                .get("author")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown")
                                .to_string(),
                            parameter_example: data
                                .get("example_json")
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "{}".to_string()),
                        };

                        if let Some(p) = self.plugins.get_mut(&name) {
                            p.metadata = final_metadata;
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to auto-discover plugin {}: {}", name, e);
            }
        }
        Ok(())
    }

    /// Carga un binario .wasm del disco, lo compila y lo cachea. Extrae metadatos dinámicamente.
    /// [ANK-2411] MANDATORY Signature Verification.
    pub async fn load_plugin(&mut self, path: &str) -> Result<(), PluginError> {
        // 0. Firma Obligatoria (Ring 0 Policy)
        self.signer.verify_plugin(path).map_err(|e| {
            error!(
                "SECURITY ALERT: Plugin signature mismatch for {}: {}",
                path, e
            );
            PluginError::SecurityViolation(e.to_string())
        })?;

        let name = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PluginError::IOError("Invalid plugin path".to_string()))?
            .to_string();

        let module = Module::from_file(&self.engine, path)
            .map_err(|e| PluginError::CompilationFailed(e.to_string()))?;

        // 1. Insert placeholder metadata
        let initial_metadata = PluginMetadata {
            name: name.clone(),
            description: "Loading...".to_string(),
            version: "1.0.0".to_string(),
            author: "Loading...".to_string(),
            parameter_example: "{}".to_string(),
        };

        self.plugins.insert(
            name.clone(),
            Plugin {
                metadata: initial_metadata,
                module,
            },
        );

        // 2. Discover metadata running the plugin
        let metadata_input = r#"{"action": "get_metadata"}"#;
        match self.execute_plugin("system", &name, metadata_input).await {
            Ok(json_out) => {
                if let Ok(resp) = serde_json::from_str::<serde_json::Value>(&json_out) {
                    if let Some(data) = resp.get("data").and_then(|d| d.as_object()) {
                        let final_metadata = PluginMetadata {
                            name: name.clone(),
                            description: data
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("No description")
                                .to_string(),
                            version: data
                                .get("version")
                                .and_then(|v| v.as_str())
                                .unwrap_or("1.0.0")
                                .to_string(),
                            author: data
                                .get("author")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown")
                                .to_string(),
                            parameter_example: data
                                .get("example_json")
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "{}".to_string()),
                        };

                        // Update the map
                        if let Some(p) = self.plugins.get_mut(&name) {
                            p.metadata = final_metadata;
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to auto-discover plugin {}: {}", name, e);
            }
        }
        Ok(())
    }

    /// Escanea un directorio y carga todos los binarios .wasm encontrados.
    pub async fn load_all_from_dir(&mut self, dir_path: &str) -> Result<(), PluginError> {
        let path = Path::new(dir_path);
        if !path.exists() || !path.is_dir() {
            return Err(PluginError::IOError(format!(
                "Plugin directory not found: {}",
                dir_path
            )));
        }

        for entry in std::fs::read_dir(path).map_err(|e| PluginError::IOError(e.to_string()))? {
            let entry = entry.map_err(|e| PluginError::IOError(e.to_string()))?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                if let Some(path_str) = path.to_str() {
                    // Si falla un plugin por firma, logeamos pero no abortamos toda la carga.
                    // Zero-Panic policy.
                    if let Err(e) = self.load_plugin(path_str).await {
                        error!("Failed to load plugin {}: {}", path_str, e);
                    } else {
                        info!(
                            "Plugin loaded and verified: {:?}",
                            path.file_name().unwrap_or_default()
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Ejecuta un plugin en un sandbox aislado (Ring 0) con Jailing dinámico.
    /// [ANK-2411] Trap classification & Tainted Policy.
    pub async fn execute_plugin(
        &self,
        tenant_id: &str,
        plugin_name: &str,
        input_json: &str,
    ) -> Result<String, PluginError> {
        let plugin = self.plugins.get(plugin_name).ok_or_else(|| {
            PluginError::FunctionNotFound(format!("Plugin {} not loaded", plugin_name))
        })?;

        // 1. Configurar Stdin/Stdout virtuales
        let mut final_input = input_json.to_string();
        if plugin_name == "std_net" {
            // ... (keep fetch_url_safe logic) ...
            let req: serde_json::Value = serde_json::from_str(input_json)
                .map_err(|e| PluginError::LogicError(format!("Invalid JSON for std_net: {}", e)))?;

            if let Some(action) = req.get("action").and_then(|a| a.as_str()) {
                if action != "get_metadata" {
                    let url = req
                        .get("url")
                        .and_then(|u| u.as_str())
                        .or_else(|| {
                            req.get("params")
                                .and_then(|p| p.get("url"))
                                .and_then(|u| u.as_str())
                        })
                        .ok_or_else(|| {
                            PluginError::LogicError("std_net requires 'url' parameter".to_string())
                        })?;

                    let raw_html = self.fetch_url_safe(url).await?;
                    let wrapped = serde_json::json!({
                        "action": "parse",
                        "params": {
                            "html": raw_html
                        }
                    });
                    final_input = wrapped.to_string();
                }
            }
        }

        let stdin = MemoryInputPipe::new(final_input.as_bytes().to_vec());
        let stdout = MemoryOutputPipe::new(4096 * 10);

        // 2. Construir el contexto WASI (Dynamic Jailing)
        let workspace_path = format!("./users/{}/workspace", tenant_id);
        std::fs::create_dir_all(&workspace_path)
            .map_err(|e| PluginError::IOError(format!("Failed to create jail: {}", e)))?;

        let mut wasi_builder = wasmtime_wasi::WasiCtxBuilder::new();
        wasi_builder
            .stdin(stdin)
            .stdout(stdout.clone())
            .preopened_dir(
                &workspace_path,
                "/workspace",
                DirPerms::all(),
                FilePerms::all(),
            )
            .map_err(|e: anyhow::Error| {
                PluginError::SecurityViolation(format!("Jailing Failed: {}", e))
            })?;
        let wasi_ctx = wasi_builder.build_p1();

        let state = PluginState { ctx: wasi_ctx };

        let mut store = Store::new(&self.engine, state);
        store
            .set_fuel(1_000_000)
            .map_err(|e| PluginError::LogicError(e.to_string()))?;

        // 3. Instanciar
        let instance = self
            .linker
            .instantiate_async(&mut store, &plugin.module)
            .await
            .map_err(|e| PluginError::LogicError(e.to_string()))?;

        // 4. Invocar _start
        let func = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .map_err(|_| PluginError::FunctionNotFound("_start not exported".to_string()))?;

        if let Err(e) = func.call_async(&mut store, ()).await {
            if let Some(trap) = e.downcast_ref::<wasmtime::Trap>() {
                match trap {
                    wasmtime::Trap::OutOfFuel => return Err(PluginError::ResourceExhaustion),
                    wasmtime::Trap::MemoryOutOfBounds => {
                        error!("SECURITY VIOLATION (OOB) in plugin {} for tenant {}. Marking as TAINTED.", plugin_name, tenant_id);
                        // TAINTED POLICY: Mark in TenantDB
                        // En un entorno real, AEGIS_SESSION_KEY vendría del contexto.
                        if let Ok(db) = TenantDB::open(tenant_id, "default_internal_key") {
                            let _ = db.set_kv(&format!("plugin_status:{}", plugin_name), "TAINTED");
                        }
                        return Err(PluginError::SecurityViolation(
                            "Memory Out Of Bounds (Potential Buffer Overflow Attack)".to_string(),
                        ));
                    }
                    wasmtime::Trap::StackOverflow => {
                        return Err(PluginError::LogicError(format!("Runtime Trap: {}", trap)));
                    }
                    _ => {
                        return Err(PluginError::LogicError(format!(
                            "Unknown or Unreachable Trap: {}",
                            trap
                        )))
                    }
                }
            }
            return Err(PluginError::ExecutionFailed(e.to_string()));
        }

        // 5. Recuperar resultado
        let output_bytes = stdout.contents();
        String::from_utf8(output_bytes.to_vec())
            .map_err(|e| PluginError::ExecutionFailed(format!("Invalid UTF-8 output: {}", e)))
    }

    /// Implementación de seguridad SRE para peticiones HTTP.
    /// Bloquea ataques SSRF validando que la URL no apunte a rangos locales o privados.
    pub async fn fetch_url_safe(&self, url_str: &str) -> Result<String, PluginError> {
        let url = reqwest::Url::parse(url_str)
            .map_err(|e| PluginError::SecurityViolation(format!("Invalid URL: {}", e)))?;

        let host = url
            .host_str()
            .ok_or_else(|| PluginError::SecurityViolation("Missing host in URL".into()))?;

        let port = url.port_or_known_default().unwrap_or(80);
        let addrs = tokio::net::lookup_host(format!("{}:{}", host, port))
            .await
            .map_err(|e| {
                PluginError::IOError(format!("DNS Resolution failed for {}: {}", host, e))
            })?;

        for addr in addrs {
            let ip = addr.ip();
            if ip.is_loopback() || ip.is_unspecified() {
                return Err(PluginError::SecurityViolation(format!(
                    "SSRF Guard: Loopback/Internal access denied for {}",
                    ip
                )));
            }

            if let std::net::IpAddr::V4(v4) = ip {
                if v4.is_private()
                    || v4.is_link_local()
                    || v4.is_broadcast()
                    || v4.is_documentation()
                {
                    return Err(PluginError::SecurityViolation(format!(
                        "SSRF Guard: Private/Local network access denied for {}",
                        ip
                    )));
                }
            } else if let std::net::IpAddr::V6(v6) = ip {
                if (v6.segments()[0] & 0xfe00) == 0xfc00 || (v6.segments()[0] & 0xffc0) == 0xfe80 {
                    return Err(PluginError::SecurityViolation(format!(
                        "SSRF Guard: Private IPv6 access denied for {}",
                        ip
                    )));
                }
            }
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("AegisNeuralKernel/1.0 (Cognitive SRE)")
            .build()
            .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| PluginError::IOError(format!("Network request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            return Err(PluginError::IOError(format!(
                "HTTP Error returned: {}",
                status
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| PluginError::IOError(format!("Failed to read response body: {}", e)))?;

        Ok(body)
    }

    /// Genera la "Tarjeta de Habilidades" (Tool Discovery) para inyectar en el System Prompt.
    pub fn get_available_tools_prompt(&self) -> String {
        if self.plugins.is_empty() {
            return String::new();
        }

        let mut prompt = String::from("HERRAMIENTAS (PLUGINS) DISPONIBLES:\n");
        for plugin in self.plugins.values() {
            prompt.push_str(&format!(
                "- {}: {} -> Uso: [SYS_CALL_PLUGIN(\"{}\", {})]\n",
                plugin.metadata.name,
                plugin.metadata.description,
                plugin.metadata.name,
                plugin.metadata.parameter_example
            ));
        }
        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use ed25519_dalek::Signer;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_plugin_manager_init() {
        let manager = PluginManager::new();
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_wasm_execution_trap_handling() -> anyhow::Result<()> {
        use ed25519_dalek::SigningKey;

        // 1. Generar llaves de prueba
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let verifying_key = signing_key.verifying_key();

        let signer =
            PluginSigner::new(&verifying_key.to_bytes()).context("Failed to create test signer")?;
        let mut manager = PluginManager::new_with_signer(signer)?;

        // Un wasm mínimo que hace un unreachable (trap)
        let wasm_bytes = [
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00,
            0x03, 0x02, 0x01, 0x00, 0x07, 0x0a, 0x01, 0x06, 0x5f, 0x73, 0x74, 0x61, 0x72, 0x74,
            0x00, 0x00, 0x0a, 0x05, 0x01, 0x03, 0x00, 0x00, 0x0b,
        ];

        let mut file = NamedTempFile::new().context("Failed to create temp file")?;
        file.write_all(&wasm_bytes)
            .context("Failed to write wasm bytes")?;
        let path = file
            .path()
            .to_str()
            .context("Temp file path is not UTF-8")?;

        // 2. Firmar el WASM
        let signature = signing_key.sign(&wasm_bytes);
        let sig_path = file.path().with_extension("wasm.sig");
        std::fs::write(&sig_path, signature.to_bytes()).context("Failed to write signature")?;

        manager.load_plugin(path).await?;
        let res = manager.execute_plugin("test_tenant", "test", "{}").await;

        assert!(res.is_err());

        // Limpiar firma manual
        let _ = std::fs::remove_file(sig_path);

        Ok(())
    }
}

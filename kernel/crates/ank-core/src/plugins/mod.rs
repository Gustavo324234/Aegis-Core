use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::Path;
use std::sync::RwLock as StdRwLock;
use std::time::Duration;
use thiserror::Error;
use tracing::{error, info, warn};
use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::p2::pipe::{MemoryInputPipe, MemoryOutputPipe};
use wasmtime_wasi::preview1::WasiP1Ctx;
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxView, WasiView};

pub mod signer;
pub mod watcher;

use self::signer::PluginSigner;
use crate::enclave::TenantDB;

/// Límite máximo de memoria lineal por instancia WASM (64 MiB).
const MAX_WASM_MEMORY_BYTES: usize = 64 * 1024 * 1024;
/// Tiempo máximo de reloj para ejecutar un plugin WASM (15s).
const MAX_WASM_EXECUTION_TIMEOUT: Duration = Duration::from_secs(15);
/// Límite máximo de cuerpo descargado en `fetch_url_safe` (2 MiB).
const MAX_FETCH_BODY_BYTES: usize = 2 * 1024 * 1024;
/// Máximo de redirecciones HTTP validadas contra SSRF.
const MAX_SAFE_REDIRECTS: usize = 3;

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

use serde::{Deserialize, Serialize};

/// --- PLUGIN METADATA ---
#[derive(Debug, Clone, Serialize, Deserialize)]
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
/// Estado interno para el sandbox Wasm (WASI P1 bridge + ResourceLimiter).
struct PluginState {
    ctx: WasiP1Ctx,
    limits: StoreLimits,
}

impl WasiView for PluginState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        self.ctx.ctx()
    }
}

fn validate_tenant_segment(tenant_id: &str) -> Result<(), PluginError> {
    if tenant_id.is_empty()
        || tenant_id == "."
        || tenant_id == ".."
        || tenant_id.contains("..")
        || tenant_id.contains('/')
        || tenant_id.contains('\\')
        || tenant_id.contains(':')
        || tenant_id.contains('\0')
    {
        return Err(PluginError::SecurityViolation(format!(
            "Invalid tenant_id for WASI sandbox jail: '{}'",
            tenant_id
        )));
    }
    Ok(())
}

fn is_forbidden_ipv4(v4: Ipv4Addr) -> bool {
    let octets = v4.octets();
    let is_cgnat = octets[0] == 100 && (octets[1] & 0xc0) == 64; // 100.64.0.0/10
    let is_benchmark = octets[0] == 198 && (octets[1] & 0xfe) == 18; // 198.18.0.0/15
    let is_reserved = octets[0] >= 240; // 240.0.0.0/4

    v4.is_loopback()
        || v4.is_private()
        || v4.is_link_local()
        || v4.is_broadcast()
        || v4.is_documentation()
        || v4.is_unspecified()
        || v4.is_multicast()
        || is_cgnat
        || is_benchmark
        || is_reserved
}

fn is_forbidden_ipv6(v6: Ipv6Addr) -> bool {
    if let Some(mapped_v4) = v6.to_ipv4_mapped() {
        return is_forbidden_ipv4(mapped_v4);
    }
    let segs = v6.segments();
    let is_unique_local = (segs[0] & 0xfe00) == 0xfc00; // fc00::/7
    let is_link_local = (segs[0] & 0xffc0) == 0xfe80; // fe80::/10
    let is_doc = segs[0] == 0x2001 && segs[1] == 0x0db8; // 2001:db8::/32

    v6.is_loopback()
        || v6.is_unspecified()
        || v6.is_multicast()
        || is_unique_local
        || is_link_local
        || is_doc
}

pub(crate) fn is_forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_forbidden_ipv4(v4),
        IpAddr::V6(v6) => is_forbidden_ipv6(v6),
    }
}

/// --- PLUGIN MANAGER ---
/// Orquestador del sistema de plugins basado en Wasmtime.
pub struct PluginManager {
    engine: Engine,
    linker: Linker<PluginState>,
    plugins: HashMap<String, Plugin>,
    signer: PluginSigner,
    domain_plugins: HashMap<String, PluginMetadata>,
    tainted_plugins: StdRwLock<HashSet<String>>,
}

impl PluginManager {
    /// Inicializa el motor de Wasm con configuraciones optimizadas
    /// y medidas de seguridad (Fuel consumption, WASI, CPU limits).
    ///
    /// CORE-FIX: la versión anterior caía silenciosamente a una public key de
    /// 32 ceros cuando `AEGIS_PLUGIN_ROOT_KEY` no estaba seteado o tenía hex
    /// inválido. Eso permitía a cualquier atacante con la private key
    /// correspondiente firmar plugins como "válidos" — sandbox bypass total.
    /// Ahora exigimos una clave explícita en producción y solo permitimos el
    /// modo inseguro si el operador lo activa a propósito con
    /// `AEGIS_ALLOW_INSECURE_PLUGINS=1` (útil para dev/CI).
    pub fn new() -> Result<Self, PluginError> {
        let allow_insecure = std::env::var("AEGIS_ALLOW_INSECURE_PLUGINS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        // Debug builds (cargo build / cargo test) default to permissive with a
        // loud warning so dev/CI does not break. Release builds (production)
        // refuse to start without an explicit key — silent zeroed signer is a
        // sandbox bypass.
        let permissive_default = cfg!(debug_assertions);

        let signer = match std::env::var("AEGIS_PLUGIN_ROOT_KEY") {
            Ok(hex_str) => {
                let bytes = hex::decode(hex_str.trim()).map_err(|e| {
                    PluginError::IOError(format!("AEGIS_PLUGIN_ROOT_KEY is not valid hex: {}", e))
                })?;
                if bytes.len() < 32 {
                    return Err(PluginError::IOError(format!(
                        "AEGIS_PLUGIN_ROOT_KEY must decode to at least 32 bytes (got {})",
                        bytes.len()
                    )));
                }
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes[..32]);
                if allow_insecure {
                    PluginSigner::new_permissive(&key)
                } else {
                    PluginSigner::new(&key)
                }
                .map_err(|e| PluginError::IOError(format!("Failed to init Signer: {}", e)))?
            }
            Err(_) if allow_insecure || permissive_default => {
                tracing::warn!(
                    "AEGIS_PLUGIN_ROOT_KEY not set — using permissive dev/test signer. \
                     Plugin signatures are NOT verified unless .wasm.sig is present. \
                     MUST NOT be used in a release build."
                );
                PluginSigner::new_permissive(&[0u8; 32])
                    .map_err(|e| PluginError::IOError(format!("Failed to init Signer: {}", e)))?
            }
            Err(_) => {
                return Err(PluginError::IOError(
                    "AEGIS_PLUGIN_ROOT_KEY is required in release builds. Set the \
                     env var to a hex-encoded ed25519 public key (≥32 bytes), or \
                     export AEGIS_ALLOW_INSECURE_PLUGINS=1 to opt in to the \
                     unverified-signature mode."
                        .to_string(),
                ));
            }
        };

        Self::new_with_signer(signer)
    }

    /// Inicializa el gestor con un firmador específico (útil para tests o rotación de llaves).
    pub fn new_with_signer(signer: PluginSigner) -> Result<Self, PluginError> {
        let mut config = Config::new();

        // --- CONFIGURACIÓN DE SEGURIDAD Y RENDIMIENTO ---
        config.async_support(true);
        config.consume_fuel(true);
        config.cranelift_opt_level(wasmtime::OptLevel::Speed);
        config.memory_reservation(512 * 1024 * 1024); // Reserva virtual máx 512MB RAM

        let engine =
            Engine::new(&config).map_err(|e| PluginError::CompilationFailed(e.to_string()))?;

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::preview1::add_to_linker_async(&mut linker, |s: &mut PluginState| &mut s.ctx)
            .map_err(|e: anyhow::Error| PluginError::CompilationFailed(e.to_string()))?;

        let mut domain_plugins = HashMap::new();
        domain_plugins.insert(
            "ledger".to_string(),
            PluginMetadata {
                name: "ledger".to_string(),
                description: "Gestión de gastos financieros (Ledger)".to_string(),
                version: "1.0.0".to_string(),
                author: "Aegis Kernel".to_string(),
                parameter_example: r#"{"action": "add_expense", "params": {"amount": 50.5, "description": "Cena", "category": "comida"}}"#.to_string(),
            },
        );
        domain_plugins.insert(
            "chronos".to_string(),
            PluginMetadata {
                name: "chronos".to_string(),
                description: "Gestión de recordatorios y tiempo (Chronos)".to_string(),
                version: "1.0.0".to_string(),
                author: "Aegis Kernel".to_string(),
                parameter_example: r#"{"action": "add_reminder", "params": {"remind_at": "2026-04-24 10:00", "description": "Reunión"}}"#.to_string(),
            },
        );

        Ok(Self {
            engine,
            linker,
            plugins: HashMap::new(),
            signer,
            domain_plugins,
            tainted_plugins: StdRwLock::new(HashSet::new()),
        })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn signer(&self) -> &PluginSigner {
        &self.signer
    }

    fn clear_tainted_for_plugin(&self, plugin_name: &str) {
        if let Ok(mut tainted) = self.tainted_plugins.write() {
            let suffix = format!(":{}", plugin_name);
            tainted.retain(|entry| !entry.ends_with(&suffix));
        }
    }

    fn is_plugin_tainted(
        &self,
        tenant_id: &str,
        plugin_name: &str,
        session_key: Option<&str>,
    ) -> bool {
        let key = format!("{}:{}", tenant_id, plugin_name);
        if let Ok(tainted) = self.tainted_plugins.read() {
            if tainted.contains(&key) {
                return true;
            }
        }
        if let Some(skey) = session_key {
            if let Ok(db) = TenantDB::open(tenant_id, skey) {
                if let Ok(Some(status)) = db.get_kv(&format!("plugin_status:{}", plugin_name)) {
                    if status == "TAINTED" {
                        if let Ok(mut tainted) = self.tainted_plugins.write() {
                            tainted.insert(key);
                        }
                        return true;
                    }
                }
            }
        }
        false
    }

    fn mark_plugin_tainted(&self, tenant_id: &str, plugin_name: &str, session_key: Option<&str>) {
        if let Ok(mut tainted) = self.tainted_plugins.write() {
            tainted.insert(format!("{}:{}", tenant_id, plugin_name));
        }
        if let Some(key) = session_key {
            if let Ok(db) = TenantDB::open(tenant_id, key) {
                let _ = db.set_kv(&format!("plugin_status:{}", plugin_name), "TAINTED");
            }
        }
    }

    pub async fn reload_plugin_module(
        &mut self,
        path: &str,
        module: Module,
    ) -> Result<(), PluginError> {
        // Por seguridad, siempre verificamos firma antes de cargar al mapa.
        self.signer
            .verify_plugin(path)
            .map_err(|e| PluginError::SecurityViolation(e.to_string()))?;

        let name = Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PluginError::IOError("Invalid plugin path".to_string()))?
            .to_string();

        // Un nuevo binario firmado rehabilita marcas TAINTED previas en memoria.
        self.clear_tainted_for_plugin(&name);

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
        match self
            .execute_plugin("system", &name, metadata_input, None)
            .await
        {
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

        let module = Module::from_file(&self.engine, path)
            .map_err(|e| PluginError::CompilationFailed(e.to_string()))?;

        self.reload_plugin_module(path, module).await
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
        session_key: Option<&str>,
    ) -> Result<String, PluginError> {
        validate_tenant_segment(tenant_id)?;

        // 0. Interceptar Domain Plugins (Native)
        if let Some(metadata) = self.domain_plugins.get(plugin_name) {
            return self
                .execute_domain_plugin(tenant_id, session_key, plugin_name, input_json, metadata)
                .await;
        }

        // CORE-363: Enforce TAINTED policy before instantiating WASM module.
        if self.is_plugin_tainted(tenant_id, plugin_name, session_key) {
            return Err(PluginError::SecurityViolation(format!(
                "Plugin '{}' is quarantined (TAINTED) for tenant '{}' due to a prior memory safety violation",
                plugin_name, tenant_id
            )));
        }

        let plugin = self.plugins.get(plugin_name).ok_or_else(|| {
            PluginError::FunctionNotFound(format!("Plugin {} not loaded", plugin_name))
        })?;

        // 1. Configurar Stdin/Stdout virtuales
        let mut final_input = input_json.to_string();
        if plugin_name == "std_net" {
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
        let stdout = MemoryOutputPipe::new(256 * 1024);
        let stderr = MemoryOutputPipe::new(64 * 1024);

        // 2. Construir el contexto WASI (Dynamic Jailing)
        let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
        let workspace_path = Path::new(&base_dir)
            .join("users")
            .join(tenant_id)
            .join("workspace");
        std::fs::create_dir_all(&workspace_path)
            .map_err(|e| PluginError::IOError(format!("Failed to create jail: {}", e)))?;

        let mut wasi_builder = wasmtime_wasi::WasiCtxBuilder::new();
        wasi_builder
            .stdin(stdin)
            .stdout(stdout.clone())
            .stderr(stderr)
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

        let limits = StoreLimitsBuilder::new()
            .memory_size(MAX_WASM_MEMORY_BYTES)
            .table_elements(16_384)
            .instances(8)
            .tables(8)
            .memories(4)
            .build();

        let state = PluginState {
            ctx: wasi_ctx,
            limits,
        };

        let mut store = Store::new(&self.engine, state);
        store.limiter(|s| &mut s.limits);
        store
            .set_fuel(2_000_000)
            .map_err(|e| PluginError::LogicError(e.to_string()))?;

        // 3. Instanciar
        let instance = self
            .linker
            .instantiate_async(&mut store, &plugin.module)
            .await
            .map_err(|e| PluginError::LogicError(e.to_string()))?;

        // 4. Invocar _start con límite duro de reloj (CORE-363)
        let func = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .map_err(|_| PluginError::FunctionNotFound("_start not exported".to_string()))?;

        match tokio::time::timeout(MAX_WASM_EXECUTION_TIMEOUT, func.call_async(&mut store, ()))
            .await
        {
            Err(_) => {
                warn!(
                    "Plugin '{}' exceeded wall-clock timeout ({:?}) for tenant '{}'",
                    plugin_name, MAX_WASM_EXECUTION_TIMEOUT, tenant_id
                );
                return Err(PluginError::ResourceExhaustion);
            }
            Ok(Err(e)) => {
                if let Some(trap) = e.downcast_ref::<wasmtime::Trap>() {
                    match trap {
                        wasmtime::Trap::OutOfFuel => return Err(PluginError::ResourceExhaustion),
                        wasmtime::Trap::MemoryOutOfBounds => {
                            error!(
                                "SECURITY VIOLATION (OOB) in plugin {} for tenant {}. Marking as TAINTED.",
                                plugin_name, tenant_id
                            );
                            self.mark_plugin_tainted(tenant_id, plugin_name, session_key);
                            return Err(PluginError::SecurityViolation(
                                "Memory Out Of Bounds (Potential Buffer Overflow Attack)"
                                    .to_string(),
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
            Ok(Ok(())) => {}
        }

        // 5. Recuperar resultado
        let output_bytes = stdout.contents();
        String::from_utf8(output_bytes.to_vec())
            .map_err(|e| PluginError::ExecutionFailed(format!("Invalid UTF-8 output: {}", e)))
    }

    /// Implementación de seguridad SRE para peticiones HTTP.
    /// Bloquea ataques SSRF (incluyendo IPv6 mapeado, CGNAT, DNS rebinding y redirecciones 30x)
    /// y acota el tamaño máximo descargado en memoria.
    pub async fn fetch_url_safe(&self, url_str: &str) -> Result<String, PluginError> {
        use futures_util::StreamExt;

        let mut current_url = reqwest::Url::parse(url_str)
            .map_err(|e| PluginError::SecurityViolation(format!("Invalid URL: {}", e)))?;

        for hop in 0..=MAX_SAFE_REDIRECTS {
            let scheme = current_url.scheme();
            if scheme != "http" && scheme != "https" {
                return Err(PluginError::SecurityViolation(format!(
                    "SSRF Guard: Unsupported URL scheme '{}'",
                    scheme
                )));
            }

            let host = current_url
                .host_str()
                .ok_or_else(|| PluginError::SecurityViolation("Missing host in URL".into()))?
                .to_string();

            let port = current_url
                .port_or_known_default()
                .unwrap_or(if scheme == "https" { 443 } else { 80 });

            let resolved_addrs: Vec<SocketAddr> =
                tokio::net::lookup_host(format!("{}:{}", host, port))
                    .await
                    .map_err(|e| {
                        PluginError::IOError(format!("DNS Resolution failed for {}: {}", host, e))
                    })?
                    .collect();

            if resolved_addrs.is_empty() {
                return Err(PluginError::IOError(format!(
                    "DNS Resolution returned no addresses for {}",
                    host
                )));
            }

            for addr in &resolved_addrs {
                if is_forbidden_ip(addr.ip()) {
                    return Err(PluginError::SecurityViolation(format!(
                        "SSRF Guard: Private/Internal network access denied for {}",
                        addr.ip()
                    )));
                }
            }

            let mut builder = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(15))
                .user_agent("AegisNeuralKernel/1.0 (Cognitive SRE)");

            if let Some(domain) = current_url.domain() {
                builder = builder.resolve_to_addrs(domain, &resolved_addrs);
            }

            let client = builder
                .build()
                .map_err(|e| PluginError::ExecutionFailed(e.to_string()))?;

            let response = client
                .get(current_url.clone())
                .send()
                .await
                .map_err(|e| PluginError::IOError(format!("Network request failed: {}", e)))?;

            let status = response.status();
            if status.is_redirection() {
                if hop == MAX_SAFE_REDIRECTS {
                    return Err(PluginError::SecurityViolation(
                        "SSRF Guard: Maximum redirect hops exceeded".into(),
                    ));
                }
                let loc = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|h| h.to_str().ok())
                    .ok_or_else(|| {
                        PluginError::IOError("Redirect response missing Location header".into())
                    })?;
                current_url = current_url.join(loc).map_err(|e| {
                    PluginError::SecurityViolation(format!("Invalid redirect URL: {}", e))
                })?;
                continue;
            }

            if !status.is_success() {
                return Err(PluginError::IOError(format!(
                    "HTTP Error returned: {}",
                    status
                )));
            }

            if let Some(len) = response.content_length() {
                if len as usize > MAX_FETCH_BODY_BYTES {
                    return Err(PluginError::ResourceExhaustion);
                }
            }

            let mut body_bytes = Vec::new();
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| {
                    PluginError::IOError(format!("Failed to read response body: {}", e))
                })?;
                if body_bytes.len().saturating_add(chunk.len()) > MAX_FETCH_BODY_BYTES {
                    return Err(PluginError::ResourceExhaustion);
                }
                body_bytes.extend_from_slice(&chunk);
            }

            return Ok(String::from_utf8_lossy(&body_bytes).into_owned());
        }

        Err(PluginError::SecurityViolation(
            "SSRF Guard: Redirect loop aborted".into(),
        ))
    }

    /// Genera la "Tarjeta de Habilidades" (Tool Discovery) para inyectar en el System Prompt.
    pub fn get_available_tools_prompt(&self) -> String {
        if self.plugins.is_empty() && self.domain_plugins.is_empty() {
            return String::new();
        }

        let mut prompt = String::from("HERRAMIENTAS (PLUGINS) DISPONIBLES:\n");

        // Plugins Nativos (Dominios)
        for plugin in self.domain_plugins.values() {
            prompt.push_str(&format!(
                "- {}: {} -> Uso: [SYS_CALL_PLUGIN(\"{}\", {})]\n",
                plugin.name, plugin.description, plugin.name, plugin.parameter_example
            ));
        }

        // Plugins WASM
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

    /// Ejecutor de Plugins de Dominio (Native Bridge to SQLCipher)
    async fn execute_domain_plugin(
        &self,
        tenant_id: &str,
        session_key: Option<&str>,
        name: &str,
        input: &str,
        metadata: &PluginMetadata,
    ) -> Result<String, PluginError> {
        let key = session_key.ok_or_else(|| {
            PluginError::SecurityViolation("Session key required for domain plugins".into())
        })?;

        let db =
            TenantDB::open(tenant_id, key).map_err(|e| PluginError::IOError(format!("{:#}", e)))?;

        let req: serde_json::Value =
            serde_json::from_str(input).map_err(|e| PluginError::LogicError(e.to_string()))?;
        let action = req.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let params = req.get("params").unwrap_or(&serde_json::Value::Null);

        match (name, action) {
            ("ledger", "add_expense") => {
                let amount = params["amount"]
                    .as_f64()
                    .ok_or_else(|| PluginError::LogicError("Missing amount".into()))?;
                let desc = params["description"]
                    .as_str()
                    .ok_or_else(|| PluginError::LogicError("Missing description".into()))?;
                let category = params["category"].as_str();
                db.add_expense(amount, desc, category)
                    .map_err(|e| PluginError::IOError(e.to_string()))?;
                Ok(r#"{"status": "success", "message": "Gasto registrado correctamente."}"#.into())
            }
            ("ledger", "get_expenses") => {
                let limit = params["limit"].as_u64().unwrap_or(10) as u32;
                let expenses = db
                    .get_expenses(limit)
                    .map_err(|e| PluginError::IOError(e.to_string()))?;
                Ok(serde_json::json!({ "status": "success", "data": expenses }).to_string())
            }
            ("chronos", "add_reminder") => {
                let remind_at = params["remind_at"]
                    .as_str()
                    .ok_or_else(|| PluginError::LogicError("Missing remind_at".into()))?;
                let desc = params["description"]
                    .as_str()
                    .ok_or_else(|| PluginError::LogicError("Missing description".into()))?;
                db.add_reminder(remind_at, desc)
                    .map_err(|e| PluginError::IOError(e.to_string()))?;
                Ok(
                    r#"{"status": "success", "message": "Recordatorio guardado correctamente."}"#
                        .into(),
                )
            }
            ("chronos", "get_reminders") => {
                let limit = params["limit"].as_u64().unwrap_or(10) as u32;
                let reminders = db
                    .get_reminders(limit)
                    .map_err(|e| PluginError::IOError(e.to_string()))?;
                Ok(serde_json::json!({ "status": "success", "data": reminders }).to_string())
            }
            (_, "get_metadata") => {
                Ok(serde_json::json!({ "status": "success", "data": metadata }).to_string())
            }
            _ => Err(PluginError::FunctionNotFound(format!(
                "Action {} not found in plugin {}",
                action, name
            ))),
        }
    }

    /// Verifica si un plugin está cargado y activo.
    pub fn is_plugin_active(&self, name: &str) -> bool {
        name == "maker" || self.domain_plugins.contains_key(name) || self.plugins.contains_key(name)
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
        let _guard = crate::enclave::acquire_test_lock();
        // PluginManager::new() defaults to permissive in debug builds
        // (cfg(debug_assertions)), so cargo test works without any env var.
        let manager = PluginManager::new();
        assert!(manager.is_ok());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_wasm_execution_trap_handling() -> anyhow::Result<()> {
        let _guard = crate::enclave::acquire_test_lock();
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
        let plugin_name = file
            .path()
            .file_stem()
            .and_then(|s| s.to_str())
            .context("Missing file stem")?
            .to_string();

        // 2. Firmar el WASM (hex format test as well as trap test)
        let signature = signing_key.sign(&wasm_bytes);
        let sig_path = file.path().with_extension("wasm.sig");
        std::fs::write(&sig_path, hex::encode(signature.to_bytes()))
            .context("Failed to write hex signature")?;

        manager.load_plugin(path).await?;
        let res = manager
            .execute_plugin("test_tenant", &plugin_name, "{}", None)
            .await;

        assert!(res.is_err());

        // Limpiar firma manual y directorio de prueba
        let _ = std::fs::remove_file(sig_path);
        let _ = std::fs::remove_dir_all("./users/test_tenant");

        Ok(())
    }

    #[test]
    fn test_ssrf_guard_blocks_ipv4_mapped_ipv6_and_cgnat() -> anyhow::Result<()> {
        assert!(is_forbidden_ip("127.0.0.1".parse()?));
        assert!(is_forbidden_ip("10.0.0.5".parse()?));
        assert!(is_forbidden_ip("169.254.169.254".parse()?));
        assert!(is_forbidden_ip("100.64.0.1".parse()?));
        assert!(is_forbidden_ip("::1".parse()?));
        assert!(is_forbidden_ip("::ffff:127.0.0.1".parse()?));
        assert!(is_forbidden_ip("::ffff:10.1.2.3".parse()?));
        assert!(!is_forbidden_ip("93.184.216.34".parse()?));
        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_tainted_plugin_blocked_on_subsequent_calls() -> anyhow::Result<()> {
        let _guard = crate::enclave::acquire_test_lock();
        let manager = PluginManager::new()?;
        manager.mark_plugin_tainted("tenant_alpha", "rogue_wasm", None);

        let err = manager
            .execute_plugin("tenant_alpha", "rogue_wasm", "{}", None)
            .await;
        assert!(matches!(err, Err(PluginError::SecurityViolation(_))));

        // Tenant isolation: another tenant is not tainted, so it gets FunctionNotFound instead
        let other = manager
            .execute_plugin("tenant_beta", "rogue_wasm", "{}", None)
            .await;
        assert!(matches!(other, Err(PluginError::FunctionNotFound(_))));
        Ok(())
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_domain_plugin_execution() -> anyhow::Result<()> {
        let _guard = crate::enclave::acquire_test_lock();
        let manager = PluginManager::new()?;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let tenant_id = format!("test_domain_user_{}", ts);
        let tenant_id = tenant_id.as_str();
        let session_key = "test_key_123";

        // Test Ledger
        let ledger_input = r#"{"action": "add_expense", "params": {"amount": 100.0, "description": "Test expense", "category": "test"}}"#;
        let res = manager
            .execute_plugin(tenant_id, "ledger", ledger_input, Some(session_key))
            .await?;
        assert!(res.contains("success"));

        let get_input = r#"{"action": "get_expenses", "params": {"limit": 1}}"#;
        let res = manager
            .execute_plugin(tenant_id, "ledger", get_input, Some(session_key))
            .await?;
        assert!(res.contains("Test expense"));

        // Test Chronos
        let chronos_input = r#"{"action": "add_reminder", "params": {"remind_at": "2026-05-01 12:00", "description": "Test reminder"}}"#;
        let res = manager
            .execute_plugin(tenant_id, "chronos", chronos_input, Some(session_key))
            .await?;
        assert!(res.contains("success"));

        let get_input = r#"{"action": "get_reminders", "params": {"limit": 1}}"#;
        let res = manager
            .execute_plugin(tenant_id, "chronos", get_input, Some(session_key))
            .await?;
        assert!(res.contains("Test reminder"));

        // Limpiar
        let _ = std::fs::remove_dir_all(format!("./users/{}", tenant_id));

        Ok(())
    }
}

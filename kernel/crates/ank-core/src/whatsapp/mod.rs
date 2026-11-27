use crate::enclave::TenantDB;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tracing::{info, warn};

const KV_REQUIRE_CONFIRMATION: &str = "wa_require_confirmation";
const KV_RATE_CONFIG: &str = "wa_rate_config";
const KV_PENDING_MESSAGES: &str = "wa_pending_messages";
const KV_SEND_LOG: &str = "wa_send_log";
const KV_CONTACT_ALIASES: &str = "wa_contact_aliases";

const PENDING_TTL_SECS: u64 = 1800; // 30 minutes
const MAX_PENDING_ITEMS: usize = 25;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppRateConfig {
    pub min_interval_secs: u64,
    pub max_per_minute: usize,
    pub max_per_hour: usize,
    pub max_per_day: usize,
    pub max_per_recipient_10m: usize,
    pub dedup_window_secs: u64,
}

impl Default for WhatsAppRateConfig {
    fn default() -> Self {
        Self {
            min_interval_secs: 5,
            max_per_minute: 6,
            max_per_hour: 30,
            max_per_day: 120,
            max_per_recipient_10m: 5,
            dedup_window_secs: 60,
        }
    }
}

impl WhatsAppRateConfig {
    /// Enforces hard anti-spam bounds so WhatsApp accounts cannot be flooded or banned.
    pub fn sanitize(mut self) -> Self {
        self.min_interval_secs = self.min_interval_secs.clamp(2, 60);
        self.max_per_minute = self.max_per_minute.clamp(1, 20);
        self.max_per_hour = self.max_per_hour.clamp(self.max_per_minute, 120);
        self.max_per_day = self.max_per_day.clamp(self.max_per_hour, 400);
        self.max_per_recipient_10m = self.max_per_recipient_10m.clamp(1, 15);
        self.dedup_window_secs = self.dedup_window_secs.clamp(10, 600);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingWhatsAppMessage {
    pub id: String,
    pub to_input: String,
    pub to_jid: String,
    pub recipient_label: String,
    pub text: String,
    pub created_at_epoch: u64,
    pub expires_at_epoch: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppContactAlias {
    pub alias: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub updated_at_epoch: u64,
}

pub fn normalize_alias_key(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_space = true;
    for ch in raw.trim().to_lowercase().chars() {
        let folded = match ch {
            'á' | 'à' | 'ä' | 'â' | 'ã' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            _ => ch,
        };
        if folded.is_ascii_alphanumeric() || folded == '+' {
            out.push(folded);
            prev_space = false;
        } else if (folded.is_whitespace() || folded == '_' || folded == '-') && !prev_space {
            out.push(' ');
            prev_space = true;
        }
    }
    out.trim().to_string()
}

pub fn list_contact_aliases(db: &TenantDB) -> Vec<WhatsAppContactAlias> {
    db.get_kv(KV_CONTACT_ALIASES)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<Vec<WhatsAppContactAlias>>(&raw).ok())
        .unwrap_or_default()
}

pub fn save_contact_alias(
    db: &TenantDB,
    alias: &str,
    target: &str,
    note: Option<&str>,
) -> Result<WhatsAppContactAlias> {
    let clean_alias = alias.trim();
    let raw_target = target.trim();
    if clean_alias.is_empty() {
        return Err(anyhow!(
            "Indicá un nombre o apodo (alias) para guardar el contacto."
        ));
    }
    if raw_target.is_empty() {
        return Err(anyhow!("Indicá el número de teléfono o JID del contacto."));
    }

    let normalized_target = if raw_target.contains('@') {
        raw_target.to_string()
    } else {
        let digits: String = raw_target.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 7 {
            format!("+{}", digits)
        } else {
            return Err(anyhow!(
                "El destino '{}' no parece un número de teléfono válido con código de país.",
                raw_target
            ));
        }
    };

    let clean_note = note
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let key = normalize_alias_key(clean_alias);
    let mut list = list_contact_aliases(db);
    let entry = WhatsAppContactAlias {
        alias: clean_alias.to_string(),
        target: normalized_target,
        note: clean_note,
        updated_at_epoch: now_epoch(),
    };

    if let Some(pos) = list
        .iter()
        .position(|c| normalize_alias_key(&c.alias) == key)
    {
        list[pos] = entry.clone();
    } else {
        list.push(entry.clone());
    }
    if list.len() > 200 {
        list.sort_by_key(|a| std::cmp::Reverse(a.updated_at_epoch));
        list.truncate(200);
    }

    db.set_kv(KV_CONTACT_ALIASES, &serde_json::to_string(&list)?)?;
    Ok(entry)
}

pub fn resolve_contact_alias(db: &TenantDB, query: &str) -> Option<WhatsAppContactAlias> {
    let q = normalize_alias_key(query);
    if q.is_empty() {
        return None;
    }
    let list = list_contact_aliases(db);
    if let Some(exact) = list.iter().find(|c| normalize_alias_key(&c.alias) == q) {
        return Some(exact.clone());
    }
    if let Some(note_exact) = list.iter().find(|c| {
        c.note
            .as_deref()
            .map(normalize_alias_key)
            .map(|n| !n.is_empty() && n == q)
            .unwrap_or(false)
    }) {
        return Some(note_exact.clone());
    }
    if q.len() >= 3 {
        let partial: Vec<&WhatsAppContactAlias> = list
            .iter()
            .filter(|c| {
                let a = normalize_alias_key(&c.alias);
                let n = c
                    .note
                    .as_deref()
                    .map(normalize_alias_key)
                    .unwrap_or_default();
                a.contains(&q) || q.contains(&a) || (!n.is_empty() && n.contains(&q))
            })
            .collect();
        if partial.len() == 1 {
            return Some(partial[0].clone());
        }
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SendLogEntry {
    ts: u64,
    to_jid: String,
    text_hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RateUsageStats {
    pub sent_last_minute: usize,
    pub sent_last_hour: usize,
    pub sent_last_day: usize,
    pub config: WhatsAppRateConfig,
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hash_message(to_jid: &str, text: &str) -> String {
    let mut h = Sha256::new();
    h.update(to_jid.as_bytes());
    h.update(b"::");
    h.update(text.trim().to_lowercase().as_bytes());
    hex::encode(h.finalize())
}

pub fn get_require_confirmation(db: &TenantDB) -> bool {
    match db.get_kv(KV_REQUIRE_CONFIRMATION) {
        Ok(Some(v)) => v.trim().to_lowercase() != "false",
        _ => true,
    }
}

pub fn set_require_confirmation(db: &TenantDB, enabled: bool) -> Result<()> {
    db.set_kv(
        KV_REQUIRE_CONFIRMATION,
        if enabled { "true" } else { "false" },
    )
}

pub fn get_rate_config(db: &TenantDB) -> WhatsAppRateConfig {
    db.get_kv(KV_RATE_CONFIG)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<WhatsAppRateConfig>(&raw).ok())
        .map(WhatsAppRateConfig::sanitize)
        .unwrap_or_default()
}

pub fn set_rate_config(db: &TenantDB, cfg: WhatsAppRateConfig) -> Result<WhatsAppRateConfig> {
    let sanitized = cfg.sanitize();
    let raw = serde_json::to_string(&sanitized)?;
    db.set_kv(KV_RATE_CONFIG, &raw)?;
    Ok(sanitized)
}

pub fn list_pending_messages(db: &TenantDB) -> Vec<PendingWhatsAppMessage> {
    let now = now_epoch();
    let list: Vec<PendingWhatsAppMessage> = db
        .get_kv(KV_PENDING_MESSAGES)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();

    let active: Vec<PendingWhatsAppMessage> = list
        .into_iter()
        .filter(|item| item.expires_at_epoch > now)
        .collect();

    active
}

fn save_pending_messages(db: &TenantDB, list: &[PendingWhatsAppMessage]) -> Result<()> {
    let json = serde_json::to_string(list)?;
    db.set_kv(KV_PENDING_MESSAGES, &json)?;
    Ok(())
}

pub fn queue_pending_message(
    db: &TenantDB,
    to_input: &str,
    to_jid: &str,
    recipient_label: &str,
    text: &str,
) -> Result<PendingWhatsAppMessage> {
    let now = now_epoch();
    let mut list = list_pending_messages(db);
    let short = &uuid::Uuid::new_v4().simple().to_string()[..6];
    let item = PendingWhatsAppMessage {
        id: format!("wa_{}", short),
        to_input: to_input.trim().to_string(),
        to_jid: to_jid.trim().to_string(),
        recipient_label: recipient_label.trim().to_string(),
        text: text.trim().to_string(),
        created_at_epoch: now,
        expires_at_epoch: now.saturating_add(PENDING_TTL_SECS),
    };
    list.push(item.clone());
    if list.len() > MAX_PENDING_ITEMS {
        let excess = list.len() - MAX_PENDING_ITEMS;
        list.drain(0..excess);
    }
    save_pending_messages(db, &list)?;
    Ok(item)
}

pub fn take_pending_message(
    db: &TenantDB,
    pending_id: Option<&str>,
) -> Result<PendingWhatsAppMessage> {
    let mut list = list_pending_messages(db);
    if list.is_empty() {
        return Err(anyhow!(
            "No hay ningún mensaje pendiente de confirmación para enviar."
        ));
    }

    let idx = match pending_id.map(str::trim).filter(|s| !s.is_empty()) {
        Some(id) => list
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| anyhow!("No encontré un mensaje pendiente con id '{}'.", id))?,
        None => {
            if list.len() == 1 {
                0
            } else {
                // Default to most recent if only clear target, or ask to specify when multiple
                return Err(anyhow!(
                    "Hay {} mensajes pendientes de confirmación. Indicá cuál querés confirmar (id: {}).",
                    list.len(),
                    list.iter().map(|i| i.id.as_str()).collect::<Vec<_>>().join(", ")
                ));
            }
        }
    };

    let item = list.remove(idx);
    save_pending_messages(db, &list)?;
    Ok(item)
}

pub fn cancel_pending_messages(db: &TenantDB, target: Option<&str>) -> Result<usize> {
    let mut list = list_pending_messages(db);
    if list.is_empty() {
        return Ok(0);
    }

    let t = target.map(str::trim).unwrap_or("");
    if t.eq_ignore_ascii_case("all")
        || t.eq_ignore_ascii_case("todos")
        || t == "*"
        || (t.is_empty() && list.len() == 1)
    {
        let count = list.len();
        save_pending_messages(db, &[])?;
        return Ok(count);
    }

    if t.is_empty() {
        // Cancel the newest draft when user says "cancelalo" without id
        list.pop();
        save_pending_messages(db, &list)?;
        return Ok(1);
    }

    let before = list.len();
    list.retain(|item| item.id != t);
    let removed = before.saturating_sub(list.len());
    if removed > 0 {
        save_pending_messages(db, &list)?;
    }
    Ok(removed)
}

fn load_send_log(db: &TenantDB, now: u64) -> Vec<SendLogEntry> {
    let cutoff = now.saturating_sub(86_400);
    db.get_kv(KV_SEND_LOG)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<Vec<SendLogEntry>>(&raw).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.ts >= cutoff)
        .collect()
}

pub fn get_rate_usage(db: &TenantDB) -> RateUsageStats {
    let now = now_epoch();
    let log = load_send_log(db, now);
    let config = get_rate_config(db);
    RateUsageStats {
        sent_last_minute: log.iter().filter(|e| now.saturating_sub(e.ts) < 60).count(),
        sent_last_hour: log
            .iter()
            .filter(|e| now.saturating_sub(e.ts) < 3600)
            .count(),
        sent_last_day: log.len(),
        config,
    }
}

pub fn check_rate_limit(db: &TenantDB, to_jid: &str, text: &str) -> Result<(), String> {
    check_rate_limit_at(db, to_jid, text, now_epoch())
}

fn check_rate_limit_at(db: &TenantDB, to_jid: &str, text: &str, now: u64) -> Result<(), String> {
    let cfg = get_rate_config(db);
    let log = load_send_log(db, now);
    let msg_hash = hash_message(to_jid, text);

    if let Some(last) = log.iter().max_by_key(|e| e.ts) {
        let elapsed = now.saturating_sub(last.ts);
        if elapsed < cfg.min_interval_secs {
            let wait = cfg.min_interval_secs - elapsed;
            return Err(format!(
                "Protección anti-spam activa: esperá {}s antes de enviar otro mensaje.",
                wait.max(1)
            ));
        }
    }

    if log.iter().any(|e| {
        e.to_jid == to_jid
            && e.text_hash == msg_hash
            && now.saturating_sub(e.ts) < cfg.dedup_window_secs
    }) {
        return Err(format!(
            "Protección anti-spam: ya enviaste ese mismo mensaje a {} hace menos de {}s.",
            to_jid, cfg.dedup_window_secs
        ));
    }

    let in_min: Vec<&SendLogEntry> = log
        .iter()
        .filter(|e| now.saturating_sub(e.ts) < 60)
        .collect();
    if in_min.len() >= cfg.max_per_minute {
        let oldest_ts = in_min.iter().map(|e| e.ts).min().unwrap_or(now);
        let wait = 60u64.saturating_sub(now.saturating_sub(oldest_ts)).max(1);
        return Err(format!(
            "Límite anti-spam alcanzado ({}/{} mensajes por minuto). Reintentá en ~{}s.",
            in_min.len(),
            cfg.max_per_minute,
            wait
        ));
    }

    let to_same_10m = log
        .iter()
        .filter(|e| e.to_jid == to_jid && now.saturating_sub(e.ts) < 600)
        .count();
    if to_same_10m >= cfg.max_per_recipient_10m {
        return Err(format!(
            "Límite anti-spam por contacto alcanzado ({}/{} mensajes en 10 minutos a {}).",
            to_same_10m, cfg.max_per_recipient_10m, to_jid
        ));
    }

    let in_hour = log
        .iter()
        .filter(|e| now.saturating_sub(e.ts) < 3600)
        .count();
    if in_hour >= cfg.max_per_hour {
        return Err(format!(
            "Límite anti-spam horario alcanzado ({}/{} mensajes en 1 hora).",
            in_hour, cfg.max_per_hour
        ));
    }

    if log.len() >= cfg.max_per_day {
        return Err(format!(
            "Límite anti-spam diario alcanzado ({}/{} mensajes en 24 horas).",
            log.len(),
            cfg.max_per_day
        ));
    }

    Ok(())
}

pub fn record_outbound_send(db: &TenantDB, to_jid: &str, text: &str) -> Result<()> {
    record_outbound_send_at(db, to_jid, text, now_epoch())
}

fn record_outbound_send_at(db: &TenantDB, to_jid: &str, text: &str, now: u64) -> Result<()> {
    let mut log = load_send_log(db, now);
    log.push(SendLogEntry {
        ts: now,
        to_jid: to_jid.to_string(),
        text_hash: hash_message(to_jid, text),
    });
    if log.len() > 500 {
        let excess = log.len() - 500;
        log.drain(0..excess);
    }
    let json = serde_json::to_string(&log)?;
    db.set_kv(KV_SEND_LOG, &json)?;
    Ok(())
}

pub struct WhatsAppManager {
    http_client: Arc<reqwest::Client>,
    data_dir: PathBuf,
    bridge_port: u16,
    bridge_url: String,
    shared_secret: String,
    bridge_dir: Option<PathBuf>,
    child: Mutex<Option<tokio::process::Child>>,
    start_lock: Mutex<()>,
    send_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl WhatsAppManager {
    pub fn new(http_client: Arc<reqwest::Client>, data_dir: PathBuf, http_port: u16) -> Self {
        let bridge_port = std::env::var("AEGIS_WHATSAPP_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or_else(|| http_port.saturating_add(10));

        let bridge_url = std::env::var("AEGIS_WHATSAPP_BRIDGE_URL")
            .ok()
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("http://127.0.0.1:{}", bridge_port));

        let wa_root = data_dir.join("whatsapp");
        let _ = std::fs::create_dir_all(&wa_root);
        let shared_secret = Self::load_or_create_secret(&wa_root);
        let bridge_dir = Self::discover_bridge_dir(&data_dir);

        if let Some(ref dir) = bridge_dir {
            info!(
                bridge_dir = %dir.display(),
                bridge_url = %bridge_url,
                "WhatsAppManager initialized"
            );
        } else {
            warn!("WhatsAppManager: tools/whatsapp_bridge not found; local QR bridge disabled until configured.");
        }

        Self {
            http_client,
            data_dir,
            bridge_port,
            bridge_url,
            shared_secret,
            bridge_dir,
            child: Mutex::new(None),
            start_lock: Mutex::new(()),
            send_locks: Mutex::new(HashMap::new()),
        }
    }

    fn load_or_create_secret(wa_root: &Path) -> String {
        if let Ok(env_secret) = std::env::var("AEGIS_WHATSAPP_SECRET") {
            if !env_secret.trim().is_empty() {
                return env_secret.trim().to_string();
            }
        }
        let secret_file = wa_root.join(".bridge_secret");
        if let Ok(existing) = std::fs::read_to_string(&secret_file) {
            let trimmed = existing.trim();
            if trimmed.len() >= 16 {
                return trimmed.to_string();
            }
        }
        let fresh = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let _ = crate::enclave::write_secret_file(&secret_file, fresh.as_bytes());
        fresh
    }

    fn discover_bridge_dir(data_dir: &Path) -> Option<PathBuf> {
        if let Ok(explicit) = std::env::var("AEGIS_WHATSAPP_BRIDGE_DIR") {
            let p = PathBuf::from(explicit.trim());
            if p.join("index.mjs").exists() {
                return Some(p);
            }
        }

        let mut candidates = Vec::new();
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join("tools").join("whatsapp_bridge"));
            if let Some(parent) = cwd.parent() {
                candidates.push(parent.join("tools").join("whatsapp_bridge"));
            }
        }
        if let Ok(exe) = std::env::current_exe() {
            for anc in exe.ancestors().take(5) {
                candidates.push(anc.join("tools").join("whatsapp_bridge"));
            }
        }
        candidates.push(data_dir.join("whatsapp_bridge"));

        candidates
            .into_iter()
            .find(|p| p.join("index.mjs").exists())
    }

    async fn tenant_send_lock(&self, tenant_id: &str) -> Arc<Mutex<()>> {
        let mut map = self.send_locks.lock().await;
        map.entry(tenant_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    async fn is_bridge_healthy(&self) -> bool {
        let url = format!("{}/health", self.bridge_url);
        match self
            .http_client
            .get(&url)
            .timeout(std::time::Duration::from_millis(600))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    pub async fn ensure_bridge_running(&self) -> Result<()> {
        if self.is_bridge_healthy().await {
            return Ok(());
        }

        let _guard = self.start_lock.lock().await;
        if self.is_bridge_healthy().await {
            return Ok(());
        }

        let bridge_dir = self.bridge_dir.as_ref().ok_or_else(|| {
            anyhow!(
                "No se encontró `tools/whatsapp_bridge/index.mjs`. Configurá `AEGIS_WHATSAPP_BRIDGE_DIR`."
            )
        })?;
        if !bridge_dir.join("node_modules").exists() {
            return Err(anyhow!(
                "Faltan dependencias en `tools/whatsapp_bridge`. Ejecutá `npm install` dentro de {}.",
                bridge_dir.display()
            ));
        }

        let wa_sessions_dir = self.data_dir.join("whatsapp");
        tokio::fs::create_dir_all(&wa_sessions_dir).await?;

        let mut child_guard = self.child.lock().await;
        if let Some(ref mut existing) = *child_guard {
            if let Ok(None) = existing.try_wait() {
                // Process exists, wait briefly for health
            } else {
                *child_guard = None;
            }
        }

        if child_guard.is_none() {
            let node_bin = std::env::var("AEGIS_NODE_BIN").unwrap_or_else(|_| "node".to_string());
            let mut cmd = tokio::process::Command::new(&node_bin);
            cmd.arg("index.mjs")
                .current_dir(bridge_dir)
                .env("WA_BRIDGE_PORT", self.bridge_port.to_string())
                .env("WA_BRIDGE_SECRET", &self.shared_secret)
                .env("WA_DATA_DIR", wa_sessions_dir.to_string_lossy().as_ref())
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true);

            #[cfg(windows)]
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW

            let spawned = cmd.spawn().map_err(|e| {
                anyhow!(
                    "No se pudo iniciar Node.js para WhatsApp ({}). Verificá que `node` esté instalado: {}",
                    node_bin,
                    e
                )
            })?;
            *child_guard = Some(spawned);
        }
        drop(child_guard);

        for _ in 0..32 {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            if self.is_bridge_healthy().await {
                info!(bridge_url = %self.bridge_url, "WhatsApp bridge sidecar started cleanly");
                return Ok(());
            }
        }

        Err(anyhow!(
            "El bridge de WhatsApp tardó demasiado en responder en {}",
            self.bridge_url
        ))
    }

    /// CORE-403: Detiene limpiamente el subproceso del bridge Node.js durante el apagado.
    pub async fn shutdown(&self) {
        let child_opt = {
            let mut guard = self.child.lock().await;
            guard.take()
        };
        if let Some(mut child) = child_opt {
            let _ = child.start_kill();
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), child.wait()).await;
        }
    }

    /// Starts the local bridge automatically on boot if any tenant has already paired a QR session.
    pub async fn auto_start_if_paired(&self) {
        let wa_dir = self.data_dir.join("whatsapp");
        let Ok(mut entries) = tokio::fs::read_dir(&wa_dir).await else {
            return;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let creds = entry.path().join("auth").join("creds.json");
            if creds.exists() {
                if let Err(e) = self.ensure_bridge_running().await {
                    warn!("WhatsApp auto-start skipped: {}", e);
                }
                break;
            }
        }
    }

    async fn bridge_request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&Value>,
        timeout_secs: u64,
    ) -> Result<Value> {
        let url = format!("{}{}", self.bridge_url, path);
        let mut req = self
            .http_client
            .request(method, &url)
            .bearer_auth(&self.shared_secret)
            .timeout(std::time::Duration::from_secs(timeout_secs));
        if let Some(payload) = body {
            req = req.json(payload);
        }

        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        let json_val: Value = serde_json::from_str(&text)
            .unwrap_or_else(|_| json!({ "error": "invalid_json", "message": text }));

        if !status.is_success() {
            let msg = json_val["message"]
                .as_str()
                .or_else(|| json_val["error"].as_str())
                .unwrap_or("Error en bridge de WhatsApp");
            if let Some(candidates) = json_val.get("candidates").filter(|c| c.is_array()) {
                return Err(anyhow!(
                    "{}. Coincidencias encontradas: {}",
                    msg,
                    candidates
                ));
            }
            return Err(anyhow!("{}", msg));
        }

        Ok(json_val)
    }

    pub async fn get_status(
        &self,
        tenant_id: &str,
        session_key: &str,
        ensure_if_creds: bool,
    ) -> Result<Value> {
        let (require_confirmation, rate_usage, pending) = {
            let db = TenantDB::open(tenant_id, session_key)?;
            (
                get_require_confirmation(&db),
                get_rate_usage(&db),
                list_pending_messages(&db),
            )
        };

        let creds_path = self
            .data_dir
            .join("whatsapp")
            .join(tenant_id)
            .join("auth")
            .join("creds.json");
        let has_saved_session = creds_path.exists();

        if ensure_if_creds && has_saved_session && !self.is_bridge_healthy().await {
            let _ = self.ensure_bridge_running().await;
        }

        let bridge_state = if self.is_bridge_healthy().await {
            self.bridge_request(
                reqwest::Method::GET,
                &format!("/status?tenant={}", urlencoding::encode(tenant_id)),
                None,
                5,
            )
            .await
            .unwrap_or_else(|e| {
                json!({
                    "status": "disconnected",
                    "connected": false,
                    "last_error": e.to_string(),
                })
            })
        } else {
            json!({
                "status": "disconnected",
                "connected": false,
                "bridge_running": false,
            })
        };

        let bridge_available = self
            .bridge_dir
            .as_ref()
            .map(|p| p.join("node_modules").exists())
            .unwrap_or(false);

        Ok(json!({
            "bridge_available": bridge_available,
            "has_saved_session": has_saved_session,
            "require_confirmation": require_confirmation,
            "rate_usage": rate_usage,
            "pending_messages": pending,
            "connection": bridge_state,
        }))
    }

    pub async fn connect(&self, tenant_id: &str) -> Result<Value> {
        self.ensure_bridge_running().await?;
        self.bridge_request(
            reqwest::Method::POST,
            "/connect",
            Some(&json!({ "tenant": tenant_id })),
            10,
        )
        .await
    }

    pub async fn disconnect(&self, tenant_id: &str, logout: bool) -> Result<Value> {
        if !self.is_bridge_healthy().await {
            if logout {
                let auth_dir = self.data_dir.join("whatsapp").join(tenant_id).join("auth");
                let _ = tokio::fs::remove_dir_all(&auth_dir).await;
            }
            return Ok(json!({
                "status": if logout { "logged_out" } else { "disconnected" },
                "connected": false,
            }));
        }
        self.bridge_request(
            reqwest::Method::POST,
            "/disconnect",
            Some(&json!({ "tenant": tenant_id, "logout": logout })),
            8,
        )
        .await
    }

    pub async fn list_chats(
        &self,
        tenant_id: &str,
        search: Option<&str>,
        limit: usize,
    ) -> Result<Value> {
        self.ensure_bridge_running().await?;
        let url = format!(
            "/chats?tenant={}&search={}&limit={}",
            urlencoding::encode(tenant_id),
            urlencoding::encode(search.unwrap_or("")),
            limit.clamp(1, 50)
        );
        self.bridge_request(reqwest::Method::GET, &url, None, 8)
            .await
    }

    pub async fn read_messages(&self, tenant_id: &str, chat: &str, limit: usize) -> Result<Value> {
        let trimmed = chat.trim();
        if trimmed.is_empty() {
            return Err(anyhow!(
                "Indicá el nombre del chat, contacto o número a leer."
            ));
        }
        self.ensure_bridge_running().await?;
        let url = format!(
            "/messages?tenant={}&chat={}&limit={}",
            urlencoding::encode(tenant_id),
            urlencoding::encode(trimmed),
            limit.clamp(1, 50)
        );
        self.bridge_request(reqwest::Method::GET, &url, None, 8)
            .await
    }

    pub async fn read_messages_with_aliases(
        &self,
        tenant_id: &str,
        session_key: &str,
        chat: &str,
        limit: usize,
    ) -> Result<Value> {
        let trimmed = chat.trim();
        if trimmed.is_empty() {
            return Err(anyhow!(
                "Indicá el nombre del chat, contacto o número a leer."
            ));
        }
        let resolved_alias = TenantDB::open(tenant_id, session_key)
            .ok()
            .and_then(|db| resolve_contact_alias(&db, trimmed));
        let query = resolved_alias
            .as_ref()
            .map(|a| a.target.as_str())
            .unwrap_or(trimmed);
        let mut res = self.read_messages(tenant_id, query, limit).await?;
        if let Some(alias) = resolved_alias {
            if let Some(obj) = res.as_object_mut() {
                obj.insert(
                    "resolved_alias".to_string(),
                    json!({
                        "alias": alias.alias,
                        "target": alias.target,
                        "note": alias.note,
                    }),
                );
            }
        }
        Ok(res)
    }

    pub async fn list_contacts(
        &self,
        tenant_id: &str,
        session_key: &str,
        search: Option<&str>,
        limit: usize,
    ) -> Result<Value> {
        let q = search.map(normalize_alias_key).unwrap_or_default();
        let saved_aliases: Vec<WhatsAppContactAlias> = {
            let db = TenantDB::open(tenant_id, session_key)?;
            list_contact_aliases(&db)
                .into_iter()
                .filter(|a| {
                    if q.is_empty() {
                        return true;
                    }
                    normalize_alias_key(&a.alias).contains(&q)
                        || a.target.contains(search.unwrap_or("").trim())
                        || a.note
                            .as_deref()
                            .map(normalize_alias_key)
                            .map(|n| n.contains(&q))
                            .unwrap_or(false)
                })
                .collect()
        };

        let mut bridge_contacts = Value::Array(vec![]);
        let mut bridge_connected = false;
        let mut total_bridge_contacts = 0usize;

        if self.ensure_bridge_running().await.is_ok() {
            let url = format!(
                "/contacts?tenant={}&search={}&limit={}",
                urlencoding::encode(tenant_id),
                urlencoding::encode(search.unwrap_or("")),
                limit.clamp(1, 100)
            );
            if let Ok(data) = self
                .bridge_request(reqwest::Method::GET, &url, None, 8)
                .await
            {
                bridge_connected = data["connected"].as_bool().unwrap_or(false);
                total_bridge_contacts = data["total_contacts"].as_u64().unwrap_or(0) as usize;
                if data["contacts"].is_array() {
                    bridge_contacts = data["contacts"].clone();
                }
            }
        }

        Ok(json!({
            "saved_aliases": saved_aliases,
            "bridge_connected": bridge_connected,
            "total_bridge_contacts": total_bridge_contacts,
            "bridge_contacts": bridge_contacts,
        }))
    }

    /// Sends or queues a WhatsApp message according to `require_confirmation`,
    /// always enforcing the anti-spam rate limiter before actual delivery.
    pub async fn send_or_queue_message(
        &self,
        tenant_id: &str,
        session_key: &str,
        to: &str,
        text: &str,
    ) -> Result<Value> {
        let clean_to = to.trim();
        let clean_text = text.trim();
        if clean_to.is_empty() || clean_text.is_empty() {
            return Err(anyhow!("Falta destinatario (`to`) o mensaje (`message`)."));
        }

        let matched_alias = TenantDB::open(tenant_id, session_key)
            .ok()
            .and_then(|db| resolve_contact_alias(&db, clean_to));
        let bridge_target = matched_alias
            .as_ref()
            .map(|a| a.target.as_str())
            .unwrap_or(clean_to);

        self.ensure_bridge_running().await?;

        // Check connection & resolve canonical target (phone/name -> JID + display label)
        let status_snap = self
            .bridge_request(
                reqwest::Method::GET,
                &format!("/status?tenant={}", urlencoding::encode(tenant_id)),
                None,
                5,
            )
            .await?;
        if !status_snap["connected"].as_bool().unwrap_or(false) {
            return Err(anyhow!(
                "WhatsApp no está conectado todavía. Pedile al usuario que escanee el QR en Settings → Cuentas Conectadas."
            ));
        }

        let resolved = self
            .bridge_request(
                reqwest::Method::POST,
                "/resolve",
                Some(&json!({ "tenant": tenant_id, "to": bridge_target })),
                8,
            )
            .await?;
        let to_jid = resolved["jid"]
            .as_str()
            .unwrap_or(bridge_target)
            .to_string();
        let bridge_label = resolved["label"]
            .as_str()
            .unwrap_or(bridge_target)
            .to_string();
        let label = match &matched_alias {
            Some(alias) if bridge_label.starts_with('+') || bridge_label == bridge_target => {
                format!("{} ({})", alias.alias, alias.target)
            }
            Some(alias) => format!("{} ({})", bridge_label, alias.alias),
            None => bridge_label,
        };

        let require_conf = {
            let db = TenantDB::open(tenant_id, session_key)?;
            get_require_confirmation(&db)
        };

        if require_conf {
            let pending = {
                let db = TenantDB::open(tenant_id, session_key)?;
                // Verify rate limit wasn't already hit before queuing
                if let Err(reason) = check_rate_limit(&db, &to_jid, clean_text) {
                    return Err(anyhow!(reason));
                }
                queue_pending_message(&db, clean_to, &to_jid, &label, clean_text)?
            };
            return Ok(json!({
                "status": "confirmation_required",
                "require_confirmation": true,
                "pending_id": pending.id,
                "to_jid": pending.to_jid,
                "recipient_label": pending.recipient_label,
                "text": pending.text,
                "instruction": "Mostrá al usuario el destinatario y el texto exacto y pedí confirmación explícita. Si confirma, llamá action='confirm_send'. Si cancela, llamá action='cancel_pending'."
            }));
        }

        self.deliver_with_rate_limit(tenant_id, session_key, &to_jid, &label, clean_text)
            .await
    }

    pub async fn confirm_pending_and_send(
        &self,
        tenant_id: &str,
        session_key: &str,
        pending_id: Option<&str>,
    ) -> Result<Value> {
        let lock = self.tenant_send_lock(tenant_id).await;
        let _guard = lock.lock().await;

        self.ensure_bridge_running().await?;

        // Check connection & rate limit BEFORE removing draft if blocked by transient rate limit
        let status_snap = self
            .bridge_request(
                reqwest::Method::GET,
                &format!("/status?tenant={}", urlencoding::encode(tenant_id)),
                None,
                5,
            )
            .await?;
        if !status_snap["connected"].as_bool().unwrap_or(false) {
            return Err(anyhow!(
                "WhatsApp no está conectado. Reconectá el QR antes de enviar."
            ));
        }

        let item = {
            let db = TenantDB::open(tenant_id, session_key)?;
            let list = list_pending_messages(&db);
            let candidate =
                match pending_id.map(str::trim).filter(|s| !s.is_empty()) {
                    Some(id) => list.iter().find(|i| i.id == id).cloned().ok_or_else(|| {
                        anyhow!("No existe un borrador pendiente con id '{}'.", id)
                    })?,
                    None if list.len() == 1 => list[0].clone(),
                    None if list.is_empty() => {
                        return Err(anyhow!("No hay ningún mensaje pendiente de confirmación."));
                    }
                    None => {
                        return Err(anyhow!(
                            "Hay {} mensajes pendientes ({}). Indicá el pending_id a confirmar.",
                            list.len(),
                            list.iter()
                                .map(|i| i.id.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                };
            if let Err(reason) = check_rate_limit(&db, &candidate.to_jid, &candidate.text) {
                return Err(anyhow!(reason));
            }
            candidate
        };

        let sent = self
            .bridge_request(
                reqwest::Method::POST,
                "/send",
                Some(&json!({
                    "tenant": tenant_id,
                    "to": item.to_jid,
                    "text": item.text,
                })),
                12,
            )
            .await;

        match sent {
            Ok(res) => {
                if let Ok(db) = TenantDB::open(tenant_id, session_key) {
                    let _ = take_pending_message(&db, Some(&item.id));
                    let _ = record_outbound_send(&db, &item.to_jid, &item.text);
                }
                Ok(json!({
                    "status": "sent",
                    "confirmed_id": item.id,
                    "to_jid": item.to_jid,
                    "recipient_label": item.recipient_label,
                    "text": item.text,
                    "bridge_response": res,
                }))
            }
            Err(e) => Err(e),
        }
    }

    async fn deliver_with_rate_limit(
        &self,
        tenant_id: &str,
        session_key: &str,
        to_jid: &str,
        recipient_label: &str,
        text: &str,
    ) -> Result<Value> {
        let lock = self.tenant_send_lock(tenant_id).await;
        let _guard = lock.lock().await;

        {
            let db = TenantDB::open(tenant_id, session_key)?;
            if let Err(reason) = check_rate_limit(&db, to_jid, text) {
                return Err(anyhow!(reason));
            }
        }

        let res = self
            .bridge_request(
                reqwest::Method::POST,
                "/send",
                Some(&json!({
                    "tenant": tenant_id,
                    "to": to_jid,
                    "text": text,
                })),
                12,
            )
            .await?;

        {
            let db = TenantDB::open(tenant_id, session_key)?;
            let _ = record_outbound_send(&db, to_jid, text);
        }

        Ok(json!({
            "status": "sent",
            "to_jid": to_jid,
            "recipient_label": recipient_label,
            "text": text,
            "bridge_response": res,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_flag_defaults_true_and_toggles() {
        let _env = crate::enclave::ScopedTestDataDir::new().unwrap();
        let db = TenantDB::open("t_wa_conf", "secret_key_123").unwrap();

        assert!(get_require_confirmation(&db));
        set_require_confirmation(&db, false).unwrap();
        assert!(!get_require_confirmation(&db));
        set_require_confirmation(&db, true).unwrap();
        assert!(get_require_confirmation(&db));
    }

    #[test]
    fn pending_queue_and_cancel_roundtrip() {
        let _env = crate::enclave::ScopedTestDataDir::new().unwrap();
        let db = TenantDB::open("t_wa_pending", "secret_key_123").unwrap();

        let item =
            queue_pending_message(&db, "Juan", "5491112345678@s.whatsapp.net", "Juan", "Hola!")
                .unwrap();
        assert!(item.id.starts_with("wa_"));
        assert_eq!(list_pending_messages(&db).len(), 1);

        let removed = cancel_pending_messages(&db, Some(&item.id)).unwrap();
        assert_eq!(removed, 1);
        assert!(list_pending_messages(&db).is_empty());
    }

    #[test]
    fn rate_limiter_blocks_burst_duplicates_and_interval() {
        let _env = crate::enclave::ScopedTestDataDir::new().unwrap();
        let db = TenantDB::open("t_wa_rate", "secret_key_123").unwrap();

        let cfg = WhatsAppRateConfig {
            min_interval_secs: 5,
            max_per_minute: 3,
            max_per_recipient_10m: 3,
            dedup_window_secs: 60,
            ..Default::default()
        };
        let saved = set_rate_config(&db, cfg).unwrap();
        assert_eq!(saved.min_interval_secs, 5);

        let t0 = 1_700_000_000;
        assert!(check_rate_limit_at(&db, "54911@s.whatsapp.net", "hola", t0).is_ok());
        record_outbound_send_at(&db, "54911@s.whatsapp.net", "hola", t0).unwrap();

        // 2s later -> blocked by min_interval_secs (5s)
        let err_interval =
            check_rate_limit_at(&db, "54922@s.whatsapp.net", "otro", t0 + 2).unwrap_err();
        assert!(err_interval.contains("esperá"));

        // 10s later, same text to same recipient -> blocked by dedup_window_secs
        let err_dup =
            check_rate_limit_at(&db, "54911@s.whatsapp.net", "hola", t0 + 10).unwrap_err();
        assert!(err_dup.contains("mismo mensaje"));

        // Send 2 more unique messages spaced 6s apart -> hits max_per_minute (3)
        record_outbound_send_at(&db, "54922@s.whatsapp.net", "uno", t0 + 10).unwrap();
        record_outbound_send_at(&db, "54933@s.whatsapp.net", "dos", t0 + 16).unwrap();
        let err_rpm =
            check_rate_limit_at(&db, "54944@s.whatsapp.net", "tres", t0 + 22).unwrap_err();
        assert!(err_rpm.contains("por minuto"));
    }

    #[test]
    fn rate_config_sanitize_enforces_hard_ceilings() {
        let wild = WhatsAppRateConfig {
            min_interval_secs: 0,
            max_per_minute: 9999,
            max_per_hour: 9999,
            max_per_day: 9999,
            max_per_recipient_10m: 9999,
            dedup_window_secs: 0,
        }
        .sanitize();
        assert_eq!(wild.min_interval_secs, 2);
        assert_eq!(wild.max_per_minute, 20);
        assert_eq!(wild.max_per_hour, 120);
        assert_eq!(wild.max_per_day, 400);
        assert_eq!(wild.max_per_recipient_10m, 15);
        assert_eq!(wild.dedup_window_secs, 10);
    }

    #[test]
    fn contact_aliases_save_normalize_and_resolve_roundtrip() {
        let _env = crate::enclave::ScopedTestDataDir::new().unwrap();
        let db = TenantDB::open("t_wa_alias", "secret_key_123").unwrap();

        let saved =
            save_contact_alias(&db, "Ana", "+54 9 11 1234-5678", Some("mi compañera Ana")).unwrap();
        assert_eq!(saved.alias, "Ana");
        assert_eq!(saved.target, "+5491112345678");

        assert_eq!(
            resolve_contact_alias(&db, "ana").unwrap().target,
            "+5491112345678"
        );
        assert_eq!(
            resolve_contact_alias(&db, "mi companera").unwrap().target,
            "+5491112345678"
        );
        assert_eq!(
            resolve_contact_alias(&db, "mi compañera").unwrap().target,
            "+5491112345678"
        );
        assert_eq!(
            resolve_contact_alias(&db, "Ana").unwrap().target,
            "+5491112345678"
        );

        // Updating same alias (different casing/whitespace) overwrites cleanly
        save_contact_alias(&db, "  ANA ", "5491112345678", Some("pareja")).unwrap();
        assert_eq!(list_contact_aliases(&db).len(), 1);
    }
}

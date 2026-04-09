use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiagEntry {
    pub first_seen: String,
    pub last_seen: String,
    pub count: u64,
    pub message: String,
    pub severity: String,
}

/// Global Diagnostic Store for AI Analysis.
/// Stores errors in dev_data/logs/diag.json without spamming the main console.
pub struct DiagnosticLogger {
    log_path: PathBuf,
    entries: Mutex<HashMap<String, DiagEntry>>,
}

static INSTANCE: OnceLock<DiagnosticLogger> = OnceLock::new();

impl DiagnosticLogger {
    pub fn global() -> &'static Self {
        INSTANCE.get_or_init(|| {
            let data_dir = std::env::var("AEGIS_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("c:\\Aegis\\dev_data"));

            let log_dir = data_dir.join("logs");
            let _ = create_dir_all(&log_dir);

            Self {
                log_path: log_dir.join("diag.json"),
                entries: Mutex::new(HashMap::new()),
            }
        })
    }

    pub fn log_error(&self, id: &str, message: &str, severity: &str) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let now = Utc::now().to_rfc3339();

        let is_new = {
            let entry = entries.entry(id.to_string()).or_insert_with(|| DiagEntry {
                first_seen: now.clone(),
                last_seen: now.clone(),
                count: 0,
                message: message.to_string(),
                severity: severity.to_string(),
            });

            entry.count += 1;
            entry.last_seen = now;
            entry.count == 1
        };

        // Persist to JSON on every update (it's dev mode, we favor persistence over perf)
        if let Ok(json) = serde_json::to_string_pretty(&*entries) {
            let _ = std::fs::write(&self.log_path, json);
        }

        // We also append to a plain text log for human readability if preferred
        let txt_path = self.log_path.with_extension("log");
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(txt_path) {
            if is_new {
                let _ = writeln!(
                    file,
                    "[{}] NEW {} [{}]: {}",
                    Utc::now().to_rfc3339(),
                    severity,
                    id,
                    message
                );
            }
        }
    }
}

/// Helper macro to log without duplicates
#[macro_export]
macro_rules! diag_log {
    ($id:expr, $msg:expr) => {
        $crate::scribe::diagnostic::DiagnosticLogger::global().log_error($id, $msg, "ERROR");
    };
    ($id:expr, $severity:expr, $msg:expr) => {
        $crate::scribe::diagnostic::DiagnosticLogger::global().log_error($id, $msg, $severity);
    };
}

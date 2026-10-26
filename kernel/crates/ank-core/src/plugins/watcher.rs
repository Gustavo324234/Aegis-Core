use crate::plugins::PluginManager;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use wasmtime::Module;

/// Normaliza un evento sobre `.wasm` o `.wasm.sig` a la ruta del binario `.wasm` asociado.
fn resolve_watched_wasm_path(path: &Path) -> Option<PathBuf> {
    let file_name = path.file_name()?.to_str()?;
    if file_name.ends_with(".wasm") {
        return Some(path.to_path_buf());
    }
    if let Some(stem) = file_name.strip_suffix(".wasm.sig") {
        return Some(path.with_file_name(format!("{}.wasm", stem)));
    }
    None
}

pub async fn watch_plugins_dir(
    dir_path: String,
    plugin_manager: Arc<RwLock<PluginManager>>,
) -> anyhow::Result<()> {
    if !Path::new(&dir_path).exists() {
        std::fs::create_dir_all(&dir_path)?;
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        move |res: Result<Vec<DebouncedEvent>, notify::Error>| {
            if let Ok(events) = res {
                for event in events {
                    let _ = tx.blocking_send(event.path);
                }
            }
        },
    )?;

    debouncer
        .watcher()
        .watch(Path::new(&dir_path), RecursiveMode::NonRecursive)?;
    info!("Wasm Hot-Reloading Watcher started on {}", dir_path);

    tokio::spawn(async move {
        // Mantenemos el debouncer vivo en este task
        let _keep_alive = debouncer;

        while let Some(raw_path) = rx.recv().await {
            let Some(wasm_path) = resolve_watched_wasm_path(&raw_path) else {
                continue;
            };
            if !wasm_path.exists() {
                continue;
            }
            let Some(path_str) = wasm_path.to_str() else {
                continue;
            };

            info!("Change detected in plugin: {}", path_str);

            // 1. Verificar firma Ed25519 ANTES de invocar Cranelift JIT (CORE-363)
            let (engine, signer) = {
                let pm = plugin_manager.read().await;
                (pm.engine().clone(), pm.signer().clone())
            };

            if let Err(e) = signer.verify_plugin(&wasm_path) {
                warn!(
                    "Hot-Reload deferred/blocked for {} until valid signature is present: {}",
                    path_str, e
                );
                continue;
            }

            // 2. Compilar fuera del write lock una vez autenticada la firma
            match Module::from_file(&engine, path_str) {
                Ok(module) => {
                    // 3. Obtain Write Lock for Atomic Hot-Swap
                    let mut pm_write = plugin_manager.write().await;

                    // 4. Inject and auto-discover
                    if let Err(e) = pm_write.reload_plugin_module(path_str, module).await {
                        error!("Hot-Reload metadata fetch failed for {}: {}", path_str, e);
                    } else {
                        info!(
                            "Plugin {} successfully hot-reloaded (Zero-Downtime)",
                            path_str
                        );
                    }
                }
                Err(e) => {
                    // SRE Rule: Zero-Panic
                    error!("Hot-Reload failed to compile {}: {}", path_str, e);
                }
            }
        }
    });

    Ok(())
}

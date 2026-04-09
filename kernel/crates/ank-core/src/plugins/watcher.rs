use crate::plugins::PluginManager;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info};
use wasmtime::Module;

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

        while let Some(path) = rx.recv().await {
            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                if let Some(path_str) = path.to_str() {
                    info!("Change detected in plugin: {}", path_str);

                    // 1. Compile BEFORE asking for write lock
                    let engine = {
                        let pm = plugin_manager.read().await;
                        pm.engine().clone()
                    };

                    match Module::from_file(&engine, path_str) {
                        Ok(module) => {
                            // 2. Obtain Write Lock for Atomic Hot-Swap
                            let mut pm_write = plugin_manager.write().await;

                            // 3. Inject and auto-discover
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
            }
        }
    });

    Ok(())
}

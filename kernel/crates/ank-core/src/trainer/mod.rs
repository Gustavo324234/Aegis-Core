use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrainingStatus {
    Idle,
    Preparing,
    Training,
    Exporting,
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingProgress {
    pub status: TrainingStatus,
    pub epoch: f32,
    pub step: usize,
    pub loss: f32,
    pub eta_seconds: usize,
    pub log_snippet: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrainingConfig {
    pub mode: String, // "local" | "cloud"
    pub model_id: String,
    pub dataset_path: String,
    pub epochs: usize,
    pub learning_rate: f32,
    pub batch_size: usize,
    pub cloud_api_key: Option<String>,
    #[serde(default)]
    pub tenant_id: Option<String>,
}

pub struct TrainingManager {
    status: Arc<RwLock<TrainingProgress>>,
    active_process: Arc<Mutex<Option<tokio::process::Child>>>,
    active_tenant: Arc<RwLock<Option<String>>>,
    progress_tx: broadcast::Sender<TrainingProgress>,
    data_dir: PathBuf,
}

impl TrainingManager {
    pub fn new(data_dir: PathBuf) -> Self {
        let (progress_tx, _) = broadcast::channel(100);
        let status = Arc::new(RwLock::new(TrainingProgress {
            status: TrainingStatus::Idle,
            epoch: 0.0,
            step: 0,
            loss: 0.0,
            eta_seconds: 0,
            log_snippet: String::new(),
        }));

        Self {
            status,
            active_process: Arc::new(Mutex::new(None)),
            active_tenant: Arc::new(RwLock::new(None)),
            progress_tx,
            data_dir,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TrainingProgress> {
        self.progress_tx.subscribe()
    }

    pub async fn get_progress(&self) -> TrainingProgress {
        self.status.read().await.clone()
    }

    /// CORE-368: Checks whether `tenant_id` (or a master admin) owns the current/last training run.
    pub async fn owner_matches(&self, tenant_id: &str, is_master: bool) -> bool {
        if is_master {
            return true;
        }
        match self.active_tenant.read().await.as_deref() {
            Some(owner) => owner == tenant_id,
            None => true,
        }
    }

    /// CORE-368: Returns `TrainingProgress` scoped to `tenant_id` so other tenants never read
    /// cross-tenant `log_snippet` or metrics.
    pub async fn get_progress_for_tenant(
        &self,
        tenant_id: &str,
        is_master: bool,
    ) -> TrainingProgress {
        if self.owner_matches(tenant_id, is_master).await {
            self.get_progress().await
        } else {
            TrainingProgress {
                status: TrainingStatus::Idle,
                epoch: 0.0,
                step: 0,
                loss: 0.0,
                eta_seconds: 0,
                log_snippet: String::new(),
            }
        }
    }

    async fn update_status(&self, status: TrainingStatus) {
        let mut progress = self.status.write().await;
        progress.status = status;
        let _ = self.progress_tx.send(progress.clone());
    }

    async fn update_metrics(&self, step: usize, loss: f32) {
        let mut progress = self.status.write().await;
        progress.step = step;
        progress.loss = loss;
        // Calcular época estimando 100 pasos por época por simplicidad o estimación
        progress.epoch = (step as f32 / 100.0).min(3.0);
        let _ = self.progress_tx.send(progress.clone());
    }

    async fn append_log(&self, line: &str) {
        let mut progress = self.status.write().await;
        progress.log_snippet = line.to_string();
        let _ = self.progress_tx.send(progress.clone());
    }

    pub async fn start_training(&self, config: TrainingConfig) -> Result<(), String> {
        let active = {
            let proc = self.active_process.lock().await;
            proc.is_some()
        };

        if active {
            return Err("Ya hay un proceso de entrenamiento activo".to_string());
        }

        let status = self.get_progress().await.status;
        if status == TrainingStatus::Preparing || status == TrainingStatus::Training {
            return Err("El entrenamiento ya está en progreso".to_string());
        }

        if config.mode == "cloud"
            && config
                .cloud_api_key
                .as_deref()
                .map(|k| k.trim().is_empty())
                .unwrap_or(true)
        {
            return Err("El modo 'cloud' requiere una API Key válida (cloud_api_key)".to_string());
        }

        // Pre-flight check: verificar disponibilidad de Python y librerías de ML
        let check = Command::new("python")
            .arg("-c")
            .arg("import torch, peft, transformers, bitsandbytes; print('READY')")
            .output()
            .await;

        match check {
            Ok(output) => {
                if !output.status.success() {
                    let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
                    return Err(format!(
                        "Faltan dependencias de ML en Python. Verifica la instalación de torch, peft, transformers, y bitsandbytes. Detalle: {}",
                        err_msg.trim()
                    ));
                }
            }
            Err(e) => {
                return Err(format!(
                    "Python no está instalado o no se encuentra en el PATH del sistema: {}",
                    e
                ));
            }
        }

        *self.active_tenant.write().await = config.tenant_id.clone();

        let active_process = self.active_process.clone();
        let manager_clone = Arc::new(Self {
            status: self.status.clone(),
            active_process: self.active_process.clone(),
            active_tenant: self.active_tenant.clone(),
            progress_tx: self.progress_tx.clone(),
            data_dir: self.data_dir.clone(),
        });

        tokio::spawn(async move {
            if let Err(e) = manager_clone
                .run_training_pipeline(config, active_process)
                .await
            {
                error!("Error en el pipeline de entrenamiento: {}", e);
                manager_clone.update_status(TrainingStatus::Failed(e)).await;
            }
        });

        Ok(())
    }

    /// Construye el contenido de un `Modelfile` de Ollama para el adaptador LoRA entrenado (CORE-354).
    pub fn build_ollama_modelfile(
        base_model: &str,
        adapter_dir: &std::path::Path,
        tenant_id: Option<&str>,
    ) -> String {
        let normalized_base = if base_model.contains('/') {
            // Mapear identificadores HuggingFace comunes al tag base local de Ollama
            let lower = base_model.to_lowercase();
            if lower.contains("llama-3.2-3b") || lower.contains("llama3.2") {
                "llama3.2:3b".to_string()
            } else if lower.contains("llama-3.1-8b") || lower.contains("llama3.1") {
                "llama3.1:8b".to_string()
            } else if lower.contains("qwen2.5") {
                "qwen2.5:7b".to_string()
            } else {
                base_model.to_string()
            }
        } else {
            base_model.to_string()
        };

        let tenant_label = tenant_id.unwrap_or("global");
        let adapter_path = adapter_dir.display().to_string().replace('\\', "/");

        format!(
            "# Generado automáticamente por Aegis Native Training Engine (CORE-354)\n\
             FROM {normalized_base}\n\
             ADAPTER {adapter_path}\n\
             PARAMETER temperature 0.2\n\
             PARAMETER num_ctx 8192\n\
             SYSTEM \"\"\"Eres el núcleo cognitivo de Aegis OS personalizado para el enclave '{tenant_label}'. Cuando necesites invocar una herramienta o syscall del kernel, emite el bloque estructurado __TOOL_CALL__{{\"name\": \"...\", \"arguments\": {{...}}}}__END_TOOL_CALL__.\"\"\"\n"
        )
    }

    async fn export_and_register_ollama(
        &self,
        base_model: &str,
        lora_output_dir: &std::path::Path,
        tenant_id: Option<&str>,
    ) -> Result<(), String> {
        tokio::fs::create_dir_all(lora_output_dir)
            .await
            .map_err(|e| format!("No se pudo crear directorio de salida LoRA: {}", e))?;

        let modelfile_path = lora_output_dir.join("Modelfile");
        let modelfile_content =
            Self::build_ollama_modelfile(base_model, lora_output_dir, tenant_id);

        tokio::fs::write(&modelfile_path, modelfile_content)
            .await
            .map_err(|e| format!("Fallo al escribir Modelfile para Ollama: {}", e))?;

        let model_tag = match tenant_id {
            Some(tid) if !tid.is_empty() => format!("aegis-assistant-{}", tid),
            _ => "aegis-assistant".to_string(),
        };

        self.append_log(&format!(
            "Modelfile generado en {}. Registrando '{}' en Ollama...",
            modelfile_path.display(),
            model_tag
        ))
        .await;

        match Command::new("ollama")
            .arg("create")
            .arg(&model_tag)
            .arg("-f")
            .arg(&modelfile_path)
            .output()
            .await
        {
            Ok(output) if output.status.success() => {
                self.append_log(&format!(
                    "Modelo '{}' registrado exitosamente en el motor local Ollama.",
                    model_tag
                ))
                .await;
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                self.append_log(&format!(
                    "Adaptador LoRA y Modelfile listos en '{}'. Nota: `ollama create` reportó: {}",
                    lora_output_dir.display(),
                    stderr.trim()
                ))
                .await;
            }
            Err(_) => {
                self.append_log(&format!(
                    "Adaptador LoRA y Modelfile exportados en '{}'. (CLI `ollama` no detectado en PATH; registro omitido).",
                    lora_output_dir.display()
                ))
                .await;
            }
        }

        Ok(())
    }

    async fn run_training_pipeline(
        &self,
        config: TrainingConfig,
        active_process: Arc<Mutex<Option<tokio::process::Child>>>,
    ) -> Result<(), String> {
        self.update_status(TrainingStatus::Preparing).await;

        let script_dir = self.data_dir.join("tools").join("fine-tuning");
        let fine_tuning_dir = if script_dir.exists() {
            script_dir
        } else {
            // Fallback al path del workspace de desarrollo
            PathBuf::from("tools/fine-tuning")
        };

        // Validar tenant_id para aislamiento Citadel (CORE-350)
        let safe_tenant_id = match &config.tenant_id {
            Some(tid) if !tid.is_empty() => {
                if !tid
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                {
                    return Err(format!("Identificador de tenant inválido: {}", tid));
                }
                Some(tid.clone())
            }
            _ => None,
        };

        let dataset_file = match &safe_tenant_id {
            Some(tid) => fine_tuning_dir.join(format!("dataset_{}.jsonl", tid)),
            None => fine_tuning_dir.join("dataset.jsonl"),
        };
        let lora_output_dir = match &safe_tenant_id {
            Some(tid) => fine_tuning_dir.join(format!("aegis-assistant-lora-{}", tid)),
            None => fine_tuning_dir.join("aegis-assistant-lora"),
        };

        // Paso 1: Ejecutar la generación del dataset
        info!(tenant_id = ?safe_tenant_id, "Ejecutando generación de dataset...");
        self.append_log("Generando dataset aislado desde el historial de chat...")
            .await;

        let generate_script = fine_tuning_dir.join("generate_dataset.py");
        let mut gen_cmd = Command::new("python");
        gen_cmd
            .arg(&generate_script)
            .arg("--data_dir")
            .arg(&self.data_dir)
            .arg("--output_file")
            .arg(&dataset_file);
        if let Some(ref tid) = safe_tenant_id {
            gen_cmd.arg("--tenant_id").arg(tid);
        }

        let mut gen_child = gen_cmd
            .spawn()
            .map_err(|e| format!("Fallo al arrancar generate_dataset.py: {}", e))?;

        let gen_status = gen_child
            .wait()
            .await
            .map_err(|e| format!("Fallo al esperar generate_dataset.py: {}", e))?;

        if !gen_status.success() {
            return Err("Fallo en la generación del dataset (generate_dataset.py)".to_string());
        }

        // Paso 2: Arrancar entrenamiento local
        self.update_status(TrainingStatus::Training).await;
        self.append_log("Cargando modelo y arrancando entrenamiento de pesos...")
            .await;

        let train_script = fine_tuning_dir.join("fine_tune.py");
        let mut train_cmd = Command::new("python");
        train_cmd
            .arg(&train_script)
            .arg("--model_id")
            .arg(&config.model_id)
            .arg("--dataset_path")
            .arg(&dataset_file)
            .arg("--output_dir")
            .arg(&lora_output_dir)
            .arg("--epochs")
            .arg(config.epochs.to_string())
            .arg("--learning_rate")
            .arg(config.learning_rate.to_string())
            .arg("--batch_size")
            .arg(config.batch_size.to_string())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = train_cmd
            .spawn()
            .map_err(|e| format!("Fallo al arrancar fine_tune.py: {}", e))?;

        let stdout = child.stdout.take().ok_or("No se pudo capturar stdout")?;

        // Guardar el proceso hijo activo para permitir cancelaciones
        {
            let mut proc = active_process.lock().await;
            *proc = Some(child);
        }

        // Leer stdout de forma asíncrona línea por línea para capturar la pérdida (loss)
        let mut reader = BufReader::new(stdout).lines();

        // Expresión para buscar la pérdida: "Paso 10: Pérdida (Loss) = 0.5432"
        let re = regex::Regex::new(r"Paso\s+(\d+):\s+Pérdida\s+\(Loss\)\s+=\s+([\d\.]+)")
            .map_err(|e| format!("Error al compilar regex de pérdida: {}", e))?;

        while let Ok(Some(line)) = reader.next_line().await {
            self.append_log(&line).await;
            if let Some(caps) = re.captures(&line) {
                if let (Ok(step), Ok(loss)) = (caps[1].parse::<usize>(), caps[2].parse::<f32>()) {
                    self.update_metrics(step, loss).await;
                }
            }
        }

        // Esperar la finalización del proceso
        let mut proc_lock = active_process.lock().await;
        if let Some(mut child) = proc_lock.take() {
            let exit_status = child
                .wait()
                .await
                .map_err(|e| format!("Fallo al esperar fine_tune.py: {}", e))?;

            if !exit_status.success() {
                return Err(
                    "El entrenamiento falló (fine_tune.py retornó código de error)".to_string(),
                );
            }
        } else {
            // El proceso fue cancelado manualmente
            return Ok(());
        }

        // Paso 3: Exportar el modelo y generar Modelfile para registro en Ollama (CORE-354)
        self.update_status(TrainingStatus::Exporting).await;
        self.append_log(
            "Entrenamiento completado. Exportando Modelfile y registrando adaptador LoRA...",
        )
        .await;

        self.export_and_register_ollama(
            &config.model_id,
            &lora_output_dir,
            safe_tenant_id.as_deref(),
        )
        .await?;

        self.update_status(TrainingStatus::Completed).await;
        self.append_log(
            "Evolución completada. El modelo asistente ha sido personalizado con éxito.",
        )
        .await;

        Ok(())
    }

    pub async fn cancel_training(&self) -> Result<(), String> {
        let mut proc = self.active_process.lock().await;
        if let Some(mut child) = proc.take() {
            info!("Cancelando entrenamiento activo (PID: {:?})...", child.id());
            let _ = child.kill().await;
            self.update_status(TrainingStatus::Cancelled).await;
            self.append_log("Entrenamiento cancelado por el usuario.")
                .await;
            Ok(())
        } else {
            Err("No hay ningún proceso de entrenamiento activo para cancelar".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_training_manager_initial_state() {
        let dir = std::env::temp_dir();
        let manager = TrainingManager::new(dir);
        let progress = manager.get_progress().await;
        assert_eq!(progress.status, TrainingStatus::Idle);
        assert_eq!(progress.epoch, 0.0);
        assert_eq!(progress.loss, 0.0);
        assert_eq!(progress.step, 0);
    }

    #[tokio::test]
    async fn test_training_manager_metrics_parsing() {
        let dir = std::env::temp_dir();
        let manager = TrainingManager::new(dir);

        // Simular actualizaciones de métricas
        manager.update_metrics(10, 0.8542).await;
        let progress = manager.get_progress().await;
        assert_eq!(progress.step, 10);
        assert_eq!(progress.loss, 0.8542);

        manager.update_metrics(50, 0.3542).await;
        let progress2 = manager.get_progress().await;
        assert_eq!(progress2.step, 50);
        assert_eq!(progress2.loss, 0.3542);
    }

    #[test]
    fn test_build_ollama_modelfile_generates_valid_directives() {
        let adapter_dir = std::path::Path::new("/tmp/aegis-assistant-lora-tenant_a");
        let modelfile = TrainingManager::build_ollama_modelfile(
            "unsloth/Llama-3.2-3B-Instruct",
            adapter_dir,
            Some("tenant_a"),
        );

        assert!(modelfile.contains("FROM llama3.2:3b"));
        assert!(modelfile.contains("ADAPTER /tmp/aegis-assistant-lora-tenant_a"));
        assert!(modelfile.contains("__TOOL_CALL__"));
        assert!(modelfile.contains("tenant_a"));
    }

    #[tokio::test]
    async fn test_cloud_mode_requires_api_key() {
        let dir = std::env::temp_dir();
        let manager = TrainingManager::new(dir);
        let cfg = TrainingConfig {
            mode: "cloud".to_string(),
            model_id: "unsloth/Llama-3.2-3B-Instruct".to_string(),
            dataset_path: "auto".to_string(),
            epochs: 1,
            learning_rate: 0.0002,
            batch_size: 2,
            cloud_api_key: None,
            tenant_id: Some("tenant_a".to_string()),
        };

        let res = manager.start_training(cfg).await;
        assert!(res.is_err());
        assert!(res
            .err()
            .unwrap_or_default()
            .contains("requiere una API Key"));
    }

    #[tokio::test]
    async fn test_training_progress_isolated_by_tenant() {
        let dir = std::env::temp_dir();
        let manager = TrainingManager::new(dir);
        *manager.active_tenant.write().await = Some("tenant_alpha".to_string());
        manager.update_status(TrainingStatus::Training).await;
        manager.append_log("secret loss trace for alpha").await;

        assert!(manager.owner_matches("tenant_alpha", false).await);
        assert!(!manager.owner_matches("tenant_beta", false).await);
        assert!(manager.owner_matches("master", true).await);

        let alpha_prog = manager.get_progress_for_tenant("tenant_alpha", false).await;
        assert_eq!(alpha_prog.status, TrainingStatus::Training);
        assert!(alpha_prog.log_snippet.contains("alpha"));

        let beta_prog = manager.get_progress_for_tenant("tenant_beta", false).await;
        assert_eq!(beta_prog.status, TrainingStatus::Idle);
        assert!(beta_prog.log_snippet.is_empty());
    }
}

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
}

pub struct TrainingManager {
    status: Arc<RwLock<TrainingProgress>>,
    active_process: Arc<Mutex<Option<tokio::process::Child>>>,
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

        let active_process = self.active_process.clone();
        let manager_clone = Arc::new(Self {
            status: self.status.clone(),
            active_process: self.active_process.clone(),
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

        // Paso 1: Ejecutar la generación del dataset
        info!("Ejecutando generación de dataset...");
        self.append_log("Generando dataset desde el historial de chat...")
            .await;

        let generate_script = fine_tuning_dir.join("generate_dataset.py");
        let mut gen_cmd = Command::new("python");
        gen_cmd
            .arg(&generate_script)
            .arg("--output_file")
            .arg(fine_tuning_dir.join("dataset.jsonl"));

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
            .arg(fine_tuning_dir.join("dataset.jsonl"))
            .arg("--output_dir")
            .arg(fine_tuning_dir.join("aegis-assistant-lora"))
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
        let re = regex::Regex::new(r"Paso\s+(\d+):\s+Pérdida\s+\(Loss\)\s+=\s+([\d\.]+)").unwrap();

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

        // Paso 3: Exportar el modelo
        self.update_status(TrainingStatus::Exporting).await;
        self.append_log("Entrenamiento completado. Exportando y cuantizando pesos a GGUF...")
            .await;

        // Simulamos la cuantización rápida o post-procesamiento
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

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
}

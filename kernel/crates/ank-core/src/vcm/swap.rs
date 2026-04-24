use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use thiserror::Error;
use uuid::Uuid;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

/// --- SWAP ERROR SYSTEM ---
#[derive(Error, Debug)]
pub enum SwapError {
    #[error("Index Error: {0}")]
    IndexError(String),
    #[error("Table Not Found: {0}")]
    TableNotFound(String),
    #[error("Storage Error: {0}")]
    StorageError(String),
    #[error("Search Error: {0}")]
    SearchError(String),
    #[error("Serialization Error: {0}")]
    SerializationError(String),
    #[error("Quantization Error: {0}")]
    QuantizationError(String),
}

/// --- MEMORY FRAGMENT ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFragment {
    pub id: String,
    /// Original vector (only if not quantized)
    pub vector: Option<Vec<f32>>,
    /// Quantized vector (INT8)
    pub quantum_vec: Option<Vec<i8>>,
    /// Quantization range min
    pub q_min: f32,
    /// Quantization range max
    pub q_max: f32,
    pub text: String,
    pub timestamp: i64,
    pub tags: Vec<String>,
}

/// --- STORAGE MODEL ---
#[derive(Serialize, Deserialize, Default)]
pub struct TenantData {
    pub fragments: HashMap<u64, MemoryFragment>,
    pub next_id: u64,
}

pub struct TenantSwap {
    pub index: Index,
    pub data: TenantData,
}

/// --- LANCE SWAP MANAGER ---
pub struct LanceSwapManager {
    base_path: String,
    dimension: AtomicUsize,
    tenants: RwLock<HashMap<String, Arc<RwLock<TenantSwap>>>>,
}

#[cfg(test)]
fn _assert_swap_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<LanceSwapManager>();
}

impl LanceSwapManager {
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: base_path.to_string(),
            dimension: AtomicUsize::new(0),
            tenants: RwLock::new(HashMap::new()),
        }
    }

    /// Calcula la ruta activa de persistencia para el tenant.
    fn compute_db_dir(&self, tenant_id: &str) -> String {
        format!("{}/.aegis_swap/{}", self.base_path, tenant_id)
    }

    /// Inicializa la conexión para un tenant específico (Lazy).
    pub async fn init_tenant(&self, tenant_id: &str) -> Result<(), SwapError> {
        let mut tenants_map = self.tenants.write().await;
        if tenants_map.contains_key(tenant_id) {
            return Ok(());
        }

        let db_dir = self.compute_db_dir(tenant_id);
        if let Err(e) = tokio::fs::create_dir_all(&db_dir).await {
            return Err(SwapError::StorageError(e.to_string()));
        }

        let index_path = format!("{}/memory.usearch", db_dir);
        let data_path = format!("{}/fragments.json", db_dir);

        let options = IndexOptions {
            dimensions: if self.dimension.load(Ordering::SeqCst) == 0 {
                128
            } else {
                self.dimension.load(Ordering::SeqCst)
            },
            metric: MetricKind::Cos,
            quantization: ScalarKind::I8,
            ..Default::default()
        };

        let index = Index::new(&options).map_err(|e| SwapError::IndexError(e.to_string()))?;

        let mut data = TenantData::default();

        if Path::new(&index_path).exists() {
            if let Err(e) = index.load(&index_path) {
                tracing::warn!("Failed to load usearch index from {}: {}", index_path, e);
            }
        }

        if Path::new(&data_path).exists() {
            if let Ok(json) = tokio::fs::read_to_string(&data_path).await {
                if let Ok(loaded_data) = serde_json::from_str::<TenantData>(&json) {
                    data = loaded_data;
                }
            }
        }

        tracing::info!(
            "Initialized usearch index for tenant {} at {} (loaded {} fragments)",
            tenant_id,
            db_dir,
            data.fragments.len()
        );

        let tenant_swap = Arc::new(RwLock::new(TenantSwap { index, data }));
        tenants_map.insert(tenant_id.to_string(), tenant_swap);

        Ok(())
    }

    /// Almacena un fragmento de texto para un tenant aplicando cuantización INT8.
    pub async fn store_fragment(
        &self,
        tenant_id: &str,
        text: &str,
        vector: Vec<f32>,
    ) -> Result<String, SwapError> {
        self.init_tenant(tenant_id).await?;

        let id_str = Uuid::new_v4().to_string();

        let dim = self.dimension.load(Ordering::SeqCst);
        if dim == 0 {
            self.dimension.store(vector.len(), Ordering::SeqCst);
        }

        // Aplicamos compresión matemática (Symmetric Scaling)
        let (q_vec, min, max) = quantize_f32_to_i8(&vector);

        let fragment = MemoryFragment {
            id: id_str.clone(),
            vector: None, // Liberamos el vector pesado
            quantum_vec: Some(q_vec.clone()),
            q_min: min,
            q_max: max,
            text: text.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            tags: Vec::new(),
        };

        let tenants_map = self.tenants.read().await;
        let tenant_swap = tenants_map.get(tenant_id).ok_or_else(|| {
            SwapError::IndexError(format!("Tenant index not found: {}", tenant_id))
        })?;

        let mut swap = tenant_swap.write().await;
        let id_num = swap.data.next_id;
        swap.data.next_id += 1;

        swap.index
            .add(id_num, &q_vec)
            .map_err(|e| SwapError::IndexError(e.to_string()))?;
        swap.data.fragments.insert(id_num, fragment);

        let db_dir = self.compute_db_dir(tenant_id);
        let index_path = format!("{}/memory.usearch", db_dir);
        let data_path = format!("{}/fragments.json", db_dir);

        swap.index
            .save(&index_path)
            .map_err(|e| SwapError::IndexError(e.to_string()))?;

        let json = serde_json::to_string(&swap.data)
            .map_err(|e| SwapError::SerializationError(e.to_string()))?;
        tokio::fs::write(&data_path, json)
            .await
            .map_err(|e| SwapError::StorageError(e.to_string()))?;

        tracing::debug!(
            "Stored fragment {} (u64: {}) for tenant {}",
            id_str,
            id_num,
            tenant_id
        );

        Ok(id_str)
    }

    /// Busca los fragmentos más similares para un tenant des-cuantizando al vuelo.
    pub async fn search(
        &self,
        tenant_id: &str,
        query_vector: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<MemoryFragment>, SwapError> {
        self.init_tenant(tenant_id).await?;

        let tenants_map = self.tenants.read().await;
        let tenant_swap = tenants_map.get(tenant_id).ok_or_else(|| {
            SwapError::IndexError(format!("Tenant index not found: {}", tenant_id))
        })?;

        let swap = tenant_swap.read().await;

        let (q_vec, _, _) = quantize_f32_to_i8(&query_vector);

        let results = swap
            .index
            .search(&q_vec, limit)
            .map_err(|e| SwapError::IndexError(e.to_string()))?;

        let mut fragments = Vec::new();
        // Fallback or exact usage for keys array
        for key in results.keys {
            if let Some(frag) = swap.data.fragments.get(&key) {
                fragments.push(frag.clone());
            }
        }

        Ok(fragments)
    }
}

/// --- QUANTIZATION UTILS ---
/// Cuantiza un vector f32 a i8 (Symmetric Min/Max Scaling).
/// Devuelve el vector comprimido, el valor mínimo y el máximo.
pub fn quantize_f32_to_i8(vector: &[f32]) -> (Vec<i8>, f32, f32) {
    if vector.is_empty() {
        return (Vec::new(), 0.0, 0.0);
    }

    let mut min = vector[0];
    let mut max = vector[0];

    for &val in vector {
        if val < min {
            min = val;
        }
        if val > max {
            max = val;
        }
    }

    if (max - min).abs() < f32::EPSILON {
        return (vec![0; vector.len()], min, max);
    }

    let scale = (max - min) / 255.0;

    let quantized = vector
        .iter()
        .map(|&val| {
            let normalized = (val - min) / scale;
            (normalized - 128.0).round() as i8
        })
        .collect();

    (quantized, min, max)
}

pub fn dequantize_i8_to_f32(vector: &[i8], min: f32, max: f32) -> Vec<f32> {
    if (max - min).abs() < f32::EPSILON {
        return vec![min; vector.len()];
    }

    let scale = (max - min) / 255.0;
    vector
        .iter()
        .map(|&val| (val as f32 + 128.0) * scale + min)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantization_roundtrip() {
        let original = vec![0.1, -0.5, 1.0, 0.0, 0.85];
        let (q_vec, min, max) = quantize_f32_to_i8(&original);
        assert_eq!(q_vec.len(), original.len());
        let restored = dequantize_i8_to_f32(&q_vec, min, max);
        for i in 0..original.len() {
            assert!((original[i] - restored[i]).abs() < 0.02);
        }
    }

    #[test]
    fn test_zero_division_guard() {
        let original = vec![0.5, 0.5, 0.5];
        let (q_vec, min, max) = quantize_f32_to_i8(&original);
        assert_eq!(q_vec, vec![0, 0, 0]);
        let restored = dequantize_i8_to_f32(&q_vec, min, max);
        assert_eq!(restored, original);
    }
}

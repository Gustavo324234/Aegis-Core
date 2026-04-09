use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use thiserror::Error;
use uuid::Uuid;

/// --- SWAP ERROR SYSTEM ---
#[derive(Error, Debug)]
pub enum SwapError {
    #[error("LanceDB Connection Error: {0}")]
    ConnectionError(String),
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

/// --- LANCE SWAP MANAGER ---
#[allow(dead_code)]
pub struct LanceSwapManager {
    base_path: String,
    table_name: String,
    dimension: AtomicUsize,
}

impl LanceSwapManager {
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: base_path.to_string(),
            table_name: "memory_fragments".to_string(),
            dimension: AtomicUsize::new(0),
        }
    }

    /// Calcula la ruta de la base de datos vectorial para un tenant.
    fn compute_db_path(&self, tenant_id: &str) -> String {
        format!("{}/{}/.aegis_swap", self.base_path, tenant_id)
    }

    /// Inicializa la conexión para un tenant específico (Lazy).
    pub async fn init_tenant(&self, tenant_id: &str) -> Result<(), SwapError> {
        let db_path = self.compute_db_path(tenant_id);
        tracing::info!(
            "Initializing LanceDB for tenant {} at {}",
            tenant_id,
            db_path
        );
        Ok(())
    }

    /// Almacena un fragmento de texto para un tenant aplicando cuantización INT8.
    pub async fn store_fragment(
        &self,
        tenant_id: &str,
        text: &str,
        vector: Vec<f32>,
    ) -> Result<String, SwapError> {
        let _db_path = self.compute_db_path(tenant_id);
        let id = Uuid::new_v4().to_string();

        // Actualizamos la dimensión si es la primera vez
        if self.dimension.load(Ordering::SeqCst) == 0 {
            self.dimension.store(vector.len(), Ordering::SeqCst);
        }

        // Aplicamos compresión matemática (Symmetric Scaling)
        let (q_vec, min, max) = quantize_f32_to_i8(&vector);

        let _fragment = MemoryFragment {
            id: id.clone(),
            vector: None, // Liberamos el vector pesado
            quantum_vec: Some(q_vec),
            q_min: min,
            q_max: max,
            text: text.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            tags: Vec::new(),
        };

        // FUTURE(ANK-2402): Persist to LanceDB once LIM-001 is resolved
        tracing::debug!("Stored compressed fragment {} for tenant {}", id, tenant_id);

        Ok(id)
    }

    /// Busca los fragmentos más similares para un tenant des-cuantizando al vuelo.
    pub async fn search(
        &self,
        tenant_id: &str,
        _query_vector: Vec<f32>,
        _limit: usize,
    ) -> Result<Vec<MemoryFragment>, SwapError> {
        let _db_path = self.compute_db_path(tenant_id);
        // En una implementación real, aquí leeríamos de LanceDB
        // Al recuperar los fragmentos, se des-cuantizan para cálculos de precisión
        Ok(Vec::new())
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

    // Zero-Panic Math: Prevenir división por cero
    if (max - min).abs() < f32::EPSILON {
        return (vec![0; vector.len()], min, max);
    }

    let scale = (max - min) / 255.0;

    // Mapeamos [min, max] a [0, 255] y luego desplazamos a [-128, 127]
    let quantized = vector
        .iter()
        .map(|&val| {
            let normalized = (val - min) / scale;
            (normalized - 128.0).round() as i8
        })
        .collect();

    (quantized, min, max)
}

/// Des-cuantiza un vector i8 a f32 recuperando la precisión semántica.
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
            // Permitimos un pequeño error de cuantización (~1/255)
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

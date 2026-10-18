use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};
use uuid::Uuid;

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
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct TenantData {
    pub fragments: HashMap<u64, MemoryFragment>,
    pub next_id: u64,
    #[serde(default)]
    pub dimension: usize,
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

fn derive_swap_key(tenant_id: &str) -> String {
    let master_secret = std::env::var("AEGIS_MASTER_KEY")
        .unwrap_or_else(|_| "aegis_citadel_swap_default_master_key_v1".to_string());
    let mut hasher = Sha256::new();
    hasher.update(b"aegis-l3-swap:");
    hasher.update(master_secret.as_bytes());
    hasher.update(b":");
    hasher.update(tenant_id.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn open_encrypted_swap_db(db_path: &str, tenant_id: &str) -> Result<Connection, SwapError> {
    let conn = Connection::open(db_path)
        .map_err(|e| SwapError::StorageError(format!("Failed to open swap DB: {}", e)))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| SwapError::StorageError(format!("Failed to set busy_timeout: {}", e)))?;
    let key = derive_swap_key(tenant_id);
    conn.pragma_update(None, "key", &key)
        .map_err(|e| SwapError::StorageError(format!("Failed to set SQLCipher key: {}", e)))?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
        .map_err(|e| {
            SwapError::StorageError(format!("SQLCipher swap DB verification failed: {}", e))
        })?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS memory_fragments (
            key_id INTEGER PRIMARY KEY,
            fragment_json TEXT NOT NULL,
            created_at INTEGER NOT NULL
        )",
        [],
    )
    .map_err(|e| SwapError::StorageError(format!("Failed to init swap DB schema: {}", e)))?;
    Ok(conn)
}

fn create_usearch_index(dimensions: usize) -> Result<Index, SwapError> {
    let options = IndexOptions {
        dimensions: dimensions.max(1),
        metric: MetricKind::Cos,
        quantization: ScalarKind::I8,
        ..Default::default()
    };
    let index = Index::new(&options).map_err(|e| SwapError::IndexError(e.to_string()))?;
    index
        .reserve(1024)
        .map_err(|e| SwapError::IndexError(format!("Failed to reserve index capacity: {}", e)))?;
    Ok(index)
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
        self.init_tenant_with_dim(tenant_id, None).await
    }

    /// Inicializa la conexión para un tenant específico con dimensión sugerida (CORE-349).
    async fn init_tenant_with_dim(
        &self,
        tenant_id: &str,
        hint_dim: Option<usize>,
    ) -> Result<(), SwapError> {
        {
            let tenants_map = self.tenants.read().await;
            if tenants_map.contains_key(tenant_id) {
                return Ok(());
            }
        }

        let mut tenants_map = self.tenants.write().await;
        if tenants_map.contains_key(tenant_id) {
            return Ok(());
        }

        let db_dir = self.compute_db_dir(tenant_id);
        if let Err(e) = tokio::fs::create_dir_all(&db_dir).await {
            return Err(SwapError::StorageError(e.to_string()));
        }

        let index_path = format!("{}/memory.usearch", db_dir);
        let encrypted_db_path = format!("{}/fragments.db", db_dir);
        let legacy_json_path = format!("{}/fragments.json", db_dir);

        // Cargar fragmentos desde SQLCipher (y migrar fragments.json legacy si existe)
        let tid_owned = tenant_id.to_string();
        let enc_db_clone = encrypted_db_path.clone();
        let leg_json_clone = legacy_json_path.clone();
        let mut data = tokio::task::spawn_blocking(move || -> Result<TenantData, SwapError> {
            let conn = open_encrypted_swap_db(&enc_db_clone, &tid_owned)?;

            // Migración transparente desde fragments.json en texto plano (CORE-349)
            if Path::new(&leg_json_clone).exists() {
                if let Ok(json_str) = std::fs::read_to_string(&leg_json_clone) {
                    if let Ok(legacy_data) = serde_json::from_str::<TenantData>(&json_str) {
                        for (k, frag) in &legacy_data.fragments {
                            if let Ok(frag_json) = serde_json::to_string(frag) {
                                let _ = conn.execute(
                                    "INSERT OR REPLACE INTO memory_fragments (key_id, fragment_json, created_at) VALUES (?1, ?2, ?3)",
                                    rusqlite::params![*k as i64, frag_json, frag.timestamp],
                                );
                            }
                        }
                    }
                }
                let _ = std::fs::remove_file(&leg_json_clone);
            }

            let mut stmt = conn
                .prepare("SELECT key_id, fragment_json FROM memory_fragments")
                .map_err(|e| SwapError::StorageError(e.to_string()))?;
            let rows = stmt
                .query_map([], |row| {
                    let k: i64 = row.get(0)?;
                    let json: String = row.get(1)?;
                    Ok((k as u64, json))
                })
                .map_err(|e| SwapError::StorageError(e.to_string()))?;

            let mut loaded = TenantData::default();
            for row_res in rows {
                let (k, json) = row_res.map_err(|e| SwapError::StorageError(e.to_string()))?;
                let frag: MemoryFragment = serde_json::from_str(&json)
                    .map_err(|e| SwapError::SerializationError(e.to_string()))?;
                if loaded.dimension == 0 {
                    if let Some(ref qv) = frag.quantum_vec {
                        if !qv.is_empty() {
                            loaded.dimension = qv.len();
                        }
                    }
                }
                if k >= loaded.next_id {
                    loaded.next_id = k + 1;
                }
                loaded.fragments.insert(k, frag);
            }
            Ok(loaded)
        })
        .await
        .map_err(|e| SwapError::StorageError(e.to_string()))??;

        let resolved_dim = if data.dimension > 0 {
            data.dimension
        } else if let Some(h) = hint_dim.filter(|&d| d > 0) {
            h
        } else {
            let global_dim = self.dimension.load(Ordering::SeqCst);
            if global_dim > 0 {
                global_dim
            } else {
                128
            }
        };
        data.dimension = resolved_dim;

        let index = create_usearch_index(resolved_dim)?;

        if Path::new(&index_path).exists() {
            if let Err(e) = index.load(&index_path) {
                tracing::warn!("Failed to load usearch index from {}: {}", index_path, e);
            }
        }

        tracing::info!(
            "Initialized encrypted L3 swap for tenant {} at {} (loaded {} fragments, dim={})",
            tenant_id,
            db_dir,
            data.fragments.len(),
            resolved_dim
        );

        let tenant_swap = Arc::new(RwLock::new(TenantSwap { index, data }));
        tenants_map.insert(tenant_id.to_string(), tenant_swap);

        Ok(())
    }

    /// Almacena un fragmento de texto para un tenant aplicando cuantización INT8 y persistencia SQLCipher (CORE-349).
    pub async fn store_fragment(
        &self,
        tenant_id: &str,
        text: &str,
        vector: Vec<f32>,
    ) -> Result<String, SwapError> {
        if vector.is_empty() {
            return Err(SwapError::QuantizationError(
                "Cannot store empty embedding vector".to_string(),
            ));
        }

        let vec_dim = vector.len();
        let _ = self
            .dimension
            .compare_exchange(0, vec_dim, Ordering::SeqCst, Ordering::SeqCst);

        self.init_tenant_with_dim(tenant_id, Some(vec_dim)).await?;

        let id_str = Uuid::new_v4().to_string();

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

        let tenant_swap = {
            let tenants_map = self.tenants.read().await;
            tenants_map.get(tenant_id).cloned().ok_or_else(|| {
                SwapError::IndexError(format!("Tenant index not found: {}", tenant_id))
            })?
        };

        let id_num = {
            let mut swap = tenant_swap.write().await;

            // Si el índice se inicializó vacío con otra dimensión (ej. 128 por defecto en search previo),
            // recrearlo automáticamente con la dimensión real del embedding.
            if swap.index.dimensions() != vec_dim {
                if swap.data.fragments.is_empty() {
                    swap.index = create_usearch_index(vec_dim)?;
                    swap.data.dimension = vec_dim;
                } else {
                    return Err(SwapError::IndexError(format!(
                        "Vector dimension mismatch for tenant {}: expected {}, got {}",
                        tenant_id,
                        swap.index.dimensions(),
                        vec_dim
                    )));
                }
            }

            if swap.index.size() + 1 >= swap.index.capacity() {
                let new_cap = (swap.index.capacity().max(64)) * 2;
                swap.index
                    .reserve(new_cap)
                    .map_err(|e| SwapError::IndexError(e.to_string()))?;
            }

            let id_num = swap.data.next_id;
            swap.data.next_id += 1;

            swap.index
                .add(id_num, &q_vec)
                .map_err(|e| SwapError::IndexError(e.to_string()))?;
            swap.data.fragments.insert(id_num, fragment.clone());
            id_num
        };

        let db_dir = self.compute_db_dir(tenant_id);
        let index_path = format!("{}/memory.usearch", db_dir);
        let encrypted_db_path = format!("{}/fragments.db", db_dir);

        let fragment_json = serde_json::to_string(&fragment)
            .map_err(|e| SwapError::SerializationError(e.to_string()))?;
        let created_at = fragment.timestamp;
        let tid_owned = tenant_id.to_string();

        // Offload blocking CPU/IO save of index and encrypted SQLCipher row to threadpool
        let tenant_swap_clone = Arc::clone(&tenant_swap);
        tokio::task::spawn_blocking(move || -> Result<(), SwapError> {
            {
                let swap_read = tenant_swap_clone.blocking_read();
                swap_read
                    .index
                    .save(&index_path)
                    .map_err(|e| SwapError::IndexError(e.to_string()))?;
            }
            let conn = open_encrypted_swap_db(&encrypted_db_path, &tid_owned)?;
            conn.execute(
                "INSERT OR REPLACE INTO memory_fragments (key_id, fragment_json, created_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![id_num as i64, fragment_json, created_at],
            )
            .map_err(|e| SwapError::StorageError(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| SwapError::StorageError(e.to_string()))??;

        tracing::debug!(
            "Stored encrypted fragment {} (u64: {}, dim: {}) for tenant {}",
            id_str,
            id_num,
            vec_dim,
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
        self.init_tenant_with_dim(tenant_id, Some(query_vector.len()))
            .await?;

        let tenants_map = self.tenants.read().await;
        let tenant_swap = tenants_map.get(tenant_id).ok_or_else(|| {
            SwapError::IndexError(format!("Tenant index not found: {}", tenant_id))
        })?;

        let swap = tenant_swap.read().await;
        if swap.data.fragments.is_empty() || query_vector.is_empty() {
            return Ok(Vec::new());
        }

        if query_vector.len() != swap.index.dimensions() {
            tracing::warn!(
                tenant_id = %tenant_id,
                query_dim = query_vector.len(),
                index_dim = swap.index.dimensions(),
                "Skipping L3 search due to query/index dimension mismatch"
            );
            return Ok(Vec::new());
        }

        let (q_vec, _, _) = quantize_f32_to_i8(&query_vector);

        let results = swap
            .index
            .search(&q_vec, limit)
            .map_err(|e| SwapError::IndexError(e.to_string()))?;

        let mut fragments = Vec::new();
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

    #[tokio::test]
    async fn test_store_and_search_dynamic_dimension_and_sqlcipher_persistence(
    ) -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let base_path = temp_dir.path().to_str().unwrap_or(".");
        let swap = LanceSwapManager::new(base_path);

        // 1. Llamar a search primero con vector dummy de 128 dims sobre índice vacío
        let empty_res = swap.search("tenant_alpha", vec![0.0; 128], 5).await?;
        assert!(empty_res.is_empty());

        // 2. Ahora insertar un embedding real de 384 dimensiones (debe re-inicializar el índice vacío a 384)
        let mut vec_384 = vec![0.1f32; 384];
        vec_384[0] = 0.95;
        vec_384[1] = -0.45;

        let frag_id = swap
            .store_fragment(
                "tenant_alpha",
                "Recuerdo confidencial en L3",
                vec_384.clone(),
            )
            .await?;
        assert!(!frag_id.is_empty());

        // Verificar que se creó fragments.db cifrado con SQLCipher y NO fragments.json en texto plano
        let db_dir = format!("{}/.aegis_swap/tenant_alpha", base_path);
        assert!(Path::new(&format!("{}/fragments.db", db_dir)).exists());
        assert!(!Path::new(&format!("{}/fragments.json", db_dir)).exists());

        // Verificar que abrir fragments.db sin clave SQLCipher falla al leer
        let raw_conn = Connection::open(format!("{}/fragments.db", db_dir))?;
        let raw_read = raw_conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()));
        assert!(
            raw_read.is_err(),
            "fragments.db must be encrypted with SQLCipher"
        );

        // 3. Re-instanciar LanceSwapManager desde disco y verificar que recupera la dimensión 384 y el fragmento
        let swap_reloaded = LanceSwapManager::new(base_path);
        let hits = swap_reloaded.search("tenant_alpha", vec_384, 5).await?;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "Recuerdo confidencial en L3");

        Ok(())
    }
}

// #![cfg(feature = "local_llm")]

use crate::chal::{DriverStatus, ExecutionError, GenerateStreamResult, Grammar, InferenceDriver, SystemError};
use async_trait::async_trait;
use std::pin::Pin;
use tokio_stream::Stream;
use tracing::info;

/// --- COGNITIVE NATIVE DRIVER (LLAMA-CPP-2) ---
#[allow(dead_code)]
pub struct LlamaNativeDriver {
    n_gpu_layers: u32,
    ctx_size: u32,
}

impl LlamaNativeDriver {
    /// Inicializa una instancia del Driver sin cargar un modelo.
    pub fn new(n_gpu_layers: u32, ctx_size: u32) -> anyhow::Result<Self> {
        Ok(Self {
            n_gpu_layers,
            ctx_size,
        })
    }
}

#[async_trait]
impl InferenceDriver for LlamaNativeDriver {
    async fn generate_stream(
        &self,
        _prompt: String,
        _grammar: Option<Grammar>,
    ) -> GenerateStreamResult
    {
        Err(SystemError::ModelNotFound(
            "Native driver disabled for tests".into(),
        ))
    }

    async fn get_health_status(&self) -> DriverStatus {
        DriverStatus {
            is_ready: false,
            vram_usage_bytes: 0,
            active_models: vec![],
        }
    }

    async fn load_model(&mut self, path: &str) -> Result<(), SystemError> {
        info!(model_path = %path, "Mock loading GGUF model into Native Driver...");
        Ok(())
    }
}

// SAFETY: LlamaNativeDriver currently holds only `n_gpu_layers: u32` and
// `ctx_size: u32`, both of which are `Copy` integers with no interior mutability.
// These fields are unconditionally `Send`.
//
// When the `local_llm` feature is enabled and the driver is extended to wrap raw
// pointers to a llama.cpp model context (`llama_model*` and `llama_context*`),
// the following invariants MUST hold for this impl to remain sound:
//
// 1. **RwLock serialization at the HAL layer.** Every caller holds the driver
//    inside a `CognitiveHAL`, which is wrapped in an `Arc<RwLock<CognitiveHAL>>`
//    at the server layer. All inference calls must acquire the write lock before
//    reaching the driver, serializing access to the underlying llama.cpp context.
//    The llama.cpp C library is NOT thread-safe for a single context — concurrent
//    calls on the same `llama_context*` result in undefined behavior.
//
// 2. **Exclusive pointer ownership.** The driver must own the context exclusively.
//    There must be no mechanism to extract, clone, or alias the raw pointer from
//    outside this struct.
//
// 3. **Sound Drop.** The driver's `Drop` impl (or destructor logic) must call the
//    appropriate llama.cpp free functions to prevent use-after-free when the driver
//    is dropped across a thread boundary.
//
// If the locking discipline above is ever relaxed (e.g., to support parallel
// inference), this `unsafe impl` must be re-evaluated. Each concurrent inference
// path must use its own dedicated `llama_context*` — never share a single context
// across threads.
unsafe impl Send for LlamaNativeDriver {}

// SAFETY: See the `Send` impl above. The same invariants apply. `Sync` is sound
// because no `&self` method provides mutable access to any internal state without
// first going through the `RwLock<CognitiveHAL>` at the HAL layer. In the current
// stub (no raw pointers), all fields are plain integers which are trivially `Sync`.
unsafe impl Sync for LlamaNativeDriver {}

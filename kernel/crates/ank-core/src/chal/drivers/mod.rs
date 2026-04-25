pub mod cloud;
pub mod cloud_voice;
pub mod embeddings;
#[cfg(feature = "local_llm")]
pub mod native;
pub mod siren;

pub use cloud::CloudProxyDriver;
#[cfg(feature = "local_llm")]
pub use native::LlamaNativeDriver;
pub use siren::{ElevenLabsDriver, GroqSttEngine, VoxtralDriver, WhisperLocalEngine};

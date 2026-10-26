use anyhow::{Context, Result};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct PluginSigner {
    pub_key: VerifyingKey,
    skip_verify: bool,
}

impl PluginSigner {
    /// Inicializa el verificador con la llave pública de Aegis.
    /// En producción, esto vendría de una variable de entorno o almacén de llaves.
    pub fn new(public_key_bytes: &[u8; 32]) -> Result<Self> {
        let pub_key = VerifyingKey::from_bytes(public_key_bytes)
            .context("Invalid Ed25519 public key bytes")?;
        Ok(Self {
            pub_key,
            skip_verify: false,
        })
    }

    /// Inicializa el verificador en modo permisivo explícito (solo para dev/test cuando
    /// no existe llave raíz configurada o `AEGIS_ALLOW_INSECURE_PLUGINS=1`).
    pub fn new_permissive(public_key_bytes: &[u8; 32]) -> Result<Self> {
        let pub_key = VerifyingKey::from_bytes(public_key_bytes)
            .context("Invalid Ed25519 public key bytes")?;
        Ok(Self {
            pub_key,
            skip_verify: true,
        })
    }

    fn resolve_sig_path(wasm_path: &Path) -> PathBuf {
        let primary = wasm_path.with_extension("wasm.sig");
        if primary.exists() {
            return primary;
        }
        let mut fallback = wasm_path.as_os_str().to_os_string();
        fallback.push(".sig");
        let fallback_path = PathBuf::from(fallback);
        if fallback_path.exists() {
            return fallback_path;
        }
        primary
    }

    /// Verifica que un archivo .wasm tenga una firma .wasm.sig válida (binaria de 64 bytes o hex de 128 chars).
    pub fn verify_plugin<P: AsRef<Path>>(&self, wasm_path: P) -> Result<()> {
        let wasm_path = wasm_path.as_ref();
        let sig_path = Self::resolve_sig_path(wasm_path);

        if !sig_path.exists() {
            if self.skip_verify {
                tracing::warn!(
                    "PluginSigner permissive mode: loading unsigned plugin {:?}",
                    wasm_path
                );
                return Ok(());
            }
            return Err(anyhow::anyhow!("Missing signature file: {:?}", sig_path));
        }

        let wasm_bytes = fs::read(wasm_path)
            .with_context(|| format!("Failed to read wasm file: {:?}", wasm_path))?;
        let sig_bytes = fs::read(&sig_path)
            .with_context(|| format!("Failed to read signature file: {:?}", sig_path))?;

        let raw_sig = if sig_bytes.len() == 64 {
            sig_bytes
        } else {
            let trimmed = std::str::from_utf8(&sig_bytes)
                .ok()
                .map(str::trim)
                .unwrap_or("");
            hex::decode(trimmed)
                .context("Invalid signature format (expected 64 raw bytes or 128 hex chars)")?
        };

        let signature = Signature::from_slice(&raw_sig).context("Invalid signature format")?;

        self.pub_key
            .verify(&wasm_bytes, &signature)
            .context("Plugin signature verification failed (Ring 0 Violation)")?;

        Ok(())
    }
}

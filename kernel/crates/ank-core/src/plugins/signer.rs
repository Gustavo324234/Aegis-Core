use anyhow::Result;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::fs;
use std::path::Path;

pub struct PluginSigner {
    pub_key: VerifyingKey,
}

impl PluginSigner {
    /// Inicializa el verificador con la llave pública de Aegis.
    /// En producción, esto vendría de una variable de entorno o almacén de llaves.
    pub fn new(public_key_bytes: &[u8; 32]) -> Result<Self> {
        use anyhow::Context;
        let pub_key = VerifyingKey::from_bytes(public_key_bytes)
            .context("Invalid Ed25519 public key bytes")?;
        Ok(Self { pub_key })
    }

    /// Verifica que un archivo .wasm tenga una firma .wasm.sig válida.
    pub fn verify_plugin<P: AsRef<Path>>(&self, wasm_path: P) -> Result<()> {
        use anyhow::Context;
        let wasm_path = wasm_path.as_ref();
        let sig_path = wasm_path.with_extension("wasm.sig");

        if !sig_path.exists() {
            return Err(anyhow::anyhow!("Missing signature file: {:?}", sig_path));
        }

        let wasm_bytes = fs::read(wasm_path)
            .with_context(|| format!("Failed to read wasm file: {:?}", wasm_path))?;
        let sig_bytes = fs::read(&sig_path)
            .with_context(|| format!("Failed to read signature file: {:?}", sig_path))?;

        let signature = Signature::from_slice(&sig_bytes).context("Invalid signature format")?;

        self.pub_key
            .verify(&wasm_bytes, &signature)
            .context("Plugin signature verification failed (Ring 0 Violation)")?;

        Ok(())
    }
}

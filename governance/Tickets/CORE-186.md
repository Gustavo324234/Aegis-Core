# CORE-186 — feat: TTS engine local con espeak-ng — sintetizar voz sin API key

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / kernel/crates/ank-core  
**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Crítica  
**Asignado a:** Kernel Engineer  
**Dependencias:** ninguna (es el engine que usa CORE-185)

---

## Contexto

El único TTS engine implementado en el `SirenRouter` es `MockSirenEngine` (ruido blanco)
y `ElevenLabsDriver` (requiere API key). `WhisperLocalEngine` es STT-only y su
`synthesize()` retorna error explícito.

Para que el TTS funcione en producción sin API key externa, se necesita un engine
local. El servidor ya tiene `espeak-ng` disponible en Debian (o puede instalarse con
`apt install espeak-ng`). Es liviano, corre en CPU, soporta español e inglés, y
produce PCM 16-bit mono directamente.

---

## Engine elegido: `espeak-ng`

**Por qué espeak-ng sobre otras opciones:**

| Engine | Sin API key | CPU-only | Calidad | En Debian | PCM directo |
|--------|-------------|----------|---------|-----------|-------------|
| espeak-ng | ✅ | ✅ | Básica | ✅ apt | ✅ |
| piper-tts | ✅ | ✅ | Buena | Manual | ✅ |
| coqui | ✅ | ✅ | Buena | Manual | ✅ |

Espeak-ng es el camino más rápido para tener voz funcional en producción hoy.
Piper-tts puede ser el upgrade futuro (CORE-190) con mejor calidad.

**Verificar disponibilidad en el servidor:**
```bash
which espeak-ng || apt-cache show espeak-ng
```

---

## Cambios Requeridos

### Nuevo archivo: `kernel/crates/ank-core/src/chal/drivers/siren/espeak.rs`

```rust
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tracing::{info, warn};

use crate::router::siren::SirenEngine;

/// EspeakEngine: TTS local usando espeak-ng CLI.
/// STT no soportado — este engine es solo TTS.
/// Requiere: apt install espeak-ng
pub struct EspeakEngine {
    voice: String,        // e.g. "es" para español, "en" para inglés
    speed: u32,           // palabras por minuto, default 150
    sample_rate: u32,     // Hz, default 22050
}

impl EspeakEngine {
    /// Constructor con parámetros configurables.
    pub fn new(voice: String, speed: u32, sample_rate: u32) -> Self {
        Self { voice, speed, sample_rate }
    }

    /// Constructor con defaults razonables para Aegis.
    pub fn default_es() -> Self {
        Self {
            voice: "es".to_string(),
            speed: 150,
            sample_rate: 22050,
        }
    }

    pub fn default_en() -> Self {
        Self {
            voice: "en".to_string(),
            speed: 150,
            sample_rate: 22050,
        }
    }

    /// Verifica si espeak-ng está instalado en el sistema.
    pub fn is_available() -> bool {
        std::process::Command::new("which")
            .arg("espeak-ng")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[async_trait]
impl SirenEngine for EspeakEngine {
    fn id(&self) -> &str {
        "espeak"
    }

    /// Sintetiza texto a PCM 16-bit mono usando espeak-ng CLI.
    async fn synthesize(&self, text: String) -> Result<Vec<u8>> {
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Limpiar el texto de markdown que no suena bien en TTS
        let clean_text = clean_for_tts(&text);

        // espeak-ng -v es -s 150 --stdout -b 1 --rate=22050 "texto"
        // --stdout: output PCM raw a stdout
        // -b 1: encoding 16-bit little-endian
        // No hay flag directo para sample rate en espeak-ng CLI;
        // el sample rate nativo es 22050Hz para espeak-ng.
        let output = tokio::process::Command::new("espeak-ng")
            .args([
                "-v", &self.voice,
                "-s", &self.speed.to_string(),
                "--stdout",
                "-b", "1",  // 16-bit little-endian
                &clean_text,
            ])
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    anyhow!("espeak-ng no encontrado. Instalá con: apt install espeak-ng")
                } else {
                    anyhow!("Error ejecutando espeak-ng: {}", e)
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("espeak-ng falló: {}", stderr.trim()));
        }

        // stdout contiene PCM raw 16-bit little-endian mono a 22050Hz
        // con un header WAV de 44 bytes al inicio — removerlo si está presente
        let pcm = strip_wav_header_if_present(output.stdout);

        info!(
            bytes = pcm.len(),
            voice = %self.voice,
            "EspeakEngine: síntesis completada."
        );

        Ok(pcm)
    }

    /// STT no soportado por este engine.
    async fn transcribe(&self, _audio: Vec<u8>) -> Result<String> {
        Err(anyhow!("EspeakEngine: STT no soportado. Usá WhisperLocalEngine o GroqSttEngine."))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Limpia texto markdown para que suene bien en TTS.
/// Elimina: **, *, __, #, `, URLs, emojis comunes.
fn clean_for_tts(text: &str) -> String {
    let mut result = text.to_string();

    // Remover bloques de código completos
    while let Some(start) = result.find("```") {
        if let Some(end) = result[start + 3..].find("```") {
            result.replace_range(start..start + 3 + end + 3, "");
        } else {
            break;
        }
    }

    // Remover markdown inline
    result = result
        .replace("**", "")
        .replace('*', "")
        .replace("__", "")
        .replace('`', "");

    // Remover headers markdown
    let lines: Vec<&str> = result.lines().collect();
    let cleaned: Vec<&str> = lines
        .iter()
        .map(|l| l.trim_start_matches('#').trim())
        .copied()
        .collect();
    result = cleaned.join(". ");

    // Remover URLs
    // Simple heurística: eliminar tokens que empiezan con http
    result = result
        .split_whitespace()
        .filter(|w| !w.starts_with("http://") && !w.starts_with("https://"))
        .collect::<Vec<_>>()
        .join(" ");

    result.trim().to_string()
}

/// espeak-ng con --stdout puede incluir header WAV dependiendo de la versión.
/// Si los primeros 4 bytes son "RIFF", saltear el header estándar de 44 bytes.
fn strip_wav_header_if_present(data: Vec<u8>) -> Vec<u8> {
    if data.len() > 44 && data.starts_with(b"RIFF") {
        data[44..].to_vec()
    } else {
        data
    }
}
```

### `kernel/crates/ank-core/src/chal/drivers/siren/mod.rs`

Agregar el nuevo módulo:

```rust
pub mod espeak;
pub use espeak::EspeakEngine;
```

### `kernel/crates/ank-core/src/chal/drivers/mod.rs`

Agregar al re-export público:

```rust
pub use siren::{ElevenLabsDriver, EspeakEngine, GroqSttEngine, VoxtralDriver, WhisperLocalEngine};
```

### `kernel/crates/ank-core/src/router/siren.rs` — auto-registrar EspeakEngine

En `SirenRouter::new()`, después de registrar WhisperLocalEngine:

```rust
// Auto-Register EspeakEngine si espeak-ng está disponible
if crate::chal::drivers::EspeakEngine::is_available() {
    let voice = std::env::var("AEGIS_TTS_VOICE").unwrap_or_else(|_| "es".to_string());
    let speed = std::env::var("AEGIS_TTS_SPEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(150u32);
    let engine = crate::chal::drivers::EspeakEngine::new(voice, speed, 22050);
    engines.insert("espeak".to_string(), Arc::new(engine));
    info!("SirenRouter: EspeakEngine registered (espeak-ng found in PATH).");
}
```

### `kernel/crates/ank-core/src/router/siren.rs` — fallback a espeak en `resolve()`

En la cadena de fallback de `resolve()`, agregar espeak antes del mock:

```rust
// 2. Fallback: Voxtral si está registrado
if let Some(voxtral) = engines.get("voxtral") {
    return Ok(voxtral.clone());
}

// 3. Fallback: EspeakEngine (local, sin API key)
if let Some(espeak) = engines.get("espeak") {
    return Ok(espeak.clone());
}

// 4. Última instancia: Mock
engines
    .get("mock")
    .cloned()
    .ok_or_else(|| anyhow::anyhow!("SirenRouter: No default 'mock' engine registered"))
```

---

## Variables de entorno (documentar en `.env.example`)

```bash
# TTS — EspeakEngine
AEGIS_TTS_VOICE=es          # Idioma: "es" (español), "en" (inglés), "es-la" (latinoam.)
AEGIS_TTS_SPEED=150         # Palabras por minuto (100-300, default 150)
```

---

## Criterios de Aceptación

- [ ] `EspeakEngine::is_available()` retorna `true` si `espeak-ng` está en PATH
- [ ] `EspeakEngine::synthesize("Hola mundo")` retorna `Vec<u8>` no vacío con PCM válido
- [ ] `EspeakEngine::synthesize("")` retorna `Vec<u8>` vacío sin error
- [ ] `EspeakEngine` aparece en el `SirenRouter` como engine registrado cuando espeak-ng está disponible
- [ ] El fallback en `resolve()` pasa por espeak antes del mock
- [ ] Test unitario: `test_espeak_synthesize` — verifica que el output no está vacío (skipear si espeak-ng no está en PATH con `#[ignore]`)
- [ ] `AEGIS_TTS_VOICE` y `AEGIS_TTS_SPEED` se leen correctamente del entorno
- [ ] `cargo build` sin warnings (`-D warnings`)
- [ ] No usar `unwrap()` ni `expect()` en código nuevo

---

## Instalación en servidor (instrucciones para Tavo)

Después de hacer merge del PR, ejecutar en el servidor:

```bash
sudo apt install -y espeak-ng
# Verificar:
espeak-ng -v es "Aegis está activo" --stdout | aplay -r 22050 -f S16_LE -c 1
```

Luego reiniciar el servicio:
```bash
sudo systemctl restart aegis.service
```

---

## Commit message

```
feat(ank-core): CORE-186 EspeakEngine — TTS local con espeak-ng sin API key
```

---

*Arquitecto IA — 2026-04-25*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: EspeakEngine implementado y registrado en SirenRouter::new (chal/drivers, detección espeak-ng en PATH, AEGIS_TTS_VOICE/SPEED).

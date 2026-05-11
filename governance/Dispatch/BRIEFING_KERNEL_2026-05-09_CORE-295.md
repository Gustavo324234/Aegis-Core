# BRIEFING — Kernel Engineer
## CORE-295 (Parte 1): SirenRouter — fallback a key del admin
**Fecha:** 2026-05-09
**Branch:** `fix/core-295-siren-admin-key-fallback`

---

## Contexto

El tenant tiene ElevenLabs configurado pero su `api_key` puede estar vacía
(guardada mal desde el panel). En ese caso el SirenRouter cae directo al Mock
que genera ruido PCM triangular — suena metálico. No hay fallback a la key
que el admin configuró en el perfil de `root`.

## Cambio — `kernel/crates/ank-core/src/router/siren.rs`

En la función `resolve()`, localizar este bloque existente:

```rust
warn!("SirenRouter: ElevenLabs selected but no valid api_key in profile. Falling back.");
```

Insertar ANTES del `// 2. Fallback: Voxtral` que sigue inmediatamente después:

```rust
// Fallback: intentar con la key global del perfil de root/admin
if profile.engine_id == "elevenlabs" {
    if let Ok(Some(admin_profile)) = self.persistence.get_voice_profile("root").await {
        if admin_profile.engine_id == "elevenlabs" {
            if let Ok(settings) =
                serde_json::from_str::<serde_json::Value>(&admin_profile.settings_json)
            {
                if let Some(api_key) = settings["api_key"].as_str() {
                    if !api_key.is_empty() {
                        let voice = if profile.voice_id.is_empty() {
                            admin_profile.voice_id.clone()
                        } else {
                            profile.voice_id.clone()
                        };
                        match crate::chal::drivers::ElevenLabsDriver::new(
                            api_key.to_string(),
                            voice,
                        ) {
                            Ok(driver) => {
                                info!(
                                    "SirenRouter: Using admin ElevenLabs key for tenant '{}'",
                                    tenant_id
                                );
                                return Ok(Arc::new(driver));
                            }
                            Err(e) => warn!("SirenRouter: Admin ElevenLabsDriver failed: {}", e),
                        }
                    }
                }
            }
        }
    }
}
```

## Criterios de aceptación

- [ ] `cargo build --workspace` pasa
- [ ] Tenant con `engine_id = "elevenlabs"` y `api_key` vacía resuelve con la key del admin
- [ ] Tenant con su propia key la sigue usando (sin cambios en ese path)

## Commit

```
fix(ank-core): CORE-295 SirenRouter fallback to admin ElevenLabs key when tenant key is empty
```

**No correr tests. No pushear a main. Abrir PR.**

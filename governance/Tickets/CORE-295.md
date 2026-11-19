# CORE-295 — Voice: fix voz metálica (Mock fallback) + key admin como fallback TTS

## Metadata
- **Epic:** 52 — Voice Quality
- **Tipo:** fix
- **Prioridad:** Crítica
- **Estado:✅ Done

---

## Problema 1 — Kernel: `SirenRouter.resolve()` cae al Mock cuando tenant no tiene key

**Archivo:** `kernel/crates/ank-core/src/router/siren.rs`

El tenant tiene `engine_id = "elevenlabs"` pero si su `api_key` en `settings_json`
está vacía, el SirenRouter cae al Mock (ruido triangular PCM — suena metálico/robótico).

No existe fallback a la key del admin/root.

**Fix — agregar fallback a perfil de `root` antes de caer al Mock:**

En `resolve()`, después del bloque que loguea
`"ElevenLabs selected but no valid api_key in profile. Falling back."`,
agregar antes del fallback a Voxtral:

```rust
// Fallback: intentar con la key global configurada en el perfil de root/admin
if profile.engine_id == "elevenlabs" {
    if let Ok(Some(admin_profile)) = self.persistence.get_voice_profile("root").await {
        if admin_profile.engine_id == "elevenlabs" {
            if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&admin_profile.settings_json) {
                if let Some(api_key) = settings["api_key"].as_str() {
                    if !api_key.is_empty() {
                        if let Ok(driver) = crate::chal::drivers::ElevenLabsDriver::new(
                            api_key.to_string(),
                            // Usar la voice_id del tenant si la tiene, si no la del admin
                            if profile.voice_id.is_empty() { admin_profile.voice_id.clone() }
                            else { profile.voice_id.clone() },
                        ) {
                            info!("SirenRouter: Using admin ElevenLabs key for tenant '{}'", tenant_id);
                            return Ok(Arc::new(driver));
                        }
                    }
                }
            }
        }
    }
}
```

**Asignado a:** Kernel Engineer

---

## Problema 2 — Shell: `SettingsPanel/VozTab` borra `stt_provider` al guardar

**Archivo:** `shell/ui/src/components/SettingsPanel.tsx` — función `handleSave` del `VozTab`

El body que manda al guardar es:
```js
{ provider, api_key: apiKey, voice_id: voiceId }
```

Omite `stt_provider` y `stt_api_key`. El kernel los recibe vacíos y los sobreescribe
con `""`, borrando la configuración de STT del tenant.

**Fix — leer el config actual antes de guardar y preservar los campos STT:**

En `VozTab`, agregar `sttProvider` y `sttApiKey` al estado local, cargarlos en
`fetchConfig()`, e incluirlos en el body del `handleSave`:

```ts
// Estado adicional
const [sttProvider, setSttProvider] = useState('browser');
const [sttApiKey, setSttApiKey] = useState('');

// En fetchConfig:
setSttProvider(data.stt_provider || 'browser');
setSttApiKey(data.stt_api_key || '');

// En handleSave body:
JSON.stringify({
    provider,
    api_key: apiKey,
    voice_id: voiceId,
    stt_provider: sttProvider,   // ← preservar
    stt_api_key: sttApiKey,      // ← preservar
})
```

**Asignado a:** Shell Engineer

---

## Problema 3 — Shell: `SirenConfigTab` (admin) no guarda `stt_provider`/`stt_api_key`

**Archivo:** `shell/ui/src/components/SirenConfigTab.tsx`

El panel de admin también omite estos campos al guardar. Mismo fix que Problema 2
pero en el componente de admin.

Verificar el body que manda `SirenConfigTab.handleSave` e incluir los campos STT
que ya tiene cargados en su estado.

**Asignado a:** Shell Engineer

---

## Criterios de aceptación

- [x] Tenant con `engine_id = "elevenlabs"` y `api_key` vacía usa la key del admin (`root`) automáticamente
- [x] Guardar desde `SettingsPanel/VozTab` no borra el `stt_provider` existente
- [x] Guardar desde `SirenConfigTab` (admin) no borra el `stt_provider` existente
- [x] `cargo build --workspace` pasa
- [x] `npm run build` pasa

## Dependencias

- Ninguna

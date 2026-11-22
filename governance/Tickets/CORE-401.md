# CORE-401: Supply Chain y Secretos (`runtime_deps.rs`, `whatsapp_bridge`, permisos `0600`, `deny.toml`)

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 61
- **Asignado a:** Kernel Engineer + DevOps Engineer

---

## Description

Fortalecer la cadena de suministro y el almacenamiento de secretos en disco:
- `ank-core/src/video_studio/runtime_deps.rs`: verificar SHA-256 al descargar binarios FFmpeg y fijar versiones/hashes en `pip install` para `edge-tts` y `pillow`.
- `tools/whatsapp_bridge/index.mjs`: pasar a modo fail-closed cuando `WA_BRIDGE_SECRET` no esté definido, eliminar el registro duplicado de `contacts.update` y evaluar cifrado/protección de sesiones.
- `ank-server/src/main.rs`, `ank-core/src/whatsapp/mod.rs` y `ank-http/src/routes/workspace.rs`: escribir llaves privadas y secretos (`plugin_signer.key`, `.bridge_secret`, `aegis_connect.json`) con permisos restrictivos (`0600` en Unix).
- `deny.toml` y `Cargo.toml`: auditar y reducir los 41 advisories RUSTSEC ignorados (actualizando dependencias como `webrtc` / `rustls-webpki` / `ring` donde haya upgrade viable y documentando cada excepción remanente).

## Acceptance Criteria

- [x] Descargas de FFmpeg y paquetes Python validan integridad por hash/versión pineada.
- [x] `whatsapp_bridge` rechaza tráfico HTTP cuando falta `WA_BRIDGE_SECRET`.
- [x] Archivos de secretos se crean con permisos restrictivos y `deny.toml` está auditado y documentado.

## Implementation & Verification

- **Secretos en disco (`0600` Unix):** agregado `ank_core::enclave::write_secret_file` y aplicado a `plugin_signer.key` (`ank-server/src/main.rs`), `.bridge_secret` (`ank-core/src/whatsapp/mod.rs`) y `aegis_connect.json` (`ank-http/src/routes/workspace.rs`).
- **Video Studio (`runtime_deps.rs`):** fijados `edge-tts==7.3.1` y `pillow==12.2.0`; agregada verificación SHA-256 obligatoria fail-closed antes de extraer FFmpeg (`parse_sha256_checksum`, `verify_file_sha256`, manifiestos gyan.dev / BtbN `checksums.sha256` y override `AEGIS_FFMPEG_SHA256`).
- **WhatsApp Bridge (`tools/whatsapp_bridge/index.mjs`):** `constantTimeBearerCheck` falla cerrado (`false`) sin `WA_BRIDGE_SECRET` + log `[SECURITY]`, eliminado listener duplicado `contacts.update`, y endurecidos directorios/archivos de sesión (`0700` dirs, `0600` `store.json` y `creds.json` para preservar compatibilidad con `useMultiFileAuthState` de Baileys).
- **Supply chain (`Cargo.lock` + `deny.toml`):** actualizados `cap-primitives` (`3.4.6`), `libz-sys` (`1.1.33`), `wasmtime`/`wasmtime-wasi` (`36.0.23`), `spin` (`0.9.9`), `url` (`2.5.9`), `form_urlencoded` (`1.2.3`), `h2` (`0.4.20`), `zstd`/`zstd-safe`/`zstd-sys`; reducidos y documentados todos los `RUSTSEC-*` restantes (de 41 a 29 excepciones activas justificadas, 0 `advisory-not-detected`).
- **Verificación:** `cargo fmt --all -- --check`, `cargo clippy --workspace` (CI flags), `cargo deny check advisories` (`advisories ok`), `node --check tools/whatsapp_bridge/index.mjs`, `cargo test -p ank-core --lib enclave` (`16 passed`) y `video_studio::runtime_deps` (`3 passed`).

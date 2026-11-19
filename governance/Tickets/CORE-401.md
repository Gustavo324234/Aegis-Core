# CORE-401: Supply Chain y Secretos (`runtime_deps.rs`, `whatsapp_bridge`, permisos `0600`, `deny.toml`)

- **Tipo:** fix
- **Estado:** 📥 Todo
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

- [ ] Descargas de FFmpeg y paquetes Python validan integridad por hash/versión pineada.
- [ ] `whatsapp_bridge` rechaza tráfico HTTP cuando falta `WA_BRIDGE_SECRET`.
- [ ] Archivos de secretos se crean con permisos restrictivos y `deny.toml` está auditado y documentado.

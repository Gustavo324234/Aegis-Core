# CORE-108 — Indicador en UI cuando STT (Whisper) no está disponible

**Epic:** 35 — Hardening Post-Launch  
**Área:** `shell/ui/` + `kernel/crates/ank-http/src/routes/`  
**Agente:** Shell Engineer (UI) + Kernel Engineer (endpoint)  
**Prioridad:** P3 — Experiencia de usuario  
**Estado:** TODO  
**Origen:** REC-016 / claude-sonnet-4-6 sección 3.4

---

## Contexto

Si `ggml-base.bin` (modelo Whisper para STT) no está presente en el directorio de
datos, el STT falla silenciosamente. El test lo detecta con `graceful skip` pero
el usuario no recibe ningún indicador en la UI. El botón de micrófono aparece activo
aunque la funcionalidad esté inoperativa.

---

## Cambios requeridos

### Kernel Engineer

1. Agregar campo `stt_available: bool` a la respuesta de `GET /api/siren/config`
   o a `GET /api/status`. El valor es `true` solo si el archivo del modelo STT
   existe en `data_dir` y es legible:

   ```rust
   let stt_available = data_dir.join("models").join("ggml-base.bin").exists();
   ```

2. El chequeo puede hacerse al recibir la request (no requiere estado en memoria).

### Shell Engineer

3. En el store Zustand que gestiona el estado de Siren, consumir el campo
   `stt_available` del endpoint.

4. En el componente `VoiceButton` (o el que corresponda):
   - Mostrar tooltip `"Micrófono no disponible — modelo de voz no instalado"`
     cuando `stt_available === false`
   - El botón debe estar visualmente deshabilitado o con badge de advertencia
   - No bloquear el TTS si este sí está disponible

5. En la sección de configuración de Siren, mostrar el estado del modelo STT
   con instrucciones de descarga si no está disponible.

---

## Criterios de aceptación

- [ ] `GET /api/status` o `GET /api/siren/config` incluye `stt_available: bool`
- [ ] Si `ggml-base.bin` no existe, `stt_available` es `false`
- [ ] El `VoiceButton` muestra indicador visual cuando `stt_available === false`
- [ ] `cargo build -p ank-http` sin errores ni warnings de clippy
- [ ] `npm run build` en `shell/ui` sin errores TypeScript
- [ ] Sin `.unwrap()` ni `.expect()` en código nuevo

---

## Dependencias

Ninguna.

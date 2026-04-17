# CORE-120 — Indicador en UI cuando STT (Whisper) no está disponible

**Epic:** Epic 35 — Hardening Pre-Launch
**Agente:** Shell Engineer
**Prioridad:** 🟢 BAJA — UX
**Estado:** TODO
**Origen:** REC-016 / Auditoría multi-modelo 2026-04-16

---

## Contexto

Cuando `ggml-base.bin` (modelo Whisper para STT) no está presente en el sistema,
el feature de voz falla silenciosamente. El test de Siren salta con un "graceful
skip" pero el usuario no recibe ningún indicador en la UI de que el botón de
micrófono está inoperativo.

El usuario activa el micrófono y no recibe respuesta ni error — la experiencia
es confusa, especialmente en instalaciones nuevas donde el modelo local no fue
descargado.

## Cambios requeridos

### Paso 1 — Exponer estado de modelos locales en el kernel

**Archivo:** `kernel/crates/ank-http/src/routes/` (handler de `/api/status` o nuevo `/api/siren/status`)

Agregar al response de `/api/siren/config` o como campo nuevo en `/api/status`:

```json
{
  "siren": {
    "stt_available": false,
    "stt_reason": "ggml-base.bin not found at /var/lib/aegis/models/ggml-base.bin",
    "tts_available": true,
    "tts_engine": "mock"
  }
}
```

En el kernel, verificar la existencia del archivo al arranque o al primer request:

```rust
// En SirenRouter o al inicializar el servicio Siren
pub fn check_stt_availability(data_dir: &Path) -> (bool, Option<String>) {
    let model_path = data_dir.join("models").join("ggml-base.bin");
    if model_path.exists() {
        (true, None)
    } else {
        (false, Some(format!("ggml-base.bin not found at {}", model_path.display())))
    }
}
```

### Paso 2 — Indicador visual en la UI

**Archivo:** `shell/ui/src/` — componente que contiene el botón de micrófono (VoiceButton o similar)

```tsx
// En VoiceButton.tsx o donde esté el control de micrófono
const { sirenStatus } = useAegisStore();

return (
  <div className="relative">
    <button
      onClick={handleMicClick}
      disabled={!sirenStatus?.stt_available}
      className={`mic-button ${!sirenStatus?.stt_available ? 'opacity-50 cursor-not-allowed' : ''}`}
      title={sirenStatus?.stt_available ? 'Activar micrófono' : sirenStatus?.stt_reason}
    >
      <MicIcon />
    </button>
    {!sirenStatus?.stt_available && (
      <Tooltip content={`STT no disponible: ${sirenStatus?.stt_reason}`}>
        <WarningIcon className="absolute -top-1 -right-1 text-yellow-500 w-3 h-3" />
      </Tooltip>
    )}
  </div>
);
```

### Paso 3 — Fetch del estado Siren al iniciar sesión

Agregar al store `useAegisStore` un campo `sirenStatus` que se populee junto
con el resto del estado de telemetría al hacer login.

## Criterios de aceptación

- [ ] Cuando `ggml-base.bin` no está presente, el botón de micrófono muestra
  un indicador visual (icono de advertencia, tooltip, o estado deshabilitado)
- [ ] El tooltip explica brevemente por qué STT no está disponible
- [ ] Cuando el modelo sí está presente, el comportamiento es idéntico al actual
- [ ] `cargo build` (kernel) pasa sin errores
- [ ] `npm run build` (shell/ui) pasa sin errores de TypeScript

## Dependencias

- El kernel debe exponer el estado de disponibilidad de Siren (Paso 1)
- El Shell Engineer implementa el Paso 2+3 una vez que el endpoint existe
- No bloquea el lanzamiento pero mejora la experiencia del primer usuario

# BRIEFING — Shell Engineer
## CORE-295 (Parte 2): VozTab + SirenConfigTab — preservar stt_provider al guardar
**Fecha:** 2026-05-09
**Branch:** `fix/core-295-voice-settings-stt-preserve`

---

## Contexto

Dos componentes guardan en `POST /api/siren/config` pero omiten `stt_provider`
y `stt_api_key` en el body. El kernel los recibe vacíos y sobreescribe la
configuración de STT con `""`. Resultado: cada vez que el usuario toca el
panel de voz, pierde su configuración de STT.

---

## Fix 1 — `shell/ui/src/components/SettingsPanel.tsx` → `VozTab`

### Estado — agregar dos campos:
```ts
const [sttProvider, setSttProvider] = useState('browser');
const [sttApiKey, setSttApiKey] = useState('');
```

### `fetchConfig` — cargar los campos STT:
```ts
// Después de setProvider / setApiKey / setVoiceId:
setSttProvider(data.stt_provider || 'browser');
setSttApiKey(data.stt_api_key || '');
```

El endpoint `GET /api/siren/config` ya devuelve `stt_provider` y `stt_api_key`.

### `handleSave` — incluirlos en el body:
```ts
body: JSON.stringify({
    provider,
    api_key: apiKey,
    voice_id: voiceId,
    stt_provider: sttProvider,   // ← agregar
    stt_api_key: sttApiKey,      // ← agregar
}),
```

---

## Fix 2 — `shell/ui/src/components/SirenConfigTab.tsx`

Mismo patrón. Leer el archivo, verificar qué campos tiene en estado y cuáles
incluye en el body del save. Agregar `stt_provider` y `stt_api_key` si no
están incluidos.

---

## Criterios de aceptación

- [ ] Guardar desde `SettingsPanel → Voz` no borra `stt_provider` existente
- [ ] Guardar desde el panel de admin (SirenConfigTab) no borra `stt_provider`
- [ ] `npm run build` pasa sin errores

## Commit

```
fix(ui): CORE-295 preserve stt_provider and stt_api_key when saving voice config
```

**No pushear a main. Abrir PR.**

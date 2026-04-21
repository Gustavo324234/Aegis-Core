# CORE-133 — Feature: Settings Panel expandido en ChatTerminal

**Epic:** 38 — Agent Persona System
**Repo:** Aegis-Core — `shell/`
**Path:** `shell/ui/src/components/ChatTerminal.tsx`
**Tipo:** feat
**Prioridad:** Media
**Asignado a:** Shell Engineer (Antigravity)
**Depende de:** CORE-129 (endpoint `/api/persona`)

---

## Contexto

El modal de Settings en `ChatTerminal` hoy solo tiene `TenantKeyManager` y
`UserPasswordChange`. El usuario no puede acceder desde ahí a:
- Editar la Persona del agente
- Configurar el motor de inferencia
- Configurar Siren (voz)
- Cambiar el idioma con un selector decente (hoy es un botón críptico hardcodeado)

Este ticket reemplaza el modal por un panel de settings con tabs internas,
sin requerir ir al Admin Dashboard. El tenant corriente debe poder configurar
todo lo suyo desde el chat.

---

## Cambios requeridos

### 1. Reemplazar el modal actual por `SettingsPanel`

Crear `shell/ui/src/components/SettingsPanel.tsx` con 4 tabs internas:

```
Persona | Motor | Voz | Seguridad
```

El componente recibe `onClose: () => void`, `tenantId: string`, `sessionKey: string`.

**Headers Citadel para todas las llamadas:**
```ts
const citadelHeaders = {
  'Content-Type': 'application/json',
  'x-citadel-tenant': tenantId,
  'x-citadel-key': sessionKey,
};
```

---

### Tab 1 — Persona

Reusar el componente `PersonaTab` de CORE-130 con adaptación para este contexto
(sin los glassmorphism extremos, más compacto).

Funcionalidad idéntica a CORE-130: textarea con contador, save/reset, preview.

---

### Tab 2 — Motor

Muestra la configuración actual del motor y permite cambiarla.

```tsx
// Estado inicial: GET /api/engine/status (headers Citadel)
// Guardar: POST /api/engine/configure (body: { api_url, model_name, api_key, provider })

interface EngineConfig {
  provider: string;  // openai | anthropic | groq | openrouter | ollama | custom
  api_url: string;
  model_name: string;
  api_key: string;   // oscurecido con type="password"
}
```

UI:
- Selector de provider (dropdown con los providers conocidos)
- Al seleccionar provider, pre-fill `api_url` con el valor por defecto
- Campo `model_name` (texto libre o dropdown si hay modelos disponibles)
- Campo `api_key` (password input)
- Botón "Guardar configuración"
- Indicador de estado actual (proveedor + modelo activo, leído de `GET /api/engine/status`)

---

### Tab 3 — Voz

Muestra el estado de Siren y permite configurar el motor de TTS.

```tsx
// Estado inicial: GET /api/siren/config (headers Citadel)
// Guardar: POST /api/siren/config (body: { provider, api_key, voice_id })

// Indicadores de estado:
// - STT disponible (stt_available del response)
// - Motor TTS actual (provider + voice_id)
// - Nota explicativa si el micrófono está bloqueado por HTTP
```

**Nota importante para UI:** Si `window.location.protocol === 'http:'` y el
hostname no es `localhost` ni `127.0.0.1`, mostrar un banner de advertencia:

```
⚠️ El micrófono requiere HTTPS para funcionar desde otros dispositivos.
Accedé a Aegis via https:// para habilitar la voz.
```

Campos configurables:
- Provider TTS (voxtral / mock / elevenlabs)
- API Key del provider (si aplica)
- Voice ID (dropdown con voces disponibles de `GET /api/siren/voices`)
- Botón "Guardar"

---

### Tab 4 — Seguridad

Contenido actual del modal existente, sin cambios:
- `TenantKeyManager` (claves API del tenant)
- `UserPasswordChange` (cambio de contraseña)
- Selector de idioma (dropdown `ES / EN` en lugar del botón toggle actual)

```tsx
// Selector de idioma:
<select
  value={currentLang}
  onChange={(e) => {
    localStorage.setItem('aegis_language', e.target.value);
    window.location.reload();
  }}
  className="..."
>
  <option value="es">Español</option>
  <option value="en">English</option>
</select>
```

---

### 2. Actualizar `ChatTerminal.tsx`

Reemplazar el bloque del modal existente por el nuevo `SettingsPanel`:

```tsx
import SettingsPanel from './SettingsPanel';

// En el JSX, reemplazar el AnimatePresence del modal:
{showSettings && (
  <SettingsPanel
    tenantId={tenantId!}
    sessionKey={sessionKey!}
    onClose={() => setShowSettings(false)}
  />
)}
```

---

## Criterios de aceptación

- [ ] `npm run build` sin errores TypeScript ni ESLint
- [ ] Botón Settings en ChatTerminal abre el nuevo SettingsPanel
- [ ] Tab Persona: edición funcional (GET/POST/DELETE `/api/persona` con headers Citadel)
- [ ] Tab Motor: muestra configuración actual y permite cambiarla
- [ ] Tab Voz: muestra estado de Siren + banner de advertencia HTTP si aplica
- [ ] Tab Seguridad: incluye selector de idioma dropdown EN/ES funcional
- [ ] Todas las llamadas usan headers `x-citadel-tenant`/`x-citadel-key` (nunca query params)
- [ ] El panel es responsive y funciona en móvil

---

## Dependencias

- CORE-129 (endpoint `/api/persona`)
- CORE-130 (`PersonaTab` reutilizable)

---

## Commit message

```
feat(shell): CORE-133 expanded settings panel — persona, motor, voz, seguridad
```

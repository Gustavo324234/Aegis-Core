# CORE-377: Zero-Friction Personal Onboarding Wizard (1-Step Local Setup) en `shell/ui` y `ank-http`

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 58
- **Asignado a:** Shell Engineer + Kernel Engineer

---

## Description
> **Nota de trazabilidad (`CORE-396`):** Este ticket pertenece formalmente a `EPIC 58`. La especificación histórica de Fase 5 (Integración MCP, commit `77e6554`) queda canonizada en [`CORE-377-p5.md`](file:///e:/Aegis/Aegis-Core/governance/Tickets/CORE-377-p5.md).

Actualmente, cuando un usuario instala Aegis por primera vez y abre `http://localhost:8000` sin el query param `?setup_token=...`, se encuentra con una pantalla roja bloqueante en `shell/ui/src/App.tsx` (líneas 165-176: *"Ring 0 Identity Required - Administrator identity not found. Verify your Setup Token"*), y luego debe atravesar 6 pasos separados (crear Admin, crear Tenant, cerrar sesión, loguearse como Tenant, cambiar contraseña forzada y configurar el motor) antes de poder enviar su primer mensaje.

## Archivos a Modificar
1. **Backend (`kernel/crates/ank-http/src/routes/auth.rs`):**
   - Agregar ruta `POST /api/auth/quick-personal-setup` con request struct:
     ```rust
     #[derive(Deserialize, ToSchema)]
     pub struct QuickPersonalSetupRequest {
         pub username: String,       // ej. "alex" (se usa como tenant personal)
         pub passphrase: String,     // mín. 8 caracteres
         pub provider: String,       // "openrouter" | "gemini" | "ollama" | "openai" | "anthropic"
         pub api_key: Option<String>,
         pub model_name: Option<String>,
         pub setup_token: Option<String>, // Opcional si IpAddr::is_loopback() y !enclave.admin_exists()
     }
     ```
   - **Seguridad:** Permitir omitir `setup_token` **únicamente** cuando `!enclave.admin_exists()` **Y** `ip.is_loopback()` (`127.0.0.1` o `::1`). Si la conexión viene de una IP externa (no loopback), exigir `setup_token` válido.
   - **Acción atómica en 1 paso:**
     1. Inicializar `admin` maestro con la misma passphrase (o hash derivado) y crear el tenant personal `username` con `must_change_password = false`.
     2. Si se proveyó `api_key` (o `provider == "ollama"`), registrar la key en `state.router` (`KeyPool`) y guardar la configuración del motor (`engine_config.json`), marcando el motor como configurado.
     3. Retornar `{ status: "READY", tenant_id: username, session_key_hash }`.

2. **Frontend (`shell/ui/src/App.tsx` líneas 165-176 y `shell/ui/src/components/PersonalQuickSetup.tsx`):**
   - Cuando `systemState === 'STATE_INITIALIZING'`, en lugar de renderizar el cartel rojo *"Ring 0 Identity Required"*, renderizar el nuevo componente `PersonalQuickSetup.tsx` (si hay `setupToken` en la URL o el usuario está en `localhost`).
   - El formulario pide en **una sola tarjeta limpia**:
     - **Tu Nombre / Usuario** (ej. `alex`)
     - **Tu Contraseña** (mín. 8 caracteres)
     - **Proveedor de IA**: selector rápido (`Google Gemini (Gratis)`, `OpenRouter`, `Ollama Local (Sin API Key)`, `OpenAI`, `Anthropic`) + campo de API Key.
   - Al enviar a `POST /api/auth/quick-personal-setup`, autentica automáticamente al usuario en `useAegisStore` (`tenantId`, `sessionKey`, `isAuthenticated: true`, `isEngineConfigured: true`) y abre directo el `ChatTerminal`.

## Acceptance Criteria
- [x] Abrir `http://localhost:8000` en una instalación limpia muestra el asistente de bienvenida de 1 paso en vez del error rojo *"Ring 0 Identity Required"*.
- [x] Completar Nombre + Contraseña + API Key (u Ollama) lleva al usuario directamente al chat funcional en menos de 60 segundos sin pedir relogin ni cambio forzado de contraseña.
- [x] Las peticiones a `/api/auth/quick-personal-setup` desde IPs no-loopback sin `setup_token` siguen siendo rechazadas con `401 Unauthorized`.

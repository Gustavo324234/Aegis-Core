# CORE-233 — feat(shell+kernel): Settings del tenant — simplificar y unificar configuración

**Tipo:** feat (refactor UX)  
**Prioridad:** Alta  
**Asignado a:** Shell Engineer  
**Epic:** Bugs pre-lanzamiento  
**Estado:** ✅ Done

---

## Contexto

El `SettingsPanel.tsx` del tenant tiene 5 tabs (Persona, Motor, Voz, Seguridad, Cuentas)
con problemas de claridad y usabilidad:

1. **Tab Motor y Tab Seguridad son confusas en conjunto**: Motor tiene API URL + modelo,
   Seguridad tiene `TenantKeyManager` (las API keys). El usuario debe ir a dos lugares
   distintos para configurar un provider: poner la URL/modelo en Motor y la key en
   Seguridad. No es obvio que están relacionados.

2. **`sttAvailable` en Tab Voz puede estar desactualizado**: se lee del store, que se
   actualiza solo cuando `fetchSirenConfig()` se llama al conectar. Si el usuario abre
   settings antes de que el WebSocket esté listo, puede ver estado incorrecto.

3. **No hay feedback de qué está activo actualmente**: Motor muestra el engine guardado
   pero no si está funcionando. Voz muestra el provider pero no si TTS está operativo.

4. **Tab Seguridad mezcla 3 cosas**: idioma, API keys personales, y cambio de contraseña.
   No tiene coherencia conceptual.

## Cambios requeridos

### 1. Fusionar "Motor" + keys del provider en una sola tab

**Archivo:** `shell/ui/src/components/SettingsPanel.tsx`

Reestructurar las tabs a 4:

- **Perfil** (antes: Persona) — nombre, prompt de personalidad
- **Motor** (fusión de Motor actual + TenantKeyManager) — provider, modelo, API key del
  provider, todo junto en una sola pantalla. El `TenantKeyManager` se integra visualmente
  como un bloque "API Keys por proveedor" dentro de esta tab.
- **Voz** — TTS/STT, sin cambios estructurales
- **Cuenta** — idioma + cambio de contraseña. Sacar las API keys de aquí (se mueven a Motor).
  Agregar `ConnectedAccountsTab` (OAuth) aquí.

### 2. Refrescar `sttAvailable` al abrir la tab Voz

**Archivo:** `shell/ui/src/components/SettingsPanel.tsx`, `VozTab`

Al montar `VozTab`, llamar `fetchSirenConfig()` del store para obtener el estado
actualizado del servidor en lugar de usar el valor cacheado.

### 3. Agregar indicador de estado "activo" en Tab Motor

Debajo del engine guardado, mostrar un badge:
- ✅ Configurado y operativo (si el último mensaje fue exitoso)
- ⚠️ Configurado, sin verificar (si no hubo mensajes aún)
- ❌ Error de autenticación (si el último request falló con 401)

Este estado puede venir del campo `lastRoutingInfo` que ya existe en el store, o de
un nuevo campo `lastEngineError`.

### 4. Agregar tooltip o tooltip/hint que explique config global vs personal

En la Tab Motor, agregar un texto:
> "Si el administrador configuró un motor global, tu configuración personal lo
> sobreescribe solo para tu cuenta."

Esto responde la pregunta que el usuario tiene y que actualmente no tiene respuesta
visible en la UI.

## Criterios de aceptación

- [ ] El usuario puede configurar provider + modelo + API key del provider en una sola tab sin ir a otro lugar
- [ ] Al abrir la tab Voz, el estado de STT es siempre fresco (no cacheado del mount inicial)
- [ ] La tab Cuenta contiene: idioma, cambio de contraseña, cuentas OAuth conectadas
- [ ] Hay un indicador de estado del motor activo en la tab Motor
- [ ] Un usuario nuevo entiende en menos de 30 segundos cómo configurar su proveedor de IA

## Dependencias

Ninguna de otros tickets de fix. Puede trabajarse en paralelo.

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: Settings unificados: tabs perfil|motor|voz|logs|cuenta; TenantKeyManager dentro de MotorTab (SettingsPanel.tsx:383); idioma+contraseña+cuentas en CuentaTab — la tab Seguridad fue disuelta como pedía el ticket.

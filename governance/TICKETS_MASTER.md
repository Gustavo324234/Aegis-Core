# TICKETS_MASTER.md — Aegis Core

> Fuente de verdad única para todos los tickets del monorepo Aegis-Core.

---

## 🏗️ EPIC 32–37: Foundation, Fixes, Hardening — DONE ✅

---

## 🤖 EPIC 38: Agent Persona System — DONE ✅

### ADR-039: Persona en `kv_store` SQLCipher del tenant. Máx 4000 chars.

*   **[CORE-128]** Fix: `SYSTEM_PROMPT_MASTER` honesto + `build_prompt(persona)` `[DONE]`
*   **[CORE-129]** Feature: Persona en SQLCipher + endpoints `/api/persona` `[DONE]`
*   **[CORE-132]** Feature: Onboarding conversacional — primer mensaje sin Persona `[DONE — Kernel Engineer]`
*   **[CORE-134]** Fix: TLS en Axum puerto 8000 — depende de CORE-142 `[DONE]`
*   **[CORE-130]** Feature: Tab "Persona" en Admin Dashboard `[DONE — Shell Engineer]`
*   **[CORE-133]** Feature: Settings Panel expandido — Persona, Motor, Voz, Seguridad, Cuentas `[DONE — Shell Engineer]`
*   **[CORE-131]** Feature: Display Persona en App modo Satélite `[DONE — Shell Engineer]`

**Orden:** CORE-142 → CORE-134 → CORE-128 → CORE-129 → CORE-132 → CORE-130/133/131

---

## 🎵 EPIC 39: Aegis Music — DONE ✅

*   **[CORE-135]** Feature: Plugin `music_search` + syscall + interceptor `[MUSIC_PLAY]` `[DONE — Kernel Engineer]`
*   **[CORE-136]** Feature: MusicPlayer flotante YouTube IFrame `[DONE — Shell Engineer]`
*   **[CORE-137]** Feature: Comandos de control por chat y voz `[DONE — Kernel Engineer]`

**Orden:** CORE-135 → CORE-136 → CORE-137 (paralelo Shell)

---

## 🔗 EPIC 40: Connected Accounts (OAuth) — DONE ✅

### ADR-041
> OAuth tokens en `kv_store` del enclave SQLCipher del tenant.
> El servidor es receptor de tokens, no OAuth client.
> La app mobile hace el flujo OAuth completo y pasa los tokens al servidor.

### ADR-042
> SystemConfig en `MasterEnclave`. TLS automático en installer, sin preguntas.

### ADR-043
> La app mobile tiene los Client IDs de Google y Spotify compilados.
> El servidor no necesita Client IDs ni registrar apps.
> Los usuarios conectan sus cuentas desde la app con un tap — flujo nativo OAuth.
> La Shell web muestra el estado de las conexiones y permite desconectar.

### Kernel
*   **[CORE-142]** Feature: SystemConfig en MasterEnclave + TLS automático `[DONE — Kernel Engineer]`
*   **[CORE-138]** Feature: OAuth token receiver — `POST /api/oauth/tokens` + `GET /api/oauth/status` + `DELETE /api/oauth/{provider}` `[DONE — Kernel Engineer]`
*   **[CORE-140]** Feature: Spotify music + Google OAuth para YouTube sin key manual `[DONE — Kernel Engineer]`
*   **[CORE-141]** Feature: Google Calendar, Drive, Gmail via syscalls `[DONE — Kernel Engineer]`

### Shell
*   **[CORE-139]** Feature: Connected Accounts — estado en SettingsPanel, desconectar desde web `[DONE — Shell Engineer]`

### App
*   **[CORE-143]** Feature: OAuth via `expo-auth-session` — Google + Spotify desde la app, envía tokens al servidor `[DONE — Shell Engineer]`

### Orden de implementación
1. **CORE-142** — TLS + SystemConfig (fundación)
2. **CORE-138** — receptor de tokens OAuth
3. **CORE-143** — app hace OAuth y envía tokens (Tavo registra apps primero)
4. **CORE-139** — Shell muestra estado
5. **CORE-140** + **CORE-141** — integraciones (paralelo)

### Acción de Tavo antes de CORE-143
- Registrar app en Google Cloud Console (Client IDs Android + iOS + Web para Expo)
- Registrar app en Spotify Developer Dashboard
- Reemplazar `PLACEHOLDER_*` en `app/src/constants/oauth.ts`

---

## 🛠️ Maintenance & Technical Debt

*   **[CORE-144]** Security: `rustls-pemfile` unmaintained (RUSTSEC-2025-0134) `[BLOCKED — upstream axum-server]`
    *   *Detalle:* La dependencia `axum-server` usa `rustls-pemfile`, que fue archivada en agosto 2025.
    *   *Solución parcial:* Se ignoró en `deny.toml` para permitir CI.
    *   *Plan:* Migrar a `axum-tls` nativo o esperar a que `axum-server` se actualice.

---

## 🔮 EPIC 33: Linux Distribution — PLANNED post-producción

---

## 🚀 SISTEMA STATUS — 2026-04-21

| Componente | Estado |
|---|---|
| Epic 32–37 | ✅ DONE |
| Epic 38: Agent Persona System | ✅ DONE — 7/7 |
| Epic 39: Aegis Music | ✅ DONE — 3/3 |
| Epic 40: Connected Accounts | ✅ DONE — 6/6 |
| Chat end-to-end | ✅ OPERATIVO |
| Siren desde LAN | ✅ TLS CONFIGURADO — merged |
| OAuth / Música integrada | ✅ COMPLETO — Spotify (CORE-140) + Google (CORE-141) |

**Total tickets pendientes: 1 (CORE-144)**
**Ticket fundación: CORE-142 — DONE**
**Acción de Tavo antes de CORE-143: completar — apps Google + Spotify registradas y placeholders reemplazados**

---

*Última actualización: 2026-04-21 — ADR-043: app mobile como OAuth client, servidor solo receptor.*

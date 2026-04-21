# DISPATCH_EPICS_38_39_40.md — Plan de despacho Epics 38, 39, 40

> **Fecha:** 2026-04-21
> **Epics:** 38 (Agent Persona), 39 (Music), 40 (Connected Accounts / OAuth)
> **Total tickets:** 16
> **Ingenieros:** Kernel Engineer (Claude Code Max) + Shell Engineer (Antigravity)

---

## ⚠️ ACCIÓN DE TAVO ANTES DE EMPEZAR

### Antes de CORE-143 (no bloquea las otras rondas):
Registrar apps OAuth:

**Google Cloud Console** (console.cloud.google.com):
1. Nuevo proyecto → nombre "Aegis OS"
2. Habilitar APIs: YouTube Data API v3, Calendar API, Drive API, Gmail API
3. Pantalla de consentimiento → "Externo" → nombre "Aegis OS"
4. Credenciales → OAuth 2.0 Client ID:
   - Tipo "Android" → package name de la app Expo
   - Tipo "iOS" → bundle ID de la app Expo
   - Tipo "Web" → redirect URI: `https://auth.expo.io/@<tu-usuario>/aegis`
5. Copiar Client ID

**Spotify Developer Dashboard** (developer.spotify.com):
1. Create App → nombre "Aegis OS"
2. Redirect URIs: `aegis://oauth/spotify` + `exp://localhost:8081/--/oauth/spotify`
3. Copiar Client ID

Una vez obtenidos, reemplazar en `app/src/constants/oauth.ts`:
- `PLACEHOLDER_GOOGLE_CLIENT_ID` → el Client ID de Google
- `PLACEHOLDER_SPOTIFY_CLIENT_ID` → el Client ID de Spotify

---

## DIAGRAMA DE DEPENDENCIAS

```
RONDA 1 (paralelo — autónomos)
  [Kernel]  CORE-142  TLS automático + SystemConfig en MasterEnclave
  [Kernel]  CORE-128  SYSTEM_PROMPT_MASTER honesto

RONDA 2 (requiere CORE-142)
  [Kernel]  CORE-134  TLS en Axum puerto 8000

RONDA 3 (requiere CORE-128)
  [Kernel]  CORE-129  Persona en SQLCipher + endpoints /api/persona
  [Kernel]  CORE-135  Plugin music_search + interceptor [MUSIC_PLAY]
  [Kernel]  CORE-138  OAuth token receiver (POST/GET/DELETE /api/oauth/*)

RONDA 4 (requiere CORE-129)
  [Kernel]  CORE-132  Onboarding conversacional de Persona

RONDA 5 (requiere CORE-138)
  [Kernel]  CORE-141  Google Calendar, Drive, Gmail via syscalls

RONDA 6 (requiere CORE-135 + CORE-138)
  [Kernel+Shell]  CORE-140  Spotify music + Google OAuth para YouTube

─────────────────────────────────────────────────────────
SHELL (puede empezar cuando el kernel correspondiente esté mergeado)

RONDA S1 (requiere CORE-129 mergeado)
  [Shell]  CORE-133  Settings Panel expandido — base de toda la UI

RONDA S2 (requiere CORE-133)
  [Shell]  CORE-130  Tab Persona en Admin Dashboard
  [Shell]  CORE-139  Connected Accounts — estado en Shell web

RONDA S3 (requiere CORE-135 mergeado)
  [Shell]  CORE-136  MusicPlayer flotante YouTube IFrame

RONDA S4 (requiere CORE-136)
  [Shell]  CORE-137  Comandos de control de música por chat y voz

RONDA S5 (requiere CORE-138 mergeado + Tavo registró apps OAuth)
  [Shell]  CORE-143  OAuth via expo-auth-session en app mobile

RONDA S6 (requiere CORE-129)
  [Shell]  CORE-131  Display Persona en App modo Satélite
```

---

## RONDA 1 — Despachar primero (paralelo)

### 🦀 KERNEL ENGINEER — CORE-142

```
Sos el Kernel Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)
REPOS DE REFERENCIA (solo lectura, nunca modificar):
  - Aegis-ANK → lógica del kernel

PROTOCOLO DE INICIO (ejecutar antes de cualquier otra cosa):
1. get_project_structure("Aegis-Core")
2. read_file("Aegis-Core", "governance/TICKETS_MASTER.md")
3. read_file("Aegis-Core", "governance/AEGIS_CONTEXT.md")
4. read_file("Aegis-Core", "governance/Tickets/CORE-142.md")
5. read_file("Aegis-Core", "kernel/crates/ank-core/src/enclave/master.rs")
6. read_file("Aegis-Core", "kernel/crates/ank-server/src/main.rs")
7. read_file("Aegis-Core", "installer/install.sh")

STACK: Rust, Tokio, Axum, SQLCipher, Bash
DIRECTORIO DE TRABAJO: Aegis-Core/kernel/ + Aegis-Core/installer/

LEYES:
- Zero-Panic: prohibido .unwrap() y .expect()
- Toda auth usa Citadel: tenant_id + SHA-256(passphrase)
- set -euo pipefail en todos los scripts Bash

GATE:
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
  shellcheck installer/install.sh installer/aegis

FLUJO GIT:
1. git checkout -b feat/core-142-system-config-tls
2. Implementar
3. git commit -m "feat(ank-core,ank-http,installer): CORE-142 SystemConfig in MasterEnclave + automatic TLS"
4. git push origin feat/core-142-system-config-tls
5. Reportar PR sugerido

AL TERMINAR:
- Marcar CORE-142 como [DONE] en governance/TICKETS_MASTER.md
- Reportar archivos modificados, rama y comando PR

TAREA: Implementar el ticket CORE-142. Lee el ticket completo antes de empezar.
```

---

### 🦀 KERNEL ENGINEER — CORE-128

```
Sos el Kernel Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. get_project_structure("Aegis-Core")
2. read_file("Aegis-Core", "governance/Tickets/CORE-128.md")
3. read_file("Aegis-Core", "kernel/crates/ank-core/src/chal/mod.rs")

STACK: Rust, Tokio
DIRECTORIO DE TRABAJO: Aegis-Core/kernel/crates/ank-core/

LEYES:
- Zero-Panic: prohibido .unwrap() y .expect()

GATE:
  cargo fmt --all
  cargo build -p ank-core
  cargo clippy -p ank-core -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

FLUJO GIT:
1. git checkout -b fix/core-128-honest-system-prompt
2. Implementar
3. git commit -m "fix(ank-core): CORE-128 honest system prompt — no hallucinated actions, no invented capabilities"
4. git push origin fix/core-128-honest-system-prompt
5. Reportar PR sugerido

AL TERMINAR:
- Marcar CORE-128 como [DONE] en governance/TICKETS_MASTER.md

TAREA: Implementar el ticket CORE-128. Lee el ticket completo antes de empezar.
```

---

## RONDA 2 — Después de CORE-142 mergeado

### 🦀 KERNEL ENGINEER — CORE-134

```
Sos el Kernel Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-134.md")
2. read_file("Aegis-Core", "kernel/crates/ank-http/src/lib.rs")
3. read_file("Aegis-Core", "kernel/crates/ank-http/src/config.rs")
4. read_file("Aegis-Core", "kernel/crates/ank-server/src/main.rs")

STACK: Rust, Axum, rustls
DIRECTORIO DE TRABAJO: Aegis-Core/kernel/crates/ank-http/

DEPENDENCIA: CORE-142 debe estar mergeado en main antes de empezar.

GATE:
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

FLUJO GIT:
1. git checkout -b fix/core-134-tls-axum
2. Implementar
3. git commit -m "fix(ank-http): CORE-134 TLS for Axum — HTTPS on port 8000 enables Siren from LAN devices"
4. git push origin fix/core-134-tls-axum
5. Reportar PR sugerido

AL TERMINAR:
- Marcar CORE-134 como [DONE] en governance/TICKETS_MASTER.md

TAREA: Implementar el ticket CORE-134. Lee el ticket completo antes de empezar.
```

---

## RONDA 3 — Después de CORE-128 mergeado (paralelo entre sí)

### 🦀 KERNEL ENGINEER — CORE-129

```
Sos el Kernel Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-129.md")
2. read_file("Aegis-Core", "kernel/crates/ank-core/src/enclave/mod.rs")
3. read_file("Aegis-Core", "kernel/crates/ank-http/src/routes/mod.rs")
4. read_file("Aegis-Core", "kernel/crates/ank-http/src/ws/chat.rs")

STACK: Rust, SQLCipher, Axum
DIRECTORIO DE TRABAJO: Aegis-Core/kernel/

DEPENDENCIA: CORE-128 debe estar mergeado en main antes de empezar.

GATE:
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

FLUJO GIT:
1. git checkout -b feat/core-129-persona-storage
2. Implementar
3. git commit -m "feat(ank-core,ank-http): CORE-129 agent persona — SQLCipher storage + REST endpoints"
4. git push origin feat/core-129-persona-storage
5. Reportar PR sugerido

AL TERMINAR:
- Marcar CORE-129 como [DONE] en governance/TICKETS_MASTER.md

TAREA: Implementar el ticket CORE-129. Lee el ticket completo antes de empezar.
```

---

### 🦀 KERNEL ENGINEER — CORE-135

```
Sos el Kernel Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-135.md")
2. read_file("Aegis-Core", "kernel/crates/ank-core/src/syscalls/mod.rs")
3. read_file("Aegis-Core", "kernel/crates/ank-http/src/ws/chat.rs")
4. read_file("Aegis-Core", "kernel/crates/ank-core/src/chal/mod.rs")

STACK: Rust, Axum WebSocket, reqwest
DIRECTORIO DE TRABAJO: Aegis-Core/kernel/

DEPENDENCIA: CORE-128 debe estar mergeado en main antes de empezar.

GATE:
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

FLUJO GIT:
1. git checkout -b feat/core-135-music-search
2. Implementar
3. git commit -m "feat(ank-core,ank-http): CORE-135 music module — YouTube search syscall + music_play event"
4. git push origin feat/core-135-music-search
5. Reportar PR sugerido

AL TERMINAR:
- Marcar CORE-135 como [DONE] en governance/TICKETS_MASTER.md

TAREA: Implementar el ticket CORE-135. Lee el ticket completo antes de empezar.
```

---

### 🦀 KERNEL ENGINEER — CORE-138

```
Sos el Kernel Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-138.md")
2. read_file("Aegis-Core", "kernel/crates/ank-core/src/enclave/mod.rs")
3. read_file("Aegis-Core", "kernel/crates/ank-http/src/routes/mod.rs")
4. read_file("Aegis-Core", "kernel/crates/ank-http/src/citadel.rs")

STACK: Rust, SQLCipher, Axum, reqwest
DIRECTORIO DE TRABAJO: Aegis-Core/kernel/

DEPENDENCIA: CORE-142 debe estar mergeado en main antes de empezar.

GATE:
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

FLUJO GIT:
1. git checkout -b feat/core-138-oauth-token-receiver
2. Implementar
3. git commit -m "feat(ank-core,ank-http): CORE-138 OAuth token receiver — store tokens from mobile app in SQLCipher"
4. git push origin feat/core-138-oauth-token-receiver
5. Reportar PR sugerido

AL TERMINAR:
- Marcar CORE-138 como [DONE] en governance/TICKETS_MASTER.md

TAREA: Implementar el ticket CORE-138. Lee el ticket completo antes de empezar.
```

---

## RONDA 4 — Después de CORE-129 mergeado

### 🦀 KERNEL ENGINEER — CORE-132

```
Sos el Kernel Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-132.md")
2. read_file("Aegis-Core", "kernel/crates/ank-http/src/ws/chat.rs")
3. read_file("Aegis-Core", "kernel/crates/ank-http/src/state.rs")
4. read_file("Aegis-Core", "kernel/crates/ank-core/src/enclave/mod.rs")

STACK: Rust, Axum WebSocket, SQLCipher
DIRECTORIO DE TRABAJO: Aegis-Core/kernel/

DEPENDENCIAS: CORE-128 + CORE-129 deben estar mergeados en main.

GATE:
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

FLUJO GIT:
1. git checkout -b feat/core-132-persona-onboarding
2. Implementar
3. git commit -m "feat(ank-http): CORE-132 conversational persona onboarding — first message setup flow"
4. git push origin feat/core-132-persona-onboarding
5. Reportar PR sugerido

AL TERMINAR:
- Marcar CORE-132 como [DONE] en governance/TICKETS_MASTER.md

TAREA: Implementar el ticket CORE-132. Lee el ticket completo antes de empezar.
```

---

## RONDA 5 — Después de CORE-138 mergeado

### 🦀 KERNEL ENGINEER — CORE-141

```
Sos el Kernel Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-141.md")
2. read_file("Aegis-Core", "kernel/crates/ank-core/src/syscalls/mod.rs")
3. read_file("Aegis-Core", "kernel/crates/ank-core/src/oauth/mod.rs")
4. read_file("Aegis-Core", "kernel/crates/ank-core/src/enclave/mod.rs")

STACK: Rust, reqwest, Google APIs
DIRECTORIO DE TRABAJO: Aegis-Core/kernel/crates/ank-core/

DEPENDENCIA: CORE-138 debe estar mergeado en main.

GATE:
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

FLUJO GIT:
1. git checkout -b feat/core-141-google-integrations
2. Implementar
3. git commit -m "feat(ank-core): CORE-141 Google integrations — Calendar, Drive, Gmail syscalls via OAuth"
4. git push origin feat/core-141-google-integrations
5. Reportar PR sugerido

AL TERMINAR:
- Marcar CORE-141 como [DONE] en governance/TICKETS_MASTER.md

TAREA: Implementar el ticket CORE-141. Lee el ticket completo antes de empezar.
```

---

## RONDA 6 — Después de CORE-135 + CORE-138 mergeados

### 🦀🎨 KERNEL + SHELL ENGINEER — CORE-140

```
Sos el Kernel Engineer de Aegis Core. Este ticket tiene componentes de kernel y shell.
Implementá la parte del kernel primero (búsqueda Spotify + eventos extendidos),
luego coordinar con el Shell Engineer para la parte del player.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-140.md")
2. read_file("Aegis-Core", "kernel/crates/ank-core/src/syscalls/mod.rs")
3. read_file("Aegis-Core", "kernel/crates/ank-core/src/oauth/mod.rs")
4. read_file("Aegis-Core", "kernel/crates/ank-http/src/ws/chat.rs")

DEPENDENCIAS: CORE-138 + CORE-135 deben estar mergeados en main.

GATE:
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

FLUJO GIT:
1. git checkout -b feat/core-140-spotify-music
2. Implementar parte kernel
3. git commit -m "feat(ank-core,shell): CORE-140 Spotify music — OAuth-based search and playback"
4. git push origin feat/core-140-spotify-music
5. Reportar PR sugerido

TAREA: Implementar el ticket CORE-140. Lee el ticket completo antes de empezar.
```

---

## SHELL — RONDA S1 (después de CORE-129 mergeado)

### 🎨 SHELL ENGINEER — CORE-133

```
Sos el Shell Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. get_project_structure("Aegis-Core")
2. read_file("Aegis-Core", "governance/Tickets/CORE-133.md")
3. read_file("Aegis-Core", "shell/ui/src/components/ChatTerminal.tsx")
4. read_file("Aegis-Core", "shell/ui/src/store/useAegisStore.ts")

STACK: React 18, TypeScript strict, Zustand, Tailwind CSS, Vite
DIRECTORIO DE TRABAJO: Aegis-Core/shell/ui/

DEPENDENCIA: CORE-129 debe estar mergeado en main antes de empezar.

GATE:
  cd shell/ui && npm run build
  cd shell/ui && npm run lint

FLUJO GIT:
1. git checkout -b feat/core-133-settings-panel
2. Implementar
3. git commit -m "feat(shell): CORE-133 expanded settings panel — persona, motor, voz, seguridad"
4. git push origin feat/core-133-settings-panel
5. Reportar PR sugerido

AL TERMINAR:
- Marcar CORE-133 como [DONE] en governance/TICKETS_MASTER.md

TAREA: Implementar el ticket CORE-133. Lee el ticket completo antes de empezar.
```

---

## SHELL — RONDA S2 (después de CORE-133 mergeado)

### 🎨 SHELL ENGINEER — CORE-130

```
Sos el Shell Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-130.md")
2. read_file("Aegis-Core", "shell/ui/src/components/AdminDashboard.tsx")

STACK: React 18, TypeScript strict, Zustand, Tailwind CSS
DIRECTORIO DE TRABAJO: Aegis-Core/shell/ui/

DEPENDENCIAS: CORE-129 + CORE-133 deben estar mergeados.

GATE:
  cd shell/ui && npm run build
  cd shell/ui && npm run lint

FLUJO GIT:
1. git checkout -b feat/core-130-persona-tab-admin
2. git commit -m "feat(shell): CORE-130 persona tab — agent identity editor in admin dashboard"
3. git push origin feat/core-130-persona-tab-admin

TAREA: Implementar el ticket CORE-130. Lee el ticket completo antes de empezar.
```

---

### 🎨 SHELL ENGINEER — CORE-139

```
Sos el Shell Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-139.md")
2. read_file("Aegis-Core", "shell/ui/src/components/ChatTerminal.tsx")

STACK: React 18, TypeScript strict, Tailwind CSS
DIRECTORIO DE TRABAJO: Aegis-Core/shell/ui/

DEPENDENCIAS: CORE-138 + CORE-133 deben estar mergeados.

GATE:
  cd shell/ui && npm run build
  cd shell/ui && npm run lint

FLUJO GIT:
1. git checkout -b feat/core-139-connected-accounts-shell
2. git commit -m "feat(shell): CORE-139 connected accounts status tab — show OAuth connections, disconnect from web"
3. git push origin feat/core-139-connected-accounts-shell

TAREA: Implementar el ticket CORE-139. Lee el ticket completo antes de empezar.
```

---

## SHELL — RONDA S3 (después de CORE-135 mergeado)

### 🎨 SHELL ENGINEER — CORE-136

```
Sos el Shell Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-136.md")
2. read_file("Aegis-Core", "shell/ui/src/components/ChatTerminal.tsx")
3. read_file("Aegis-Core", "shell/ui/src/store/useAegisStore.ts")

STACK: React 18, TypeScript strict, Zustand, Tailwind CSS, YouTube IFrame API
DIRECTORIO DE TRABAJO: Aegis-Core/shell/ui/

DEPENDENCIA: CORE-135 debe estar mergeado en main.

GATE:
  cd shell/ui && npm run build
  cd shell/ui && npm run lint

FLUJO GIT:
1. git checkout -b feat/core-136-music-player
2. git commit -m "feat(shell): CORE-136 music player UI — floating YouTube player with play/pause/volume controls"
3. git push origin feat/core-136-music-player

TAREA: Implementar el ticket CORE-136. Lee el ticket completo antes de empezar.
```

---

## SHELL — RONDA S4 (después de CORE-136 mergeado)

### 🎨 SHELL ENGINEER — CORE-137

```
Sos el Shell Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-137.md")
2. read_file("Aegis-Core", "shell/ui/src/store/musicStore.ts")
3. read_file("Aegis-Core", "shell/ui/src/store/useAegisStore.ts")
4. read_file("Aegis-Core", "kernel/crates/ank-http/src/ws/chat.rs")

DEPENDENCIAS: CORE-135 + CORE-136 mergeados. Coordinar con Kernel Engineer
para que agregue los regex de control en ws/chat.rs.

GATE:
  cd shell/ui && npm run build && npm run lint

FLUJO GIT:
1. git checkout -b feat/core-137-music-controls
2. git commit -m "feat(ank-http,shell): CORE-137 music controls — pause/resume/stop/volume via chat and voice"
3. git push origin feat/core-137-music-controls

TAREA: Implementar el ticket CORE-137. Lee el ticket completo antes de empezar.
```

---

## SHELL — RONDA S5 (después de CORE-138 mergeado + Tavo registró apps)

### 🎨 SHELL ENGINEER — CORE-143

```
Sos el Shell Engineer de Aegis Core. Este ticket es para la app mobile (React Native / Expo).

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-143.md")
2. read_file("Aegis-Core", "app/src/services/bffClient.ts")
3. read_file("Aegis-Core", "app/src/stores/authStore.ts")
4. read_file("Aegis-Core", "app/app/(main)/_layout.tsx")
5. read_file("Aegis-Core", "app/app.json")

STACK: React Native, Expo SDK 52, TypeScript, expo-auth-session, expo-web-browser
DIRECTORIO DE TRABAJO: Aegis-Core/app/

DEPENDENCIAS:
- CORE-138 mergeado en main (endpoint /api/oauth/tokens operativo)
- Tavo reemplazó los PLACEHOLDER en app/src/constants/oauth.ts con Client IDs reales

GATE:
  npx expo export

FLUJO GIT:
1. git checkout -b feat/core-143-oauth-mobile
2. git commit -m "feat(app): CORE-143 OAuth via expo-auth-session — Google and Spotify connect from mobile app"
3. git push origin feat/core-143-oauth-mobile

AL TERMINAR:
- Marcar CORE-143 como [DONE] en governance/TICKETS_MASTER.md

TAREA: Implementar el ticket CORE-143. Lee el ticket completo antes de empezar.
```

---

## SHELL — RONDA S6 (después de CORE-129 mergeado)

### 🎨 SHELL ENGINEER — CORE-131

```
Sos el Shell Engineer de Aegis Core. Este ticket es para la app mobile.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-131.md")
2. read_file("Aegis-Core", "app/src/stores/settingsStore.ts")
3. read_file("Aegis-Core", "app/app/(main)/settings.tsx")

STACK: React Native, Expo SDK 52, TypeScript, Zustand
DIRECTORIO DE TRABAJO: Aegis-Core/app/

DEPENDENCIA: CORE-129 mergeado (endpoint /api/persona operativo).

GATE:
  npx expo export

FLUJO GIT:
1. git checkout -b feat/core-131-persona-app
2. git commit -m "feat(app): CORE-131 persona display in settings — satellite mode awareness"
3. git push origin feat/core-131-persona-app

TAREA: Implementar el ticket CORE-131. Lee el ticket completo antes de empezar.
```

---

## RESUMEN VISUAL

```
TAVO (antes de CORE-143):
  → Registrar apps Google + Spotify → reemplazar PLACEHOLDERs en oauth.ts

KERNEL ENGINEER:
  Ronda 1 (paralelo):  CORE-142  CORE-128
  Ronda 2:             CORE-134  (requiere 142)
  Ronda 3 (paralelo):  CORE-129  CORE-135  CORE-138  (requieren 128 o 142)
  Ronda 4:             CORE-132  (requiere 129)
  Ronda 5:             CORE-141  (requiere 138)
  Ronda 6:             CORE-140  (requiere 135+138)

SHELL ENGINEER:
  Ronda S1:  CORE-133  (requiere 129)
  Ronda S2:  CORE-130  CORE-139  (requieren 133)
  Ronda S3:  CORE-136  (requiere 135)
  Ronda S4:  CORE-137  (requiere 136)
  Ronda S5:  CORE-143  (requiere 138 + placeholders reemplazados)
  Ronda S6:  CORE-131  (requiere 129)
```

---

*Generado por Arquitecto IA — 2026-04-21*
*Para agregar tickets futuros: seguir el mismo formato de prompt.*

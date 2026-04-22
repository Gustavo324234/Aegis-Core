# DISPATCH_EPIC_41.md — Plan de despacho Epic 41

> **Fecha:** 2026-04-22
> **Epic:** 41 — UX, Onboarding & Reliability
> **Tickets:** CORE-145, CORE-146, CORE-147
> **Prioridad:** CORE-147 primero (crítico), luego CORE-145 y CORE-146 en paralelo

---

## ORDEN DE DESPACHO

```
INMEDIATO:
  [Kernel/DevOps]  CORE-147  Fix TLS — autónomo, crítico

EN PARALELO (después de CORE-147 mergeado):
  [Kernel]         CORE-145  Onboarding conversacional (kernel)
  [Shell]          CORE-145  Onboarding — renombrar tab a "Identidad"
  [Kernel]         CORE-146  QR endpoint + Cloudflare tunnel
  [Shell]          CORE-146  QR component en Shell web
  [Shell/App]      CORE-146  Scanner QR en app mobile
```

---

## 🔴 CORE-147 — Despachar AHORA

### 🦀 KERNEL ENGINEER — CORE-147

```
Sos el Kernel Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. get_project_structure("Aegis-Core")
2. read_file("Aegis-Core", "governance/Tickets/CORE-147.md")
3. read_file("Aegis-Core", "installer/aegis")
4. read_file("Aegis-Core", "installer/install.sh")
5. read_file("Aegis-Core", "kernel/crates/ank-server/src/main.rs")

STACK: Bash, Rust/Axum
DIRECTORIOS: Aegis-Core/installer/ + Aegis-Core/kernel/crates/ank-server/

LEYES:
- set -euo pipefail en todos los scripts Bash
- Zero-Panic en Rust: prohibido .unwrap() y .expect()

GATE:
  shellcheck installer/aegis installer/install.sh
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

FLUJO GIT:
1. git checkout -b fix/core-147-tls-update
2. Implementar los 4 fixes del ticket
3. git commit -m "fix(installer): CORE-147 aegis update regenerates TLS cert + tls-regen command + server TLS logging"
4. git push origin fix/core-147-tls-update
5. Reportar PR sugerido

AL TERMINAR:
- Marcar CORE-147 como [DONE] en governance/TICKETS_MASTER.md

TAREA: Implementar el ticket CORE-147. Lee el ticket completo antes de empezar.
El fix tiene 4 partes: (1) cmd_update regenera cert, (2) función regenerate_tls_cert,
(3) nuevo comando aegis tls-regen, (4) logs TLS explícitos en ank-server/main.rs.
```

---

## 🤖 CORE-145 — Onboarding conversacional

### 🦀 KERNEL ENGINEER — CORE-145 (parte kernel)

```
Sos el Kernel Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-145.md")
2. read_file("Aegis-Core", "kernel/crates/ank-http/src/ws/chat.rs")
3. read_file("Aegis-Core", "kernel/crates/ank-core/src/enclave/mod.rs")
4. read_file("Aegis-Core", "kernel/crates/ank-http/src/routes/persona_api.rs")

STACK: Rust, Axum WebSocket, SQLCipher
DIRECTORIO: Aegis-Core/kernel/

DEPENDENCIA: CORE-147 mergeado (servidor estable con TLS).

LEYES:
- Zero-Panic: prohibido .unwrap() y .expect()

GATE:
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

FLUJO GIT:
1. git checkout -b feat/core-145-persona-onboarding-chat
2. Implementar state machine de onboarding en ws/chat.rs
3. Agregar métodos de onboarding en TenantDB (enclave/mod.rs)
4. git commit -m "feat(ank-http,ank-core): CORE-145 conversational persona onboarding in chat"
5. git push origin feat/core-145-persona-onboarding-chat
6. Reportar PR

AL TERMINAR: Marcar CORE-145 como [DONE] en TICKETS_MASTER.md

TAREA: Implementar la parte kernel del ticket CORE-145.
El flujo tiene 3 estados: sin Persona → awaiting_name → awaiting_style → Persona guardada.
Lee el ticket completo antes de empezar.
```

---

### 🎨 SHELL ENGINEER — CORE-145 (parte shell)

```
Sos el Shell Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-145.md")
2. read_file("Aegis-Core", "shell/ui/src/components/SettingsPanel.tsx")
3. read_file("Aegis-Core", "shell/ui/src/components/AdminDashboard.tsx")

STACK: React 18, TypeScript, Tailwind CSS
DIRECTORIO: Aegis-Core/shell/ui/

TAREA ESPECÍFICA (solo la parte Shell de CORE-145):
- Renombrar el tab "Persona" a "Identidad" en SettingsPanel.tsx
- Cambiar el ícono del tab a Sparkles (lucide-react)
- Actualizar la descripción del tab
- Agregar nota en el botón Reset: "Al resetear, el agente te pedirá nombre y estilo nuevamente"
- Si existe el tab "Persona" en AdminDashboard, renombrarlo también a "Identidad"
- NO modificar la lógica de la PersonaTab — solo los labels y el ícono

GATE:
  cd shell/ui && npm run build
  cd shell/ui && npm run lint

FLUJO GIT:
1. git checkout -b feat/core-145-identity-tab-rename
2. Implementar
3. git commit -m "feat(shell): CORE-145 rename Persona tab to Identidad + reset hint"
4. git push origin feat/core-145-identity-tab-rename
5. Reportar PR

TAREA: Implementar la parte Shell del ticket CORE-145. Lee el ticket completo.
```

---

## 📱 CORE-146 — QR + Tunnel

### 🦀 KERNEL ENGINEER — CORE-146 (parte kernel)

```
Sos el Kernel Engineer de Aegis Core.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-146.md")
2. read_file("Aegis-Core", "kernel/crates/ank-server/src/main.rs")
3. read_file("Aegis-Core", "kernel/crates/ank-http/src/routes/mod.rs")
4. read_file("Aegis-Core", "kernel/crates/ank-http/src/state.rs")
5. read_file("Aegis-Core", "installer/install.sh")

STACK: Rust, Tokio, Bash
DIRECTORIOS: Aegis-Core/kernel/ + Aegis-Core/installer/

LEYES:
- Zero-Panic: prohibido .unwrap() y .expect()
- El tunnel es best-effort: si cloudflared no está instalado, el servidor arranca igual

GATE:
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
  shellcheck installer/install.sh

FLUJO GIT:
1. git checkout -b feat/core-146-qr-tunnel-kernel
2. Implementar:
   - AppState: agregar tunnel_url: Arc<RwLock<Option<String>>>
   - main.rs: lanzar tunnel manager como tokio::spawn
   - routes/status.rs o nuevo routes/connection_info.rs: GET /api/system/connection-info
   - installer/install.sh: función install_cloudflared()
3. git commit -m "feat(ank-server,installer): CORE-146 Cloudflare tunnel + connection-info endpoint"
4. git push origin feat/core-146-qr-tunnel-kernel
5. Reportar PR

TAREA: Implementar la parte kernel del ticket CORE-146. Lee el ticket completo.
```

---

### 🎨 SHELL ENGINEER — CORE-146 (Shell + App)

```
Sos el Shell Engineer de Aegis Core. Este ticket cubre la Shell web y la app mobile.

REPO DE TRABAJO: Aegis-Core (único repo activo)

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-146.md")
2. read_file("Aegis-Core", "shell/ui/src/components/ChatTerminal.tsx")
3. read_file("Aegis-Core", "shell/ui/package.json")
4. read_file("Aegis-Core", "app/app/(auth)/login.tsx")
5. read_file("Aegis-Core", "app/package.json")
6. read_file("Aegis-Core", "app/app.json")

STACK:
- Shell: React 18, TypeScript, Tailwind CSS, qrcode.react
- App: React Native, Expo SDK 52, expo-camera

DEPENDENCIA: CORE-146 kernel mergeado (endpoint /api/system/connection-info operativo)

GATE Shell:
  cd shell/ui && npm install qrcode.react
  cd shell/ui && npm run build
  cd shell/ui && npm run lint

GATE App:
  cd app && npx expo install expo-camera
  npx expo export

FLUJO GIT:
1. git checkout -b feat/core-146-qr-shell-app
2. Implementar:
   Shell: ConnectionQR.tsx + botón QR en ChatTerminal header
   App: botón "Escanear QR" en login.tsx + CameraView scanner
3. git commit -m "feat(shell,app): CORE-146 QR connection UI in Shell + QR scanner in app"
4. git push origin feat/core-146-qr-shell-app
5. Reportar PR

TAREA: Implementar la parte Shell + App del ticket CORE-146. Lee el ticket completo.
```

---

## RESUMEN

```
INMEDIATO:
  Kernel → CORE-147 (fix TLS — crítico)

CUANDO CORE-147 MERGEADO:
  Kernel → CORE-145 kernel (onboarding en chat)
  Shell  → CORE-145 shell  (renombrar tab)   [paralelo]
  Kernel → CORE-146 kernel (tunnel + endpoint)
  Shell  → CORE-146 shell+app (QR UI)        [depende de kernel]
```

---

*Generado: 2026-04-22 — Arquitecto IA*

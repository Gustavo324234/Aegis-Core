# DISPATCH_EPIC_41.md — Plan de despacho Epic 41

> **Fecha:** 2026-04-22
> **Epic:** 41 — UX, Onboarding & Reliability
> **Tickets:** CORE-145, CORE-146, CORE-147

---

## ORDEN

```
INMEDIATO:
  [Kernel]  CORE-147  Fix TLS — crítico, autónomo

DESPUÉS DE CORE-147:
  [Kernel]  CORE-145  Onboarding en chat (SOLO kernel — Shell ya está)
  [Kernel]  CORE-146  QR endpoint + Cloudflare tunnel
  [Shell]   CORE-146  QR component en Shell + scanner en app
```

---

## 🔴 CORE-147 — Fix TLS (despachar YA)

```
Sos el Kernel Engineer de Aegis Core.

PROTOCOLO DE INICIO:
1. get_project_structure("Aegis-Core")
2. read_file("Aegis-Core", "governance/Tickets/CORE-147.md")
3. read_file("Aegis-Core", "installer/aegis")
4. read_file("Aegis-Core", "installer/install.sh")
5. read_file("Aegis-Core", "kernel/crates/ank-server/src/main.rs")

STACK: Bash + Rust
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
5. Reportar PR

AL TERMINAR: Marcar CORE-147 como [DONE] en governance/TICKETS_MASTER.md

TAREA: Implementar el ticket CORE-147. Lee el ticket completo antes de empezar.
```

---

## 🤖 CORE-145 — Onboarding conversacional (SOLO KERNEL)

```
Sos el Kernel Engineer de Aegis Core.

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-145.md")
2. read_file("Aegis-Core", "kernel/crates/ank-http/src/ws/chat.rs")
3. read_file("Aegis-Core", "kernel/crates/ank-core/src/enclave/mod.rs")

STACK: Rust, Axum WebSocket, SQLCipher
DIRECTORIO: Aegis-Core/kernel/

DEPENDENCIA: CORE-147 mergeado.

NOTA IMPORTANTE: Los cambios de Shell para este ticket YA ESTÁN implementados
(SettingsPanel tiene el tab Persona con icono Sparkles y hint de reset).
Este ticket es EXCLUSIVAMENTE kernel — cero cambios en shell/ ni en app/.

LEYES: Zero-Panic — prohibido .unwrap() y .expect()

GATE:
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

FLUJO GIT:
1. git checkout -b feat/core-145-identity-onboarding
2. Implementar en kernel/ únicamente:
   - Métodos de onboarding en TenantDB (enclave/mod.rs): get/set/clear onboarding_step, set/get onboarding_name
   - Interceptor de onboarding en ws/chat.rs: saludo al conectar + state machine de 2 steps
3. git commit -m "feat(ank-http,ank-core): CORE-145 conversational identity onboarding — agent asks name and style in chat"
4. git push origin feat/core-145-identity-onboarding
5. Reportar PR

AL TERMINAR: Marcar CORE-145 como [DONE] en governance/TICKETS_MASTER.md

TAREA: Implementar el ticket CORE-145. Lee el ticket completo antes de empezar.
```

---

## 📱 CORE-146 — QR + Tunnel (Kernel)

```
Sos el Kernel Engineer de Aegis Core.

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-146.md")
2. read_file("Aegis-Core", "kernel/crates/ank-server/src/main.rs")
3. read_file("Aegis-Core", "kernel/crates/ank-http/src/routes/mod.rs")
4. read_file("Aegis-Core", "kernel/crates/ank-http/src/state.rs")
5. read_file("Aegis-Core", "installer/install.sh")

STACK: Rust, Tokio, Bash
LEYES: Zero-Panic. El tunnel es best-effort — si cloudflared no está, el servidor arranca igual.

GATE:
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
  shellcheck installer/install.sh

FLUJO GIT:
1. git checkout -b feat/core-146-qr-tunnel
2. Implementar:
   - AppState: agregar tunnel_url: Arc<RwLock<Option<String>>>
   - main.rs: lanzar tunnel manager como tokio::spawn (best-effort)
   - GET /api/system/connection-info: retorna local_url, tunnel_url, tunnel_status
   - installer/install.sh: función install_cloudflared()
3. git commit -m "feat(ank-server,installer): CORE-146 Cloudflare tunnel + connection-info endpoint"
4. git push origin feat/core-146-qr-tunnel
5. Reportar PR

TAREA: Implementar la parte kernel del ticket CORE-146. Lee el ticket completo.
```

---

## 📱 CORE-146 — QR + Tunnel (Shell + App)

```
Sos el Shell Engineer de Aegis Core.

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-146.md")
2. read_file("Aegis-Core", "shell/ui/src/components/ChatTerminal.tsx")
3. read_file("Aegis-Core", "shell/ui/package.json")
4. read_file("Aegis-Core", "app/app/(auth)/login.tsx")
5. read_file("Aegis-Core", "app/package.json")
6. read_file("Aegis-Core", "app/app.json")

STACK: React 18 + React Native / Expo SDK 52
DEPENDENCIA: CORE-146 kernel mergeado (endpoint /api/system/connection-info operativo).

GATE Shell:
  cd shell/ui && npm install qrcode.react
  cd shell/ui && npm run build && npm run lint

GATE App:
  cd app && npx expo install expo-camera
  npx expo export

FLUJO GIT:
1. git checkout -b feat/core-146-qr-ui
2. Implementar:
   - shell/: ConnectionQR.tsx (QR con tunnel_url o local_url) + botón QR en ChatTerminal header
   - app/: botón "Escanear QR" en login.tsx + CameraView scanner + permisos en app.json
3. git commit -m "feat(shell,app): CORE-146 QR connection UI in Shell + QR scanner in mobile app"
4. git push origin feat/core-146-qr-ui
5. Reportar PR

TAREA: Implementar la parte Shell + App del ticket CORE-146. Lee el ticket completo.
```

---

## RESUMEN

```
INMEDIATO:
  Kernel → CORE-147 (fix TLS — crítico)

CUANDO CORE-147 MERGEADO:
  Kernel → CORE-145 (onboarding en chat, solo kernel)
  Kernel → CORE-146 kernel (tunnel + endpoint)   [paralelo]
  Shell  → CORE-146 UI (QR shell + app)          [depende de CORE-146 kernel]
```

---

*Generado: 2026-04-22 — Arquitecto IA*

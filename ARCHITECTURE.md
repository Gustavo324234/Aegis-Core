# Aegis Core — Architecture

> **Version:** 1.2.0
> **Updated:** 2026-11-18

---

## 1. Visión

Aegis Core es la implementación unificada del ecosistema Aegis OS.
El objetivo a largo plazo es una distribución Linux (`aegis-distro`) donde
el kernel cognitivo corre a nivel de sistema operativo — no como una app
de userspace, sino como un servicio de sistema con acceso directo al hardware
para inferencia de modelos.

El prerequisito para llegar ahí es un binario único sin dependencias de runtime.
Ese es el objetivo de este repositorio.

---

## 2. Lo que resuelve vs. el legacy

El sistema legacy (`Aegis-ANK` + `Aegis-Shell`) tiene una capa de traducción
frágil entre ambos componentes:

```
Browser
  │  WebSocket + REST
  ▼
BFF Python (Aegis-Shell/bff/)   ← ~1000 líneas, traduce WS↔gRPC
  │  gRPC + TLS
  ▼
ANK Kernel (Aegis-ANK)
```

**Problemas concretos:**
- Cada cambio en `kernel.proto` requiere regenerar stubs Python manualmente
- Serialización doble en cada mensaje (JSON ↔ Protobuf)
- Dos procesos independientes con fallo inconsistente
- Python como dependencia de runtime para usuarios finales

---

## 3. Arquitectura target

```
Browser / Aegis-App
        │  HTTP + WebSocket
        ▼
 ank-server  (único binario Rust)
        │
        ├── crate: ank-http   ← servidor Axum (HTTP :8000 / sandbox :8090)
        │     ├── /api/auth          autenticación Citadel
        │     ├── /api/admin         gestión de tenants y system-config
        │     ├── /api/engine        configuración de modelos
        │     ├── /api/oauth         Google OAuth + Spotify (Cuentas Conectadas)
        │     ├── /api/whatsapp      proxy + anti-spam hacia sidecar Baileys (:8100)
        │     ├── /api/router        ruteo cognitivo y catálogo CMR v2
        │     ├── /api/chat          historial paginado, ui-state y visual-clear
        │     ├── /api/personal      resumen personal, hechos, recordatorios y Chronos
        │     ├── /api/chronos/events eventos próximos de recordatorios
        │     ├── /api/workspace     configuración del workspace
        │     ├── /api/fs            árbol de archivos + contenido (Epic 44)
        │     ├── /api/prs           ciclo de vida de PRs (Epic 44)
        │     ├── /api/git           status / branches / commits (Epic 44)
        │     ├── /api/agents        orquestador jerárquico (Epic 43)
        │     ├── /api/antigravity   bridge e IDE hooks de Google Antigravity SDK
        │     ├── /api/providers     pool de API keys y modelos activos
        │     ├── /api/siren         STT/TTS y configuración Siren
        │     ├── /api/video         Aegis Video Studio (proyectos, escenas, render MP4 y diagnóstico PC/APIs)
        │     ├── /api/persona / /api/mcp / /api/memory / /api/status / /api/system
        │     ├── /ws/chat           streaming cognitivo
        │     ├── /ws/siren          audio bidireccional
        │     ├── /ws/agents         eventos de agentes en vivo
        │     └── /assets/*          SPA React embebida
        │
        ├── crate: ank-core   ← motor cognitivo
        │     ├── CognitiveScheduler
        │     ├── CognitiveHAL + drivers + StreamTextSanitizer + failover dinámico
        │     ├── VCM (Virtual Context Manager)
        │     ├── Citadel Protocol (TenantDB cifrada con SQLCipher)
        │     ├── DAG compiler
        │     ├── OAuth + Google Calendar (`google_calendar.rs`)
        │     ├── WhatsAppService (`whatsapp/`) — anti-spam, cola pendiente y alias
        │     ├── Video Studio (`video_studio/`, `docs/VIDEO_STUDIO.md`) — pipeline MP4 1-paso, Pollinations Flux/Pixabay/Edge TTS/FFmpeg, fallbacks y advisor HW
        │     ├── Plugin system (Wasm)
        │     ├── Siren Protocol (VAD+STT+TTS)
        │     ├── MCP client
        │     ├── AgentOrchestrator + AntigravityRunner (Epic 43 / 59)
        │     └── Developer Workspace (Epic 44)
        │           ├── WorkspaceConfig  — config cifrada por tenant
        │           ├── TerminalExecutor — comandos con streaming + allowlist
        │           ├── GitHubBridge     — git local + GitHub API
        │           └── PrManager        — ciclo de vida de PRs + polling CI
        │
        ├── crate: ank-proto  ← contratos Protobuf
        │
        └── gRPC :50051       ← API externa (CLI, multi-nodo, clientes externos)

aegis-supervisor              ← process manager (un solo proceso a manejar)
```

---

## 4. Estructura del repositorio

```
aegis-core/
├── kernel/
│   ├── crates/
│   │   ├── ank-core/         motor cognitivo central
│   │   ├── ank-http/         servidor HTTP/WS (Axum) — NUEVO
│   │   ├── ank-proto/        Protobuf → Rust stubs
│   │   ├── ank-server/       binario principal
│   │   ├── ank-cli/          CLI administrativa
│   │   ├── ank-mcp/          cliente MCP
│   │   ├── aegis-supervisor/ process manager
│   │   ├── aegis-sdk/        SDK plugins Wasm
│   │   └── aegis-connect-relay/ relay de Aegis Connect (túneles WebSocket persistentes)
│   ├── modules/              módulos de dominio del microkernel (`aegis-biz`, `aegis-creator`, `aegis-dev`, `aegis-video`)
│   └── proto/
│       ├── kernel.proto      contrato gRPC externo
│       └── siren.proto       contrato audio
│
├── shell/
│   └── ui/                   React 18 + Vite + TypeScript + Zustand + Tailwind
│
├── tools/
│   ├── agent_sandbox/        simulador Python + launcher web aislado (:8090)
│   ├── whatsapp_bridge/      sidecar Node.js (Baileys) para sesiones WhatsApp QR (:8100)
│   └── antigravity_bridge/   worker Python + hooks IDE para Google Antigravity SDK
│
├── app/                      React Native + Expo SDK 52
│
├── installer/
│   ├── install.sh            instalador unificado
│   ├── docker-compose.yml    modo Docker (opcional)
│   └── setup-service.sh      configuración de servicio nativo
│
├── run_sandbox.bat           lanzador sandbox Windows (web :8090 + CLI)
├── update.bat / update.ps1   scripts de actualización de binario y UI en Windows
│
├── governance/
│   ├── TICKETS_MASTER.md
│   ├── AEGIS_CONTEXT.md
│   ├── AEGIS_MASTER_CODEX.md
│   └── Tickets/
│
└── distro/                   (futuro) configuración de distro Linux
```

---

## 5. Epics completados

| Epic | Título | Estado |
|---|---|---|
| Epic 32 | Unificación — binario único Rust (ank-server) | ✅ Done |
| Epic 42 | Realignment — deuda técnica, auth, OAuth, Router | ✅ Done |
| Epic 43 | Hierarchical Multi-Agent Orchestration | ✅ Done |
| Epic 44 | Developer Workspace (terminal, file browser, Git, PR manager) | ✅ Done |
| Epic 45 | Cognitive Agent Architecture (CAA) | ✅ Done |
| Epic 46 | Public Launch (docs, community, open source health) | ✅ Done |
| Epic 51 | Model Intelligence (PinchBench, Ollama Cloud, CMR v2 context scoring) | ✅ Done |
| Epic 52 | Voice Quality (Siren audio WebRTC streaming stabilization & mic-mute) | ✅ Done |
| Epic 53 | Stabilization (Real LLM agent loop, observability dashboard, SRE fixes) | ✅ Done |
| Epic 54 | Aegis Connect (Persistent WebSocket tunnels mapped to Orion ID) | ✅ Done |
| Epic 58 | Personal Assistant Experience, Zero-Friction UX & Community Onboarding | ✅ Done |
| Epic 59 | Antigravity SDK Delegation, Bidirectional IDE Bridge & Natural Voice/Identity Studio | ✅ Done |
| Epic 60 | Real-World Channels (WhatsApp & Google Calendar), Chat UX & ReAct Resilience (`CORE-387..393`) | ✅ Done |
| CORE-150 | Sandbox scripting (Maker Capability - autonomous JS sandbox) | ✅ Done |
| CORE-151 | Project context integration (Git/VCM active context tracking) | ✅ Done |

---

## 6. ADRs

| # | Decisión | Estado |
|---|---|---|
| ADR-001 | Rust para el kernel | Activo |
| ADR-002 | gRPC + Protobuf como API externa | Activo |
| ADR-003 | Citadel Protocol (Zero-Trust multi-tenant) | Activo |
| ADR-006 | LLMs como ALUs (no oráculos) | Activo |
| ADR-010 | Docker como modo opcional (no requerido) | Activo |
| ADR-021 | React Native + Expo para mobile | Activo |
| ADR-022 | App mobile usa HTTP/WS (no gRPC nativo) | Activo |
| ADR-027 | aegis-supervisor como process manager en Rust | Activo |
| ADR-028 | Paths por OS via crate `dirs` | Activo |
| ADR-030 | ank-http: Axum embebido en ank-server | Activo |
| ADR-031 | BFF Python es legacy — no se porta a Core | Activo |
| ADR-032 | Monorepo aegis-core para el sistema unificado | Activo |
| ADR-033 | distro/ reservado para futura distro Linux | Planificado |
| ADR-034 | TenantDB es !Send — ops bloqueantes via spawn_blocking | Activo |
| ADR-035 | GitHub token nunca serializado en respuestas HTTP | Activo |
| ADR-036 | Rutas de FS validadas con canonicalize() + starts_with(root) | Activo |
| ADR-037 | Siren Audio migrado a WebRTC/WebTransport para baja latencia | Activo |
| ADR-038 | Sandbox JS autónomo para Maker Capability (`CORE-150`) | Activo |
| ADR-039 | Aegis Connect: túneles WebSocket inversos y persistentes a Orion ID | Activo |

---

## 6. Relación con repos legacy

| Repo | Rol en Aegis-Core |
|---|---|
| `Aegis-ANK` | Referencia para lógica del kernel, contratos proto, módulos core |
| `Aegis-Shell` | Referencia para endpoints HTTP, lógica UI, Zustand stores |
| `Aegis-Installer` | Referencia para scripts de deployment y systemd |
| `Aegis-App` | Referencia para lógica mobile, modos Satellite/Cloud |
| `Aegis-Governance` | Normativa vigente — TICKETS_MASTER, CODEX, CONTEXT |

Los repos legacy no se modifican desde Aegis-Core.
Aegis-Core implementa el sistema correcto consultando el legacy como referencia.

---

## 7. Visión distro Linux

`distro/` es el destino final. Una base Linux mínima e inmutable con:

- ANK como servicio de sistema con acceso directo al hardware GPU
- Root filesystem read-only, partición `/data` cifrada con SQLCipher
- Aegis Shell servida por ANK al arrancar (sin display manager)
- Imagen deployable en x86_64 y ARM64 (Raspberry Pi 5, NVIDIA Jetson)

Prerequisito: Epic 32 completa (binario único sin dependencias de runtime).

---

*Arquitecto IA — 2026-04-08*

# Aegis Core — Architecture

> **Version:** 1.0.0
> **Updated:** 2026-04-08

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
        ├── crate: ank-http   ← servidor Axum (HTTP :8000)
        │     ├── /api/*           REST endpoints
        │     ├── /ws/chat/        streaming cognitivo
        │     ├── /ws/siren/       audio bidireccional
        │     └── /assets/*        SPA React embebida
        │
        ├── crate: ank-core   ← motor cognitivo
        │     ├── CognitiveScheduler
        │     ├── CognitiveHAL + drivers
        │     ├── VCM (Virtual Context Manager)
        │     ├── Citadel Protocol
        │     ├── DAG compiler
        │     ├── Plugin system (Wasm)
        │     ├── Siren Protocol (VAD+STT+TTS)
        │     └── MCP client
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
│   │   └── aegis-sdk/        SDK plugins Wasm
│   └── proto/
│       ├── kernel.proto      contrato gRPC externo
│       └── siren.proto       contrato audio
│
├── shell/
│   └── ui/                   React 18 + Vite + TypeScript + Zustand + Tailwind
│
├── app/                      React Native + Expo SDK 52
│
├── installer/
│   ├── install.sh            instalador unificado
│   ├── docker-compose.yml    modo Docker (opcional)
│   └── setup-service.sh      configuración de servicio nativo
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

## 5. ADRs

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
| ADR-030 | ank-http: Axum embebido en ank-server | **Epic 32** |
| ADR-031 | BFF Python es legacy — no se porta a Core | Activo |
| ADR-032 | Monorepo aegis-core para el sistema unificado | Activo |
| ADR-033 | distro/ reservado para futura distro Linux | Planificado |

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

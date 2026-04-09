# AEGIS_CONTEXT.md — Aegis Core

> **Versión:** 1.0.0
> **Actualizado:** 2026-04-08
> **Estado:** EPIC 32 COMPLETE — binario único operativo

---

## 1. Visión

Aegis Core es un sistema operativo cognitivo open source. Trata a los LLMs como
ALUs probabilísticas bajo un motor de ejecución determinista. El objetivo a largo
plazo es una distribución Linux (`aegis-distro`) con el kernel cognitivo embebido
a nivel de sistema operativo.

El hito actual: un único binario Rust que sirve todo, sin dependencias de runtime
externas. Sin Python. Sin dos procesos que sincronizar.

---

## 2. Arquitectura del sistema

```
Browser / Aegis-App
        │
        │  HTTP REST + WebSocket
        ▼
┌─────────────────────────────────────────────┐
│  ank-server  (único proceso Rust)           │
│                                             │
│  ┌─────────────────┐  ┌──────────────────┐ │
│  │   ank-http      │  │   Tonic gRPC     │ │
│  │   Axum :8000    │  │   :50051         │ │
│  │                 │  │                  │ │
│  │  /api/*         │  │  KernelService   │ │
│  │  /ws/chat/      │  │  SirenService    │ │
│  │  /ws/siren/     │  │                  │ │
│  │  /assets/*      │  │  (CLI, multi-    │ │
│  └────────┬────────┘  │   nodo, externos)│ │
│           │           └──────────────────┘ │
│           ▼                                │
│  ┌─────────────────────────────────────┐   │
│  │              ank-core               │   │
│  │                                     │   │
│  │  CognitiveScheduler  │  Citadel     │   │
│  │  CognitiveHAL        │  VCM         │   │
│  │  DAG Compiler        │  Scribe      │   │
│  │  Plugin System       │  Siren       │   │
│  │  CognitiveRouter     │  MCP Client  │   │
│  └─────────────────────────────────────┘   │
└─────────────────────────────────────────────┘

aegis-supervisor  →  levanta y monitorea ank-server
aegis-app         →  cliente mobile (HTTP/WS — ADR-022)
ank-cli           →  CLI administrativa (gRPC directo)
```

---

## 3. Crates del workspace

| Crate | Path | Descripción |
|---|---|---|
| `ank-proto` | `kernel/crates/ank-proto/` | Contratos Protobuf compilados a Rust |
| `ank-core` | `kernel/crates/ank-core/` | Motor cognitivo central |
| `ank-http` | `kernel/crates/ank-http/` | Servidor HTTP/WS (Axum) |
| `ank-server` | `kernel/crates/ank-server/` | Entrypoint — levanta Axum + Tonic |
| `ank-cli` | `kernel/crates/ank-cli/` | CLI administrativa vía gRPC |
| `ank-mcp` | `kernel/crates/ank-mcp/` | Cliente MCP (StdIO + SSE) |
| `aegis-supervisor` | `kernel/crates/aegis-supervisor/` | Process manager |
| `aegis-sdk` | `kernel/crates/aegis-sdk/` | SDK Wasm para plugins |
| `plugins_src` | `kernel/plugins_src/` | Plugins estándar compilados a Wasm |

---

## 4. Interfaces públicas

### HTTP — puerto 8000

#### Auth y Admin
| Método | Path | Auth | Descripción |
|---|---|---|---|
| `POST` | `/api/auth/login` | — | Citadel handshake |
| `POST` | `/api/admin/setup` | — | Bootstrap Master Admin |
| `POST` | `/api/admin/setup-token` | — | Bootstrap con OTP |
| `POST` | `/api/admin/tenant` | Admin | Crear tenant |
| `GET` | `/api/admin/tenants` | Admin | Listar tenants |
| `DELETE` | `/api/admin/tenant/:id` | Admin | Eliminar tenant |
| `POST` | `/api/admin/reset_password` | Admin | Reset password |

#### Engine y Telemetría
| Método | Path | Auth | Descripción |
|---|---|---|---|
| `GET` | `/api/engine/status` | — | Estado del engine |
| `POST` | `/api/engine/configure` | Tenant | Configurar engine |
| `GET` | `/api/status` | Tenant | Métricas del kernel |
| `GET` | `/api/system/state` | — | Estado público |
| `GET` | `/health` | — | Health check |

#### Router CMR
| Método | Path | Descripción |
|---|---|---|
| `POST/GET/DELETE` | `/api/router/keys/global` | KeyPool global |
| `POST/GET/DELETE` | `/api/router/keys/tenant` | KeyPool tenant |
| `GET` | `/api/router/models` | Catálogo de modelos |
| `POST` | `/api/router/sync` | Forzar sync catálogo |

#### Workspace y Voz
| Método | Path | Descripción |
|---|---|---|
| `POST` | `/api/workspace/upload` | Subir archivo al workspace |
| `POST` | `/api/providers/models` | Listar modelos de un provider |
| `GET/POST` | `/api/siren/config` | Config de voz |
| `GET` | `/api/siren/voices` | Voces disponibles |

#### WebSocket
| Path | Protocolo | Descripción |
|---|---|---|
| `/ws/chat/{tenant_id}` | `session-key.<key>` | Streaming cognitivo |
| `/ws/siren/{tenant_id}` | `session-key.<key>` | Audio bidireccional |

### gRPC — puerto 50051

`KernelService`: `SubmitTask`, `WatchTask`, `GetSystemStatus`, `ListProcesses`,
`TeleportProcess`, `InitializeMasterAdmin`, `CreateTenant`, `ConfigureEngine`,
`ResetTenantPassword`, `ListTenants`, `DeleteTenant`, `AddGlobalKey`,
`ListGlobalKeys`, `DeleteKey`, `ListMyKeys`, `SyncRouterCatalog`,
`ListRouterModels`, `GetSirenConfig`, `SetSirenConfig`, `ListSirenVoices`

`SirenService`: `SirenStream` (bidireccional)

---

## 5. ADRs activos

| # | Decisión | Estado |
|---|---|---|
| ADR-001 | Rust para el kernel | Activo |
| ADR-002 | gRPC + Protobuf como API externa | Activo |
| ADR-003 | Citadel Protocol (Zero-Trust multi-tenant) | Activo |
| ADR-006 | LLMs como ALUs (no oráculos) | Activo |
| ADR-007 | Wasmtime para plugins | Activo |
| ADR-008 | SQLCipher para enclaves | Activo |
| ADR-010 | Docker como opción (no requerido) | Activo |
| ADR-021 | React Native + Expo para mobile | Activo |
| ADR-022 | App mobile usa HTTP/WS (no gRPC nativo) | Activo |
| ADR-027 | aegis-supervisor como process manager Rust | Activo |
| ADR-028 | Paths por OS via crate `dirs` | Activo |
| ADR-029 | Docker permanece como opción válida | Activo |
| ADR-030 | ank-http: Axum embebido en ank-server | **Implementado** |
| ADR-031 | BFF Python es legacy — no existe en Core | **Implementado** |
| ADR-032 | Monorepo aegis-core | **Activo** |
| ADR-033 | distro/ reservado para futura distro Linux | Planificado |

---

## 6. Repos legacy (referencia de solo lectura)

| Repo | Qué aporta como referencia |
|---|---|
| `Aegis-ANK` | Lógica del kernel, contratos proto, módulos ank-core |
| `Aegis-Shell` | Endpoints HTTP, lógica UI, Zustand stores |
| `Aegis-Installer` | Scripts de deployment, systemd |
| `Aegis-App` | Lógica mobile, modos Satellite/Cloud |
| `Aegis-Governance` | Normativa, CODEX, tickets históricos |

---

## 7. Limitaciones conocidas y deuda técnica

| ID | Área | Descripción |
|---|---|---|
| LIM-001 | ank-core | LanceDB desactivado — conflictos de compilación |
| LIM-002 | ank-core | ONNX Local Embeddings pendiente (post-launch) |
| LIM-003 | ank-http | embed-ui feature flag no implementado en Fase 1 |
| DT-001 | ank-core | MCP Tool Orchestrator Schema Mapping pendiente |
| DT-002 | ank-core | Hardware Dual (NVIDIA + Coral) pendiente |
| DT-003 | distro/ | Sin contenido — prerequisito: Epic 32 estable en producción |

---

*Documento mantenido por: Arquitecto IA*
*Última actualización: 2026-04-08 — Epic 32 completa*

# AEGIS_CONTEXT.md — Aegis Core

> **Versión:** 1.2.0
> **Actualizado:** 2026-04-21
> **Estado:** EPIC 38 IN PROGRESS — Agent Persona System

---

## 1. Visión

Aegis Core es un sistema operativo cognitivo open source. Trata a los LLMs como
ALUs probabilísticas bajo un motor de ejecución determinista. El objetivo a largo
plazo es una distribución Linux (`aegis-distro`) con el kernel cognitivo embebido
a nivel de sistema operativo.

El hito actual: un único binario Rust que sirve todo, sin dependencias de runtime
externas. Sin Python. Sin dos procesos que sincronizar. Chat end-to-end operativo.

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

### Flujo de inferencia con Persona (Epic 38)

```
WebSocket /ws/chat
    │  PCB vía SchedulerEvent::ScheduleTaskConfirmed
    ▼
CognitiveScheduler (ready_queue)
    │  execution_tx — canal mpsc
    ▼
HAL Runner (tokio::spawn en main.rs)
    │  Lee Persona del enclave SQLCipher del tenant (best-effort)
    │  hal.route_and_execute(shared_pcb, persona: Option<String>)
    ▼
CognitiveHAL::build_prompt(instruction, persona)
    │  SYSTEM_PROMPT_MASTER + [IDENTIDAD CONFIGURADA] + instrucción
    ▼
CognitiveRouter → CloudProxyDriver → LLM API
    │  token stream
    ▼
event_broker → WebSocket → Browser
```

---

## 3. Crates del workspace

| Crate | Path | Descripción |
|---|---|---|
| `ank-proto` | `kernel/crates/ank-proto/` | Contratos Protobuf compilados a Rust |
| `ank-core` | `kernel/crates/ank-core/` | Motor cognitivo central |
| `ank-http` | `kernel/crates/ank-http/` | Servidor HTTP/WS (Axum) |
| `ank-server` | `kernel/crates/ank-server/` | Entrypoint — levanta Axum + Tonic + HAL Runner |
| `ank-cli` | `kernel/crates/ank-cli/` | CLI administrativa vía gRPC |
| `ank-mcp` | `kernel/crates/ank-mcp/` | Cliente MCP (StdIO + SSE) |
| `aegis-supervisor` | `kernel/crates/aegis-supervisor/` | Process manager |
| `aegis-sdk` | `kernel/crates/aegis-sdk/` | SDK Wasm para plugins |
| `plugins_src` | `kernel/plugins_src/` | Plugins estándar compilados a Wasm |

---

## 4. Interfaces públicas

### Protocolo Citadel (obligatorio en todas las rutas protegidas)

**Headers HTTP:** `x-citadel-tenant: <tenant_id>` + `x-citadel-key: <passphrase_plaintext>`

El servidor aplica SHA-256 a `x-citadel-key` antes de validar contra el enclave.
El enclave recibe `SHA-256(passphrase)` y aplica Argon2id sobre ese valor (no sobre
el passphrase original). Este comportamiento es intencional: el hash de 256 bits de
entropía fija es un input válido para Argon2id y evita que el plaintext cruce el
boundary del enclave. Las credenciales **nunca** viajan en query params, body ni FormData.

**WebSocket:** subprotocol `session-key.<passphrase_plaintext>` — el servidor aplica
SHA-256 al valor extraído del subprotocol antes de llamar a `authenticate_tenant`.
Flujo idéntico al HTTP: SHA-256(passphrase) → Argon2id → verificación.

### HTTP — puerto 8000

#### Auth y Admin
| Método | Path | Auth | Descripción |
|---|---|---|---|
| `POST` | `/api/auth/login` | — | Citadel handshake. Responde `{ status, role }` |
| `POST` | `/api/admin/setup` | — | Bootstrap Master Admin |
| `POST` | `/api/admin/setup-token` | — | Bootstrap con OTP |
| `POST` | `/api/admin/tenant` | Admin (headers) | Crear tenant |
| `GET` | `/api/admin/tenants` | Admin (headers) | Listar tenants |
| `DELETE` | `/api/admin/tenant/:id` | Admin (headers) | Eliminar tenant |
| `POST` | `/api/admin/reset_password` | Admin (headers) | Reset password |

#### Engine y Telemetría
| Método | Path | Auth | Descripción |
|---|---|---|---|
| `GET` | `/api/engine/status` | — | Estado del engine (lee de `data_dir/engine_config.json`) |
| `POST` | `/api/engine/configure` | Tenant (headers) | Configurar engine |
| `GET` | `/api/status` | Tenant (headers) | Métricas del kernel |
| `GET` | `/api/system/state` | — | Estado público |
| `GET` | `/health` | — | Health check |

#### Router CMR
| Método | Path | Auth | Descripción |
|---|---|---|---|
| `POST` | `/api/router/keys/global` | Admin (headers + authenticate_master) | Agregar key global |
| `GET` | `/api/router/keys/global` | Admin (CitadelAuthenticated) | Listar keys globales |
| `DELETE` | `/api/router/keys/global/:id` | Admin (CitadelAuthenticated) | Eliminar key global |
| `POST/GET/DELETE` | `/api/router/keys/tenant` | Tenant (CitadelAuthenticated) | KeyPool tenant |
| `GET` | `/api/router/models` | Tenant (CitadelAuthenticated) | Catálogo de modelos |
| `POST` | `/api/router/sync` | Admin (CitadelAuthenticated) | Forzar sync catálogo |

#### Workspace, Voz y Persona
| Método | Path | Auth | Descripción |
|---|---|---|---|
| `POST` | `/api/workspace/upload` | Tenant (CitadelAuthenticated en headers) | Subir archivo |
| `POST` | `/api/providers/models` | Tenant (CitadelAuthenticated) | Listar modelos de provider |
| `GET` | `/api/siren/config` | Tenant (CitadelAuthenticated) | Config de voz |
| `POST` | `/api/siren/config` | Tenant (CitadelAuthenticated) | Actualizar config de voz |
| `GET` | `/api/siren/voices` | — | Voces disponibles |
| `GET` | `/api/persona` | Tenant (CitadelAuthenticated) | Leer Persona del agente (Epic 38) |
| `POST` | `/api/persona` | Tenant (CitadelAuthenticated) | Guardar Persona — body: `{ "persona": "..." }` |
| `DELETE` | `/api/persona` | Tenant (CitadelAuthenticated) | Eliminar Persona (restaura default) |

#### WebSocket
| Path | Protocolo | Descripción |
|---|---|---|
| `/ws/chat/{tenant_id}` | `session-key.<passphrase>` | Streaming cognitivo |
| `/ws/siren/{tenant_id}` | `session-key.<passphrase>` | Audio bidireccional |

### gRPC — puerto 50051

`KernelService` (implementado): `SubmitTask`, `WatchTask`, `GetSystemStatus`,
`ListProcesses`, `InitializeMasterAdmin`, `CreateTenant`, `ResetTenantPassword`,
`ListTenants`, `DeleteTenant`, `AddGlobalKey`, `ListGlobalKeys`, `DeleteKey`,
`ListMyKeys`, `SyncRouterCatalog`, `ListRouterModels`

`KernelService` (stub pendiente): `TeleportProcess`, `ConfigureEngine`,
`GetSirenConfig`, `SetSirenConfig`, `ListSirenVoices`

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
| ADR-028 | Paths por OS via crate `dirs` + AEGIS_DATA_DIR | Activo |
| ADR-029 | Docker permanece como opción válida | Activo |
| ADR-030 | ank-http: Axum embebido en ank-server | Activo |
| ADR-031 | BFF Python es legacy — no existe en Core | Activo |
| ADR-032 | Monorepo aegis-core | Activo |
| ADR-033 | distro/ reservado para futura distro Linux | Planificado |
| ADR-034 | Citadel credentials via HTTP headers únicamente — nunca query params ni body | Activo |
| ADR-035 | HAL Runner: goroutine dedicada en main.rs conecta Scheduler → HAL → event_broker | Activo |
| ADR-036 | Anthropic/DeepSeek/Mistral/Qwen se acceden via OpenRouter (protocolo OpenAI-compatible) | Activo |
| ADR-038 | VCM L3: `fast-hnsw` como motor de vector search embebido (pure Rust, zero dependencies) | Activo |
| ADR-039 | Agent Persona: almacenada en `kv_store` del enclave SQLCipher del tenant, clave `"agent_persona"`, máx. 4000 chars | **Activo (Epic 38)** |

---

## 6. Repos legacy (referencia de solo lectura)

| Repo | Qué aporta como referencia |
|---|---|
| `Aegis-ANK` | Lógica del kernel, contratos proto, módulos ank-core |
| `Aegis-Shell` | Endpoints HTTP legacy, lógica UI, Zustand stores |
| `Aegis-Installer` | Scripts de deployment, systemd |
| `Aegis-App` | Lógica mobile, modos Satellite/Cloud |
| `Aegis-Governance` | Normativa, CODEX, tickets históricos |

---

## 7. Limitaciones conocidas y deuda técnica

| ID | Área | Descripción |
|---|---|---|
| LIM-001 | ank-core | LanceDB desactivado — conflictos de compilación (RESUELTO via ADR-038 con fast-hnsw) |
| LIM-002 | ank-core | ONNX Local Embeddings pendiente (post-launch) |
| LIM-003 | ank-http | embed-ui feature flag no implementado |
| LIM-004 | ank-http | ws/siren STT completo pendiente — path mínimo implementado |
| LIM-005 | ank-core | Anthropic/DeepSeek/Mistral/Qwen requieren key de OpenRouter, no key directa del provider |
| DT-001 | ank-core | MCP Tool Orchestrator Schema Mapping pendiente |
| DT-002 | ank-core | Hardware Dual (NVIDIA + Coral) pendiente |
| DT-003 | distro/ | Sin contenido — prerequisito: smoke test en producción |

---

*Documento mantenido por: Arquitecto IA*
*v1.0.0 — 2026-04-08: Epic 32 completa*
*v1.1.0 — 2026-04-13: Epic 34 completa — audit fixes, flujo de inferencia conectado*
*v1.2.0 — 2026-04-16: ADR-038 — VCM L3 con fast-hnsw (CORE-098)*
*v1.3.0 — 2026-04-21: Epic 38 iniciada — ADR-039 Agent Persona, endpoints /api/persona*

---

## ADR-039: Agent Persona — Identidad configurable por tenant

**Fecha:** 2026-04-21
**Status:** ACEPTADA
**Tickets:** CORE-128, CORE-129, CORE-130, CORE-131

### Problema

El `SYSTEM_PROMPT_MASTER` sin contexto de identidad produce:
1. Alucinación de acciones no ejecutadas ("He registrado ese gasto")
2. Identidad genérica inventada en lugar de una presentación honesta
3. Formato lista innecesario en respuestas conversacionales

### Decisión

**La Persona es un atributo del enclave del tenant.** Se persiste en la `kv_store`
del `TenantDB` bajo la clave `"agent_persona"`. Máximo 4000 caracteres. Vacía por
defecto — el agente se presenta como "Aegis" y no inventa capacidades.

**No se añade tabla nueva en SQLCipher.** El `kv_store` genérico ya existe y tiene
exactamente la semántica requerida: clave → valor persistido, acceso by key.

**Lectura en el WebSocket handler (best-effort).** Al recibir cada mensaje de chat,
el handler intenta leer la persona del enclave. Si falla (error de IO, enclave
corrompido), continúa sin persona en lugar de rechazar la inferencia.

**La Persona NO viaja en el PCB ni en el TaskRequest.** Es responsabilidad del
handler HTTP/WS leerla del enclave antes de llamar al HAL. El Kernel no expone
la Persona hacia el exterior.

### Consecuencias positivas
- Sin cambios en Protobuf — cero impacto en gRPC
- Sin nueva tabla SQLCipher — cero migración de esquema
- Retrocompatible — tenants sin persona no ven cambio alguno
- El operador puede personalizar completamente el agente sin tocar código

### Consecuencias negativas
- Leer el enclave en cada mensaje de chat añade latencia de disco (SQLite, ~1ms)
- Si el enclave del tenant cambia de passphrase, la persona se vuelve inaccesible
  hasta que el tenant se re-autentique (comportamiento esperado — enclave bloqueado)

---

## ADR-038: Estrategia para VCM L3 post-launch

**Fecha:** 2026-04-16
**Status:** ACEPTADA
**Agente:** Kernel Engineer
**Ticket:** CORE-098

### Contexto

El VCM (Vector Cognitive Memory) opera actualmente con L1 (contexto de ventana) y L2 (caché en memoria).
L3 (memoria a largo plazo via vector search) está desactivado. El código stub (`swap.rs`) tiene la estructura
para `LanceSwapManager` pero la persistencia está marcada como `FUTURE(ANK-2402)` sin implementación.

**LIM-001** reportaba "conflictos de compilación" con LanceDB. Investigación reveló:
- LanceDB NO está actualmente en el workspace de Cargo.toml
- LanceDB requiere Arrow como dependencia, cuyas versiones frecuentemente entran en conflicto
- El workspace usa Arrow 58 (especifico para compatibilidad con prost/prost-types)

### Decisión

Elegir `fast-hnsw` — pure Rust, zero Arrow conflicts, API ideal para payloads, persistencia built-in.

Ver documento completo en v1.2.0.

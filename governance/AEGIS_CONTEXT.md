# AEGIS_CONTEXT.md — Aegis Core

> **Versión:** 1.1.0
> **Actualizado:** 2026-04-13
> **Estado:** EPIC 34 COMPLETE — sistema funcional end-to-end, audit fixes aplicados

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

### Flujo de inferencia (Epic 34 — CORE-085)

```
WebSocket /ws/chat
    │  PCB vía SchedulerEvent::ScheduleTaskConfirmed
    ▼
CognitiveScheduler (ready_queue)
    │  execution_tx — canal mpsc
    ▼
HAL Runner (tokio::spawn en main.rs)
    │  hal.route_and_execute(shared_pcb)
    ▼
CognitiveRouter → CloudProxyDriver → LLM API
    │  token stream
    ▼
event_broker (broadcast::Sender por PID)
    │
    ▼
WebSocket → Browser
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

#### Workspace y Voz
| Método | Path | Auth | Descripción |
|---|---|---|---|
| `POST` | `/api/workspace/upload` | Tenant (CitadelAuthenticated en headers) | Subir archivo |
| `POST` | `/api/providers/models` | Tenant (CitadelAuthenticated) | Listar modelos de provider |
| `GET` | `/api/siren/config` | Tenant (CitadelAuthenticated) | Config de voz |
| `POST` | `/api/siren/config` | Tenant (CitadelAuthenticated) | Actualizar config de voz |
| `GET` | `/api/siren/voices` | — | Voces disponibles |

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
| ADR-034 | Citadel credentials via HTTP headers únicamente — nunca query params ni body | **Activo (Epic 34)** |
| ADR-035 | HAL Runner: goroutine dedicada en main.rs conecta Scheduler → HAL → event_broker | **Activo (Epic 34)** |
| ADR-036 | Anthropic/DeepSeek/Mistral/Qwen se acceden via OpenRouter (protocolo OpenAI-compatible) | **Activo (Epic 34)** |
| ADR-038 | VCM L3: `fast-hnsw` como motor de vector search embebido (pure Rust, zero dependencies) | **Activo (CORE-098)** |

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

### Análisis de alternativas

| Opción | Dependencias | Madurez | API | Compatibilidad Arrow 58 | Veredicto |
|---|---|---|---|---|---|
| **LanceDB** | Arrow, object_store | Alta | Excelente | Conflicto frecuente | RECHAZADO |
| **qdrant-client** | Servidor externo | Alta | Excelente | N/A (no embebido) | RECHAZADO (añade dependencia externa) |
| **usearch** | C++ via cxx-build | Alta | Buena | N/A (C++ puro) | ALTERNATIVA (bindings C++) |
| **hnsw_rs** | parking_lot, serde | Media | Media | Compatible | CONSIDERADO |
| **fast-hnsw** | memmap2 (pure Rust) | Alta (Feb 2026) | Excelente | Compatible | **ELEGIDO** |

### Decisión

**Elegir `fast-hnsw`** por las siguientes razones:

1. **Zero dependencies externas** — Solo `memmap2` (pure Rust), sin bindings C++ ni Arrow
2. **API ideal para payloads** — `LabeledIndex<T>` permite almacenar `MemoryFragment` completo
3. **Persistencia built-in** — `save()`/`load()` con mmap para datasets grandes
4. **Pure Rust** — Cumple con las Laws SRE del CLAUDE.md (zero panic, manejo de errores via Result)
5. **Benchmarks** — 1.5-3.7x más rápido que hnsw_rs en búsqueda
6. **Activamente mantenido** — v1.0.0 released Feb 2026
7. **Licencia MIT** — Permisiva para el proyecto

### API requerida vs. fast-hnsw

El VCM requiere:
- `insert(vector, id)` → `store_fragment()` ✅
- `search(query_vector, k)` → `search()` ✅
- `delete(id)` → pendiente de implementar en swap.rs
- Persistencia por tenant → un índice por `tenant_id`

`fast-hnsw` soporta:
- `insert(Vec<f32>) -> usize` (id automático)
- `search(&[f32], k, ef) -> Vec<(usize, f32)>` ✅
- `LabeledIndex<T>` para payload con `encode`/`decode` ✅
- `save()`/`load()`/`load_mmap()` ✅

### Implementación (ticket separado CORE-109)

Este ticket (CORE-098) es de investigación. La implementación real requiere:
1. Agregar `fast-hnsw = "1.0"` a workspace dependencies
2. Refactorizar `LanceSwapManager` para usar `LabeledIndex<MemoryFragment>`
3. Implementar `delete` para cumplir con la API del VCM
4. Tests de round-trip (insert → persist → load → search)
5. Integration test con `assemble_context()` del VCM

### Consecuencias

**Positivas:**
- L3 del VCM operativo para memoria a largo plazo
- Zero Arrow conflicts — mantiene compatibilidad prost 0.13
- Sin dependencias externas de runtime

**Negativas:**
- Learning curve de API nueva
- Migration del stub existente en `swap.rs`

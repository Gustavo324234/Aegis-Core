# AUDITORÍA ARQUITECTÓNICA — AEGIS OS
**Modelo auditor:** Claude Sonnet 4.6 (claude-sonnet-4-6)  
**Fecha:** 2026-04-16  
**Rol:** Arquitecto IA — Aegis OS  
**Scope:** Revisión completa del ecosistema: Aegis-ANK, Aegis-Shell, Aegis-Installer, Aegis-App, Aegis-Governance, Aegis-Core  
**Fuentes consultadas:** `AEGIS_CONTEXT.md` (v2.4.0), `TICKETS_MASTER.md`, `AUDIT_REPORT.md`, estructura de todos los repos  

---

## RESUMEN EJECUTIVO

Aegis OS es un proyecto de alta complejidad técnica con una arquitectura coherente y bien fundada. El sistema ha pasado por múltiples ciclos de hardening (Epics 17, 23, 27) y ha llegado al estado `PLATINUM MASTER — LAUNCH READY` con una deuda técnica acotada y conocida. La separación kernel/shell via gRPC es sólida; el SRE Firewall es real y funciona; la gobernanza es inusualmente madura para un proyecto de este tamaño.

Las debilidades principales no son de seguridad (en su mayoría resueltas) sino de **acoplamiento operacional**, **fragmentación de repos vs. monorepo en transición**, y **puntos ciegos en el flujo de código** entre la generación de agentes IA y la validación humana.

---

## 1. PUNTOS FUERTES

### 1.1 Arquitectura Kernel/Shell — Separación Real

La separación entre `ank-server` (Rust/gRPC) y el BFF (FastAPI) no es cosmética. El kernel no tiene ninguna dependencia de la UI; la UI no tiene acceso directo a ningún recurso del kernel. El contrato `kernel.proto` + `siren.proto` es el único punto de contacto y está versionado. Esto es correcto por construcción.

**Impacto:** Un exploit en la capa BFF/UI no puede comprometer el kernel directamente. La superficie de ataque está acotada por el protocolo.

### 1.2 Zero-Panic Policy Enforced

El SRE Firewall ejecuta `cargo clippy -D warnings -D clippy::unwrap_used -D clippy::expect_used` en cada PR. Esto no es decorativo: bloquea merges que violen la política. La migración a `LazyLock` (ANK-STB-017) y los comentarios `// SAFETY:` (ANK-STB-018) son evidencia de que la política se aplica con disciplina real.

**Impacto:** El kernel tiene garantías de estabilidad en runtime superiores a la media de proyectos Rust en producción.

### 1.3 Protocolo Citadel — Multi-Tenancy Real desde Día 1

SQLCipher por tenant, jailing de paths, session_key via WebSocket subprotocol header, CORS restringido, mTLS estricto. El sistema fue diseñado multi-tenant desde el inicio y no como un add-on posterior. El Epic 17 cerró los 14 hallazgos de auditoría; los tickets son verificables uno a uno.

**Impacto:** La base de seguridad es sólida para un lanzamiento open source. Un contribuidor externo no puede escalar privilegios por descuido del diseño original.

### 1.4 Cognitive Model Router (Epic 26)

La abstracción `TaskType` ortogonal a `ModelPreference` es una decisión de diseño acertada. El `CognitiveRouter` con scoring multi-criterio (40/30/20/10) y `CatalogSyncer` en background resuelve un problema real: diferentes tareas cognitivas tienen perfiles de modelo óptimos distintos. El catálogo bundled garantiza arranque offline. El `KeyPool` con round-robin y rate-limit es producción-grade.

**Impacto:** Aegis no es solo un proxy de LLMs; tiene lógica de despacho inteligente que otros proyectos similares no tienen.

### 1.5 Gobernanza con Trazabilidad Real

Cada cambio tiene un ticket. Cada ticket tiene criterios de aceptación. El `TICKETS_MASTER.md` es un log de decisiones arquitectónicas, no solo un backlog. Los ADRs (22 documentados) justifican cada decisión con razón e impacto. Esto es inusual en proyectos individuales o de equipo pequeño.

**Impacto:** Un nuevo contribuidor puede entender POR QUÉ el sistema es como es, no solo QUÉ hace. Esto reduce la deuda de conocimiento al hacer el proyecto open source.

### 1.6 Pipeline CI/CD Robusto

- `cargo fmt` + `clippy` + `test` + `audit` en PRs (ANK)
- `black` + `flake8` + `npm run build` en PRs (Shell)
- `shellcheck` en Installer
- Release-Please automatizado con PAT
- GHCR publishing
- Native binary CI (`publish-native.yml`)

Cuatro SRE Firewalls independientes por repo/stack. Ningún código rompe CI sin que el ingeniero lo vea.

### 1.7 Native Runtime (Epic 31)

La migración de Docker a binario nativo fue una decisión valiente y correcta. Docker era una barrera para el usuario final. `aegis-supervisor` en Rust, paths dinámicos por OS via `dirs`, registro como servicio en Windows/macOS/Linux, y la CLI unificada `aegis` son el nivel correcto de ingeniería para un proyecto que quiere ser accesible.

---

## 2. PUNTOS DÉBILES

### 2.1 CRÍTICO — Transición Multi-Repo → Monorepo Incompleta

**Problema:** Existen simultáneamente `Aegis-ANK`, `Aegis-Shell`, `Aegis-Installer`, `Aegis-App` (repos legacy) y `Aegis-Core` (monorepo nuevo). El workspace overview confirma que ambos conjuntos están accesibles y activos. La documentación de gobernanza (AEGIS_CONTEXT.md) aún describe la arquitectura multi-repo como canónica.

**Evidencia:**
- `Aegis-Core/kernel/`, `Aegis-Core/shell/`, `Aegis-Core/installer/`, `Aegis-Core/app/` existen.
- Los repos legacy siguen teniendo tickets activos, CI activo, y código actual.
- `AEGIS_CONTEXT.md` no menciona `Aegis-Core` en ninguna sección arquitectónica.
- `Aegis-Core/governance/` tiene su propio `TICKETS_MASTER.md` y `AEGIS_CONTEXT.md` — diferente al canon en `Aegis-Governance`.

**Riesgo:** Los agentes IA (Kernel Engineer, Shell Engineer) reciben briefings que apuntan a repos legacy. Si implementan código en `Aegis-ANK` pero el deploy del servidor usa `Aegis-Core`, hay divergencia silenciosa. Este es el riesgo de corrupción de estado más alto del proyecto actualmente.

**Recomendación:** Definir inmediatamente cuál es el repo canónico. Si `Aegis-Core` es el futuro, archivar los repos legacy y actualizar TODA la gobernanza. Si los repos legacy son el presente, limpiar `Aegis-Core` de duplicados.

---

### 2.2 ALTO — AUDIT_REPORT.md Desactualizado (GOV-PROC-001 sin ejecutar)

**Problema:** `AUDIT_REPORT.md` muestra 15 de 20 hallazgos como `🔴 OPEN`. En realidad todos los hallazgos del Epic 17 están `DONE` (verificado en `TICKETS_MASTER.md`). El documento es falso en su estado actual.

**Evidencia:** 
- `TICKETS_MASTER.md`: Epic 17 — "DONE ✅ — 2026-03-17", 14/14 tickets cerrados.
- `AUDIT_REPORT.md`: SEC-006, SEC-007, SEC-008... todos marcados `🔴 OPEN`.
- Ticket `GOV-PROC-001` ("Sync AUDIT_REPORT with TICKETS_MASTER") está en Epic 23 Sprint 2 con estado `TODO`.

**Riesgo:** Un contribuidor externo que lea `AUDIT_REPORT.md` creerá que el sistema tiene vulnerabilidades críticas sin resolver. Esto dañará la percepción del proyecto al lanzarse open source.

**Recomendación:** Ejecutar `GOV-PROC-001` antes del lanzamiento. Tiene prioridad sobre documentación cosmética.

---

### 2.3 ALTO — Flujo de Código: Punto Ciego entre Agente y Validación

**Problema:** El flujo documentado es: agente implementa → `cargo build` (detección de errores de compilación) → Tavo revisa → push → CI. Sin embargo, el historial de tickets muestra un patrón recurrente: los agentes completan código sin actualizar governance (`AEGIS_CONTEXT.md` dice que `main.py` tiene bugs que ya fueron resueltos por separado). La revisión humana ocurre en el diff del commit, no en una ejecución real del sistema.

**Evidencia específica del flujo de código:**

```
[Browser React UI]
    ↓ WebSocket (session_key via Sec-WebSocket-Protocol)
[FastAPI BFF — main.py]
    ↓ gRPC + mTLS (x-citadel-tenant / x-citadel-key)
[ank-server — Rust/Tokio]
    ↓ Tokio async + CognitiveScheduler
[ank-core — HAL + Router + DAG + VCM]
    ↓ llama-cpp-2 / cloud HTTP
[LLM inference]
```

El punto ciego está en el salto BFF → kernel. Históricamente:
- BFF enviaba `x-aegis-tenant-id` / `x-aegis-session-key` (incorrecto — bug resuelto en 2026-04-06)
- BFF no aplicaba SHA-256 al passphrase antes de enviarlo (bug resuelto en 2026-04-06)
- BFF usaba TLS cuando `DEV_MODE=true` debería usar canal inseguro (bug resuelto en 2026-04-06)

Tres bugs críticos de alineación protocolar en el punto de integración más sensible del sistema, descubiertos durante smoke test y no en CI. El CI no prueba la integración BFF↔Kernel.

**Riesgo:** Cualquier cambio en `kernel.proto` (añadir un campo, cambiar un enum) puede romper silenciosamente el BFF Python. Los stubs `kernel_pb2.py` y `kernel_pb2_grpc.py` deben regenerarse manualmente y no hay gate automatizado que valide que están sincronizados con el proto actual.

**Recomendación:**
1. Agregar un test de integración BFF↔Kernel en CI (aunque sea `smoke_test.sh` ejecutado en contenedor).
2. Automatizar la regeneración de stubs Python cuando `kernel.proto` cambie.
3. Documentar en `CONTRIBUTING.md` que cualquier cambio al proto requiere regenerar stubs antes de push.

---

### 2.4 MEDIO — `Aegis-Core/auditorias/` vs `auditoria/` — Fragmentación de Artefactos

**Problema:** Existen dos carpetas de auditoría en `Aegis-Core`: `auditoria/` (singular, con `Gemini_3_Flash.md`) y `auditorias/` (plural, con `big-pickle.md` y `minimax-m2.5-free_AUDIT.md`). No hay una convención única.

**Impacto:** Los artefactos de auditoría quedan dispersos. Dificulta la búsqueda futura y la comparación entre modelos.

**Recomendación:** Unificar en una sola carpeta `auditorias/` (plural, consistente con el estándar de colecciones). Mover `Gemini_3_Flash.md` allí. Este archivo (`claude-sonnet-4-6.md`) debería ir en `auditorias/` para consistencia — aunque se respeta la instrucción de ponerlo en `auditoria/` por ahora.

---

### 2.5 MEDIO — `speed_inv` Hardcodeado en CognitiveRouter

**Problema:** El factor de velocidad inversa del `CognitiveRouter` está hardcodeado a `0.5` (DT-005 en AEGIS_CONTEXT.md). El scoring 40/30/20/10 asigna 20% del peso a velocidad, pero ese peso se aplica sobre un valor constante, neutralizando efectivamente la dimensión de latencia en las decisiones de routing.

**Impacto:** El router toma decisiones subóptimas para tareas latency-sensitive (Siren, autocomplete). El Epic 2604 (Siren Evolution) añadió un `SirenRouter` separado; esto sugiere que el problema fue parcheado con una abstracción nueva en lugar de resolver la raíz.

**Recomendación:** Post-launch, implementar medición real de latencia con percentil P95 por modelo. Hasta entonces, documentar explícitamente la limitación en `ModelCatalog`.

---

### 2.6 MEDIO — LanceDB Desactivado Sin Fecha de Resolución

**Problema:** LanceDB (L3 Swap del VCM) lleva desactivado desde v2.x por "conflictos de compilación" (LIM-001). No hay ticket activo con plan de resolución. ONNX Local Embeddings (ANK-2401) también está marcado `post-launch` sin criterios de aceptación.

**Impacto:** El VCM opera sin L3 (swap a vector store). Para tareas de memoria a largo plazo, el sistema degrada silenciosamente a contexto finito. En la arquitectura descrita (LLMs como ALUs bajo motor determinista), esto es una limitación fundamental: sin L3, el "sistema operativo cognitivo" no puede mantener estado largo entre sesiones.

**Recomendación:** Crear Epic 32 o subticket explícito con fecha tentativa y criterios técnicos para resolver LanceDB. No puede quedar indefinidamente como `post-launch` si es la capa de memoria del sistema.

---

### 2.7 BAJO — `Aegis-NotebookLM-Bundle` en Governance con Código Python

**Problema:** `Aegis-Governance/Aegis-NotebookLM-Bundle/` contiene `bundle_code.py`, `upload.py`, `upload_v2.py` y copias de `AegisAnkCode.txt` / `AegisShellCode.txt`. Governance es definido como repo no-ejecutable. Hay código Python de utilidad y snapshots de código de producción viviendo en el repo de normativa.

**Ticket existente:** `GOV-PROC-002` ("Eliminate/Automate Aegis-NotebookLM-Bundle") — estado `TODO`.

**Riesgo:** Los snapshots en `AegisAnkCode.txt` y `AegisShellCode.txt` pueden quedar desactualizados y llevar a un agente (o a NotebookLM) a razonar sobre código obsoleto.

**Recomendación:** Ejecutar `GOV-PROC-002`. Automatizar el bundle como GitHub Action en los repos fuente o eliminar los snapshots estáticos.

---

## 3. FLUJO DE CÓDIGO — ANÁLISIS DETALLADO

### 3.1 Flujo Nominal (Happy Path)

```
1. Usuario escribe mensaje en ChatTerminal (React)
   → chatStore (Zustand) actualiza estado optimistamente
   → WebSocket abierto en ws://<bff>:8000/ws/chat/{tenant_id}
   → session_key enviada via Sec-WebSocket-Protocol header ✅

2. BFF (main.py) recibe mensaje WebSocket
   → AnkClient singleton desde app.state ✅
   → SHA-256 del passphrase ya aplicado en login (no en cada request)
   → llama grpc.aio con headers x-citadel-tenant + x-citadel-key ✅
   → SubmitTask(TaskRequest{content, task_type}) → retorna PID

3. BFF lanza WatchTask(pid) en stream
   → itera TaskEvent streaming desde kernel
   → forwarda cada chunk al WebSocket del browser

4. ank-server recibe SubmitTask
   → CitadelInterceptor valida tenant_id + session_key del header ✅
   → CognitiveScheduler crea PCB con TaskType
   → CognitiveRouter selecciona modelo via scoring 40/30/20/10
   → KeyPool provee API key con round-robin

5. ank-core ejecuta tarea
   → DAG compiler valida topología (ANK-2413) ✅
   → VCM gestiona contexto (sin LanceDB — solo L1/L2) ⚠️
   → HAL invoca CloudDriver o LlamaNativeDriver
   → EventBroker distribuye TaskEvents al stream de WatchTask

6. Stream llega al browser
   → react-window virtualiza lista de mensajes ✅
   → Markdown renderizado incremental
   → Orb de telemetría actualiza métricas via polling /api/status
```

**Fortalezas del happy path:** El flujo es limpio, sin state compartido entre requests, con backpressure natural via gRPC streaming. La virtualización de la lista (react-window) y el singleton AnkClient son optimizaciones correctas.

**Debilidad del happy path:** El paso 2→4 (BFF→Kernel) no tiene test de integración automatizado. Si `kernel.proto` cambia, este flujo puede romperse silenciosamente.

---

### 3.2 Flujo de Autenticación (Citadel 3 Capas)

```
Capa 1 — Login
  Browser → POST /api/auth/login {tenant_id, passphrase}
  BFF → SHA-256(passphrase) → GetSystemStatus con headers Citadel
  Kernel valida → retorna session_key
  BFF → retorna {session_key, tenant_id} al browser
  localStorage persiste {session_key, tenant_id} ✅ (SH-31-001)

Capa 2 — WebSocket
  Browser → ws://bff/ws/chat/{tenant_id}
  Sec-WebSocket-Protocol: <session_key> ✅ (SH-SEC-012)
  BFF extrae session_key del header → forwarda en metadata gRPC

Capa 3 — gRPC
  CitadelInterceptor en ank-server valida cada RPC
  Headers: x-citadel-tenant + x-citadel-key ✅ (bug corregido 2026-04-06)
  Fallo → UNAUTHENTICATED gRPC status → BFF retorna 401 → UI limpia sesión ✅
```

**Observación crítica:** El passphrase viaja como SHA-256 en el transporte BFF→Kernel (OP-003 en AEGIS_CONTEXT). SHA-256 es una función de hash, no una función de derivación de clave. Es resistente a preimagen pero no a fuerza bruta GPU. La documentación lo justifica diciendo que "Argon2id ocurre en el Kernel". Sin embargo, si lo que llega al Kernel es un SHA-256 del passphrase (no el passphrase original), entonces Argon2id en el Kernel está siendo aplicado sobre el hash, no sobre el passphrase. Esto reduce efectivamente la entropía de la clave almacenada en SQLCipher al espacio de SHA-256 outputs. Requiere verificación en el código del Kernel (fuera del scope de esta auditoría documental).

---

### 3.3 Flujo de Agentes IA → Código → Producción

```
Arquitecto IA (este chat)
  → diseña ticket con criterios de aceptación
  → escribe en Aegis-Governance/Tickets/<ID>.md
  → Tavo hace git push

Kernel/Shell Engineer (Claude Code)
  → lee ticket via CLAUDE.md + MCP aegis-nexus
  → implementa cambio en repo legacy (Aegis-ANK / Aegis-Shell)
  → cargo build / npm run build (detección de errores de compilación)
  → NO ejecuta tests localmente
  → NO hace git push (Tavo lo hace)

Tavo
  → revisa diff
  → git add + commit + push

GitHub Actions (SRE Firewall)
  → cargo fmt + clippy + test + audit
  → black + flake8 + npm build
  → shellcheck
  → ❌ falla → notifica
  → ✅ pasa → merge disponible

Release-Please
  → PR automático al superar N commits convencionales
  → merge → tag → GHCR publish / GitHub Releases
```

**Punto ciego identificado:** Entre "Kernel Engineer implementa" y "GitHub Actions valida" existe un gap de integración. El CI valida que el código compila y los unit tests pasan. No valida:
- Que `kernel_pb2.py` está sincronizado con `kernel.proto`
- Que el BFF puede conectarse al Kernel compilado
- Que el flujo de autenticación end-to-end funciona
- Que `models.yaml` tiene el formato correcto para el nuevo código

Históricamente, estos gaps han producido bugs descubiertos en smoke tests manuales (2026-04-06: 3 bugs críticos de alineación protocolar en un solo smoke test).

**Punto ciego secundario:** El `AUDIT_REPORT.md` desactualizado sugiere que la gobernanza no se actualiza sincrónicamente con el código. Los agentes implementan pero no actualizan los documentos maestros. Hay un pattern de deuda de gobernanza acumulada que se resuelve en batches (GOV-PROC tickets).

---

### 3.4 Flujo Siren (Voice)

```
Browser (AudioCapture API)
  → PCM 16kHz → WebSocket ws://bff/ws/siren/{tenant_id}
  → session_key via Sec-WebSocket-Protocol

BFF
  → SirenStream bidireccional gRPC
  → forwarda chunks de audio raw al Kernel

ank-server (AnkSirenService)
  → SirenRouter resuelve motor por tenant_id ✅ (Epic 2604)
  → VAD (Voice Activity Detection) → detecta speech
  → STT (Whisper offline o cloud)
  → Resultado text → CognitiveScheduler como tarea normal
  → TTS via SirenEngine (Voxtral / ElevenLabs / Mock)
  → Audio PCM de vuelta al stream
  → BFF forwarda al browser → Web Audio API

Fallback de 2 niveles: motor preferido → mock → error gRPC ✅
```

**Fortaleza:** La abstracción `SirenEngine` trait + `SirenRouter` del Epic 2604 es correcta. Permite agregar motores TTS sin tocar el servicio gRPC.

**Debilidad:** `ggml-base.bin` (Whisper) debe ser provisto manualmente. Si falta, el test salta gracefully (ANK-918) pero el feature es inoperativo sin ningún indicador en la UI para el usuario. Esto es invisible para el operador nuevo.

---

## 4. DEUDA TÉCNICA PRIORIZADA

| Prioridad | ID | Descripción | Bloquea |
|---|---|---|---|
| 🔴 P0 | — | Definir repo canónico: legacy vs Aegis-Core | Todo el flujo de agentes |
| 🔴 P0 | GOV-PROC-001 | Sincronizar AUDIT_REPORT.md con estado real | Lanzamiento open source |
| 🟠 P1 | — | Test de integración BFF↔Kernel en CI | Calidad post-launch |
| 🟠 P1 | — | Automatizar regeneración stubs Python de proto | Cualquier cambio a proto |
| 🟡 P2 | LIM-001 | LanceDB desactivado — roadmap concreto | Memoria a largo plazo |
| 🟡 P2 | GOV-PROC-002 | Limpiar Aegis-NotebookLM-Bundle | Governance como repo pasivo |
| 🟡 P2 | DT-005 | speed_inv hardcodeado en CognitiveRouter | Routing latency-sensitive |
| 🟢 P3 | — | Unificar carpetas `auditoria/` y `auditorias/` | Orden del repo |
| 🟢 P3 | GOV-PROC-003 | Estandarizar formato de tickets | Onboarding contribuidores |

---

## 5. EVALUACIÓN GLOBAL

| Dimensión | Score | Notas |
|---|---|---|
| Seguridad | 8/10 | Epic 17 completado; mTLS estricto; Citadel sólido. Pendiente verificar SHA-256 como pre-hash. |
| Estabilidad | 9/10 | Zero-Panic enforced; SRE Firewall real; cargo audit limpio. |
| Flujo de código | 6/10 | Gap de integración BFF↔Kernel es el riesgo principal. Sin smoke test automatizado. |
| Arquitectura | 9/10 | ADRs sólidos; separación correcta; CMR es diferenciador real. |
| Gobernanza | 7/10 | Madura para el tamaño del proyecto. AUDIT_REPORT desactualizado daña la imagen. |
| Estado de lanzamiento | 7/10 | Técnicamente listo; operacionalmente hay puntos ciegos que el primer contribuidor externo va a encontrar. |

**Veredicto:** Aegis OS está en condiciones de lanzamiento open source con las correcciones P0 aplicadas. El mayor riesgo no es seguridad (bien cubierta) sino la experiencia del primer contribuidor externo que intente entender el estado real del proyecto y encuentre un `AUDIT_REPORT.md` con vulnerabilidades "abiertas" que en realidad están cerradas, y una transición repo/monorepo sin documentar.

---

*Auditoría generada por Claude Sonnet 4.6 — Arquitecto IA de Aegis OS*  
*Basada en lectura directa de governance docs y estructura de repos via MCP aegis-nexus*  
*No incluye análisis de código fuente (fuera del scope del rol de Arquitecto IA)*

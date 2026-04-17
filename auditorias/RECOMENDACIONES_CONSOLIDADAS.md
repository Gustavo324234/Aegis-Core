# RECOMENDACIONES CONSOLIDADAS — AUDITORÍAS MULTI-MODELO

> **Generado por:** Arquitecto IA (Claude Sonnet 4.6)  
> **Fecha:** 2026-04-16  
> **Fuentes:** big-pickle · claude-sonnet-4-6 · minimax-m2.5-free · Gemini 3 Flash · gpt-oss-120b_free  
> **Criterio de inclusión:** Se consolidaron únicamente recomendaciones accionables, no duplicadas con la deuda técnica ya documentada en `AEGIS_CONTEXT.md` (LIM/DT), y con origen verificable en el código.  
> **Criterio de exclusión:** Recomendaciones que ya están implementadas, que duplican LIM/DT existentes, o que provienen de auditorías que no leyeron el código fuente (gpt-oss-120b_free fue descartada en su mayoría por este motivo).

---

## PRIORIDAD P0 — Bloquea lanzamiento open source

### REC-001 — Sincronizar `AUDIT_REPORT.md` con estado real
**Origen:** claude-sonnet-4-6 (GOV-PROC-001)  
**Área:** `governance/` / Aegis-Governance  
**Problema:** `AUDIT_REPORT.md` muestra hallazgos del Epic 17 como `🔴 OPEN` cuando todos están cerrados desde 2026-03-17. Un contribuidor externo que lo lea creerá que el sistema tiene vulnerabilidades críticas sin resolver.  
**Acción:** Ejecutar GOV-PROC-001 — actualizar cada hallazgo con su ticket de cierre y fecha.  
**Ticket existente:** GOV-PROC-001 (TODO)

---

### REC-002 — Documentar estado de la transición multi-repo → monorepo
**Origen:** claude-sonnet-4-6  
**Área:** `governance/AEGIS_CONTEXT.md`  
**Problema:** `AEGIS_CONTEXT.md` no menciona que `Aegis-Core` es el monorepo activo. Los repos legacy (`Aegis-ANK`, `Aegis-Shell`, etc.) siguen existiendo y tienen CI activo. Un agente nuevo o contribuidor externo no sabe cuál es el repo canónico.  
**Acción:** Agregar una sección explícita en `AEGIS_CONTEXT.md` que declare `Aegis-Core` como repo canónico y los repos legacy como referencia de solo lectura (archivados/privados). Actualizar el briefing de cada agente.  
**Ticket existente:** Ninguno. Requiere ticket nuevo.

---

## PRIORIDAD P1 — Riesgo técnico real post-launch

### REC-003 — Implementar rate limiting en autenticación
**Origen:** big-pickle (DEBT-004)  
**Área:** `kernel/crates/ank-http/src/routes/auth.rs`  
**Problema:** No existe rate limiting en intentos de login ni en conexiones WebSocket. El endpoint `/api/auth/login` es vulnerable a fuerza bruta.  
**Acción:** Implementar rate limiting por IP y por `tenant_id` en el handler de login. Recomendado: `tower_governor` o implementación manual con `DashMap<IpAddr, AtomicU32>` en `AppState`.  
**Impacto:** Seguridad — brute force contra credenciales de tenant.

---

### REC-004 — Limpiar estado `current_running` cuando HAL Runner muere
**Origen:** big-pickle (DEBT-001)  
**Área:** `kernel/crates/ank-core/src/scheduler/mod.rs`  
**Problema:** Si el HAL Runner (goroutine en `main.rs`) muere inesperadamente, el `CognitiveScheduler` nunca recibe `ProcessCompleted` y el proceso queda en estado `running` indefinidamente. El scheduler no encola nuevas tareas mientras hay un proceso marcado como running.  
**Acción:** Agregar un mecanismo de heartbeat o watchdog: si `execution_tx` cierra, el scheduler debe marcar el proceso actual como fallido y retomarlo de la cola.  
**Impacto:** Estabilidad — deadlock silencioso del scheduler en caso de panic del HAL Runner.

---

### REC-005 — Reuse de `CloudProxyDriver` (evitar instancia por request)
**Origen:** big-pickle (DEBT-003)  
**Área:** `kernel/crates/ank-core/src/chal/mod.rs` línea ~239  
**Problema:** `execute_with_decision` crea un nuevo `CloudProxyDriver` por cada request, lo que implica un nuevo cliente HTTP (`reqwest::Client`) por inferencia. `reqwest::Client` tiene un connection pool interno que se pierde en cada instancia.  
**Acción:** Mover `CloudProxyDriver` al `AppState` o como campo del `CognitiveHAL`, inicializado una sola vez. Alternativamente, hacer `CloudProxyDriver` con `Arc<reqwest::Client>` compartido.  
**Impacto:** Performance — overhead de conexión TCP en cada inferencia; el connection pool de reqwest no puede hacer su trabajo.

---

### REC-006 — Reemplazar `std::sync::Mutex` por `tokio::sync::Mutex` en HAL
**Origen:** minimax-m2.5-free  
**Área:** `kernel/crates/ank-core/src/chal/mod.rs` línea ~87  
**Problema:** Usar `std::sync::Mutex` en código async de Tokio puede producir deadlocks si el lock se mantiene a través de un `.await`. Clippy no siempre detecta este patrón.  
**Acción:** Reemplazar por `tokio::sync::Mutex` o `tokio::sync::RwLock` según el patrón de acceso.  
**Impacto:** Estabilidad — deadlock potencial en producción bajo carga concurrente.

---

### REC-007 — Agregar retry logic en `CloudProxyDriver`
**Origen:** big-pickle (DEBT-007)  
**Área:** `kernel/crates/ank-core/src/chal/drivers/cloud.rs`  
**Problema:** Si la API del proveedor LLM retorna un error transitorio (429, 502, 503), el driver falla inmediatamente y la tarea queda en error. No hay reintentos ni backoff.  
**Acción:** Implementar retry con exponential backoff (máximo 3 intentos) para errores HTTP 429/5xx. Recomendado: `tokio::time::sleep` + contador de intentos en el loop del stream. Un circuit breaker (Gemini Flash lo sugirió correctamente) sería el paso siguiente.  
**Impacto:** Resiliencia — cualquier flap de red o rate limit del proveedor resulta en error visible para el usuario.

---

### REC-008 — Verificar SHA-256 como pre-hash antes de Argon2id
**Origen:** claude-sonnet-4-6 (sección 3.2)  
**Área:** `kernel/crates/ank-core/src/enclave/master.rs` + `kernel/crates/ank-http/src/citadel.rs`  
**Problema:** Si el passphrase llega al kernel ya como SHA-256, y el kernel aplica Argon2id sobre ese hash, entonces Argon2id protege un hash de 256 bits fijo, no el passphrase original. Esto no reduce la seguridad en la práctica (SHA-256 tiene 2^256 posibles outputs), pero es una inconsistencia arquitectónica que debe verificarse y documentarse explícitamente.  
**Acción:** Verificar en `authenticate_tenant` qué recibe exactamente: ¿el plaintext o el hash? Documentar el resultado en `AEGIS_CONTEXT.md` sección 4 (Protocolo Citadel). Si hay un bug, corregirlo. Si es intencional, justificarlo con un comentario `// SECURITY:` en el código.  
**Impacto:** Seguridad — no es un CVE inmediato, pero es una ambigüedad en la capa más sensible del sistema.

---

## PRIORIDAD P2 — Deuda técnica que acumula interés

### REC-009 — Implementar preemption real en el Scheduler
**Origen:** big-pickle (DEBT-002)  
**Área:** `kernel/crates/ank-core/src/scheduler/mod.rs` — handler de `PreemptCurrent`  
**Problema:** `handle_event(PreemptCurrent)` está marcado como TODO y no interrumpe la inferencia en curso. Una tarea de prioridad crítica debe esperar a que termine la tarea actual, sin importar su prioridad.  
**Acción:** Implementar cancellation via `tokio_util::sync::CancellationToken`. El HAL Runner debe verificar el token periódicamente (entre chunks del stream).  
**Impacto:** Fairness — el scheduler de prioridades no funciona como tal mientras no haya preemption.

---

### REC-010 — Crear un roadmap concreto para LanceDB
**Origen:** claude-sonnet-4-6, minimax-m2.5-free (LIM-001)  
**Área:** `kernel/crates/ank-core/` — VCM L3  
**Problema:** LanceDB lleva desactivado "por conflictos de compilación" sin fecha de resolución ni ticket activo. Sin L3, el VCM opera solo con L1/L2 (contexto finito). Para un "sistema operativo cognitivo", la memoria a largo plazo es una feature fundamental, no opcional.  
**Acción:** Crear un ticket con criterios técnicos específicos para resolver el conflicto (auditar las dependencias en conflicto con `cargo tree`, evaluar si hay alternativas como `qdrant-client` embebido o `usearch`). Establecer una fecha tentativa post-launch.  
**Nota:** No está incluido en LIM/DT con plan concreto — solo como "post-launch" indefinido.

---

### REC-011 — Automatizar regeneración de stubs Python cuando `kernel.proto` cambie
**Origen:** claude-sonnet-4-6 (sección 2.3)  
**Área:** CI / `.github/workflows/`  
**Problema:** Los stubs `kernel_pb2.py` y `kernel_pb2_grpc.py` deben regenerarse manualmente cuando cambia `kernel.proto`. No hay gate en CI que detecte divergencia. Históricamente produjo 3 bugs críticos en un solo smoke test (2026-04-06).  
**Acción:** Agregar un step en CI que regenere los stubs y falle si el diff no está commiteado. Alternativamente, documentar en `CONTRIBUTING.md` como paso obligatorio antes de push cuando se modifica el proto.  
**Impacto:** Calidad — cualquier cambio al contrato gRPC puede romper silenciosamente la integración.

---

### REC-012 — Agregar telemetría de tokens por segundo y costo estimado
**Origen:** Gemini 3 Flash  
**Área:** `kernel/crates/ank-http/src/routes/` (endpoint `/api/status`)  
**Problema:** Las métricas actuales del kernel no incluyen throughput de inferencia (tokens/seg) ni costo estimado por sesión. Útil para operadores y para el routing del `CognitiveRouter`.  
**Acción:** Agregar `tokens_per_second` y `estimated_cost_usd` al `SystemMetrics`. El costo puede calcularse con las tablas de precios por modelo del catálogo. Los TPS se miden dividiendo tokens emitidos / tiempo de inferencia por PCB.  
**Impacto:** Operabilidad + mejora del scoring del CognitiveRouter (reemplaza parcialmente `speed_inv` hardcodeado — ver REC-013).

---

### REC-013 — Reemplazar `speed_inv = 0.5` hardcodeado en `CognitiveRouter`
**Origen:** big-pickle (DEBT-005), claude-sonnet-4-6 (sección 2.5)  
**Área:** `kernel/crates/ank-core/src/router/mod.rs` línea ~180  
**Problema:** El factor de velocidad en el scoring 40/30/20/10 usa `speed_inv = 0.5` constante, haciendo que el 20% del peso dedicado a latencia sea efectivamente neutro. El router no puede diferenciar entre modelos lentos y rápidos.  
**Acción:** Medir latencia real (P95) por modelo y proveedor, almacenarla en el `ModelCatalog` como campo `avg_latency_ms`, y usarla en `compute_score()`. Como interim, al menos variar el valor por proveedor en el catálogo bundled.  
**Dependencia:** REC-012 (métricas de TPS facilitan esta medición).

---

### REC-014 — Generar documentación OpenAPI/Swagger para los endpoints HTTP
**Origen:** Gemini 3 Flash, gpt-oss-120b_free  
**Área:** `kernel/crates/ank-http/`  
**Problema:** No hay documentación de API autogenerada. Los contribuidores externos deben leer el código Axum para entender los endpoints disponibles.  
**Acción:** Integrar `utoipa` + `utoipa-axum` para generar `/api/docs` con Swagger UI. Las anotaciones `#[utoipa::path]` se agregan incrementalmente sin refactoring.  
**Impacto:** Onboarding de contribuidores y adopción del proyecto open source.

---

## PRIORIDAD P3 — Orden y mantenibilidad

### REC-015 — Unificar carpeta de auditorías
**Origen:** claude-sonnet-4-6 (sección 2.4)  
**Área:** `Aegis-Core/auditorias/`  
**Problema:** Este archivo ya fue creado en `auditorias/` (plural). Verificar que no existe una carpeta `auditoria/` (singular) con archivos sueltos. Si existe, moverlos aquí.  
**Acción:** `git mv auditoria/* auditorias/ && rmdir auditoria` si aplica.

---

### REC-016 — Documentar la limitación de `ggml-base.bin` en la UI
**Origen:** claude-sonnet-4-6 (sección 3.4)  
**Área:** `shell/ui/` — componente Siren / settings  
**Problema:** Si `ggml-base.bin` (Whisper) no está presente, el STT falla silenciosamente (el test salta con `graceful skip`). El usuario no recibe ningún indicador en la UI de que la funcionalidad de voz está inoperativa por falta del modelo.  
**Acción:** Agregar un endpoint de health que exponga el estado de los modelos locales. En la UI, mostrar un badge o tooltip en el botón de micrófono cuando el STT no está disponible.

---

### REC-017 — Limpiar `Aegis-NotebookLM-Bundle` de Governance
**Origen:** claude-sonnet-4-6 (sección 2.7)  
**Área:** `Aegis-Governance/Aegis-NotebookLM-Bundle/`  
**Problema:** Governance contiene código Python ejecutable y snapshots estáticos de código de producción (`AegisAnkCode.txt`, `AegisShellCode.txt`) que pueden quedar desactualizados.  
**Acción:** Ejecutar GOV-PROC-002 — automatizar el bundle como GitHub Action o eliminar los snapshots estáticos.  
**Ticket existente:** GOV-PROC-002 (TODO)

---

## Resumen ejecutivo

| # | Recomendación | Prioridad | Área | Origen |
|---|---|---|---|---|
| REC-001 | Sincronizar AUDIT_REPORT.md | P0 | Governance | claude-sonnet-4-6 |
| REC-002 | Documentar transición monorepo | P0 | AEGIS_CONTEXT | claude-sonnet-4-6 |
| REC-003 | Rate limiting en login | P1 | ank-http/auth | big-pickle |
| REC-004 | Limpiar `current_running` en scheduler | P1 | ank-core/scheduler | big-pickle |
| REC-005 | Reuse de CloudProxyDriver | P1 | ank-core/chal | big-pickle |
| REC-006 | `tokio::sync::Mutex` en HAL | P1 | ank-core/chal | minimax |
| REC-007 | Retry logic en CloudProxyDriver | P1 | ank-core/drivers | big-pickle |
| REC-008 | Verificar SHA-256 pre-hash | P1 | enclave/citadel | claude-sonnet-4-6 |
| REC-009 | Preemption real en Scheduler | P2 | ank-core/scheduler | big-pickle |
| REC-010 | Roadmap concreto para LanceDB | P2 | ank-core/VCM | claude-sonnet-4-6 |
| REC-011 | Regenerar stubs Python en CI | P2 | CI / proto | claude-sonnet-4-6 |
| REC-012 | Telemetría TPS y costo estimado | P2 | ank-http/status | Gemini |
| REC-013 | Reemplazar `speed_inv` hardcodeado | P2 | ank-core/router | big-pickle |
| REC-014 | Documentación OpenAPI/Swagger | P2 | ank-http | Gemini / gpt |
| REC-015 | Unificar carpeta auditorías | P3 | repo estructura | claude-sonnet-4-6 |
| REC-016 | Indicador UI para STT inoperativo | P3 | shell/ui | claude-sonnet-4-6 |
| REC-017 | Limpiar NotebookLM-Bundle | P3 | Governance | claude-sonnet-4-6 |

---

*Archivo generado por Arquitecto IA — Aegis OS*  
*No implementar directamente — cada REC requiere ticket individual antes de dispatch a ingenieros*

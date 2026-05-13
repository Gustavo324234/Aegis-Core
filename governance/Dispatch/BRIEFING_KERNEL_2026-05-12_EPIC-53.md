# BRIEFING — Kernel Engineer
## EPIC 53 — Stabilization: Fase 1 + Fase 2 (Kernel)
**Fecha:** 2026-05-12
**Branch base:** `feat/epic-53-stabilization-kernel`

---

## Contexto

Primera sesión real de uso productivo de Aegis detectó bugs críticos en el
pipeline multi-agente y en el aislamiento de datos entre tenants. Este briefing
cubre todos los tickets de kernel del EPIC 53, ordenados por dependencia.

**Orden obligatorio de implementación:** CORE-300 → CORE-298 → CORE-262 → CORE-263 → CORE-253 → CORE-257 → CORE-256

Cada ticket va en su propio branch y PR. No acumular en un branch único.

---

## CORE-300 — Aislamiento cross-tenant en ProjectRegistry
**Branch:** `fix/core-300-project-registry-tenant-isolation`
**Prioridad:** CRÍTICA — implementar primero

El tenant `Sole` ve el proyecto `aegis-core` que pertenece al tenant `Tavo`.
El `ProjectRegistry` no particiona proyectos por `tenant_id`.

### Cambios

**1. Particionar el registry por tenant_id**

```rust
// Antes (probable):
projects: HashMap<ProjectId, Project>

// Después:
projects: HashMap<TenantId, HashMap<ProjectId, Project>>
```

**2. Todos los métodos reciben `tenant_id`**

```rust
pub fn get_active_projects(&self, tenant_id: &str) -> Vec<&Project>
pub fn get_project(&self, tenant_id: &str, project_id: &str) -> Option<&Project>
pub fn create_project(&mut self, tenant_id: &str, project: Project)
pub fn list_projects(&self, tenant_id: &str) -> Vec<&Project>
```

**3. `get_agent_status` tool — filtrar por tenant del PCB activo**

La herramienta solo debe retornar proyectos y agentes del `tenant_id` del PCB
que origina la llamada.

**4. Verificar persistencia**

Al restaurar `agent_tree.json` al arranque, verificar que cada nodo tiene su
`tenant_id` y no se mezclan tenants en la restauración.

### Criterios
- [ ] `get_agent_status` de tenant `Sole` no retorna proyectos de tenant `Tavo`
- [ ] `get_agent_status` de tenant `Tavo` no retorna proyectos de tenant `Sole`
- [ ] Crear proyecto en tenant A no lo hace visible en tenant B
- [ ] El aislamiento persiste tras reinicio del servidor
- [ ] `cargo build --workspace` pasa

### Commit
```
fix(ank-core): CORE-300 tenant isolation in ProjectRegistry — partition by tenant_id
```

---

## CORE-298 — Dispatch post-spawn falla: No channel for agent
**Branch:** `fix/core-298-agent-channel-lifecycle`
**Prioridad:** CRÍTICA — implementar segundo

El supervisor se crea correctamente pero cuando el Chat Agent intenta despacharle
una tarea inmediatamente después del spawn, el canal ya no existe:
```
WARN: CORE-264: dispatch post-spawn falló: No channel for agent <uuid>
```

El agente termina antes de que llegue el mensaje porque no tiene LLM real
(CORE-262 pendiente) y cierra el canal.

### Cambios

**1. `orchestrator.rs` — mantener canal activo en estado Idle**

El `spawn_loop` no debe terminar hasta recibir `Shutdown` explícito:

```rust
AgentState::Idle => {
    match rx.recv().await {
        Some(msg) => { /* procesar */ }
        None => break, // canal cerrado externamente → terminar
    }
}
```

**2. `orchestrator.rs` — detectar canal muerto al reusar agente**

```rust
if let Some(sender) = channels.get(&agent_id) {
    if sender.is_closed() {
        // Canal muerto — respawnear
        self.respawn_agent(agent_id).await?;
    }
}
```

**3. `chal/mod.rs` — retry con backoff en dispatch post-spawn**

```rust
let mut attempts = 0;
loop {
    match orchestrator.dispatch_to_agent(agent_id, task.clone()).await {
        Ok(_) => break,
        Err(_) if attempts < 3 => {
            attempts += 1;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err(e) => {
            warn!("dispatch post-spawn falló tras {} intentos: {}", attempts, e);
            break;
        }
    }
}
```

### Criterios
- [ ] Crear supervisor + dispatch inmediato entrega la tarea sin WARN
- [ ] El supervisor permanece activo entre spawn y primer dispatch
- [ ] Segundo dispatch a agente "already active" también funciona
- [ ] No hay `No channel for agent` en logs durante uso normal
- [ ] `cargo build --workspace` pasa

### Commit
```
fix(ank-core): CORE-298 keep agent channel alive until explicit shutdown, retry dispatch post-spawn
```

---

## CORE-262 — AgentOrchestrator: inferencia LLM real en run_agent_loop
**Branch:** `feat/core-262-agent-llm-inference`
**Prioridad:** CRÍTICA — implementar tercero

Ver ticket completo en `governance/Tickets/CORE-262.md` — tiene el código
detallado. Resumen de los 4 cambios:

1. Agregar `hal: Arc<CognitiveHAL>` al struct `AgentOrchestrator`
2. Actualizar `main.rs` para pasar el HAL al crear el orchestrator
3. Agregar método `execute_agent_loop` al `CognitiveHAL` en `chal/mod.rs`
4. Refactorizar arm `Dispatch` en `run_agent_loop` para llamar al LLM real

### Criterios
- [ ] Los supervisores llaman al LLM con su system prompt y la tarea recibida
- [ ] El reporte al padre contiene la respuesta real del LLM
- [ ] `AgentOrchestrator::new` recibe `Arc<CognitiveHAL>`
- [ ] `cargo build --workspace` pasa

### Commit
```
feat(ank-core): CORE-262 AgentOrchestrator — real LLM inference in run_agent_loop via CognitiveHAL
```

---

## CORE-263 — Herramienta ask_user + estado WaitingUser
**Branch:** `feat/core-263-ask-user-tool`
**Prioridad:** Alta — implementar cuarto

Los agentes no tienen forma de pedir input al usuario cuando lo necesitan.
Sin esto, los supervisores hacen suposiciones o se bloquean silenciosamente.

### Cambios

**1. Nueva tool `ask_user` en el ToolRegistry de agentes**

```rust
// Cuando el agente llama ask_user(question: String):
// 1. El agente entra en estado WaitingUser
// 2. El mensaje llega al Chat Agent vía AgentMessage::Report con metadata type=ask_user
// 3. El Chat Agent lo muestra al usuario como pregunta
// 4. La respuesta del usuario se despacha de vuelta al agente como Dispatch
```

**2. Nuevo estado `WaitingUser` en `AgentState`**

```rust
pub enum AgentState {
    Idle,
    Running,
    WaitingUser { question: String }, // nuevo
    Complete,
    Failed(String),
}
```

**3. Enrutamiento en el Chat Agent**

Cuando el Chat Agent recibe un Report con `type=ask_user`, en lugar de
mostrarlo como resultado lo presenta al usuario como pregunta pendiente
e incluye el `agent_id` en el contexto para poder devolver la respuesta.

### Criterios
- [ ] Un agente puede llamar `ask_user` y quedar en estado `WaitingUser`
- [ ] El usuario ve la pregunta en el chat con indicador visual de que viene de un agente
- [ ] La respuesta del usuario llega al agente correcto como nuevo `Dispatch`
- [ ] `cargo build --workspace` pasa

### Commit
```
feat(ank-core): CORE-263 ask_user tool + WaitingUser state + bottom-up routing via Chat Agent
```

---

## CORE-253 — SYS_CALL_PLUGIN: error legible cuando plugin no encontrado
**Branch:** `fix/core-253-plugin-error-message`
**Prioridad:** CRÍTICA

Cuando un agente llama a un plugin que no existe, el kernel devuelve un error
técnico interno. El usuario ve algo críptico o no ve nada.

Localizar el handler de `SYS_CALL_PLUGIN` y asegurar que cuando el plugin
no está registrado, retorna un string legible:

```rust
Err(_) => "El plugin '{name}' no está instalado o no está activo en este tenant. \
            Podés activarlo desde Configuración → Plugins.".to_string()
```

### Criterios
- [ ] Llamar a un plugin inexistente devuelve mensaje legible al usuario
- [ ] El error aparece en el chat, no solo en logs
- [ ] `cargo build --workspace` pasa

### Commit
```
fix(ank-core): CORE-253 SYS_CALL_PLUGIN returns user-friendly error when plugin not found
```

---

## CORE-257 — Tunnel Manager: no reintentar si cloudflared no instalado
**Branch:** `fix/core-257-tunnel-manager-no-retry`
**Prioridad:** Media

El TunnelManager reintenta indefinidamente aunque `cloudflared` no esté
instalado, generando ruido en logs.

Agregar detección de binario antes del primer intento:

```rust
if !std::process::Command::new("cloudflared")
    .arg("--version")
    .output()
    .map(|o| o.status.success())
    .unwrap_or(false)
{
    info!("TunnelManager: cloudflared not installed — tunnel disabled");
    return; // No reintentar
}
```

### Criterios
- [ ] Sin cloudflared instalado: un solo log INFO y no más reintentos
- [ ] Con cloudflared instalado: comportamiento sin cambios
- [ ] `cargo build --workspace` pasa

### Commit
```
fix(ank-core): CORE-257 TunnelManager — skip retry loop if cloudflared binary not found
```

---

## CORE-256 — Admin: endpoint de gestión del servicio
**Branch:** `feat/core-256-service-management-endpoint`
**Prioridad:** Alta

Nuevo endpoint para que la UI pueda gestionar el servicio systemd/proceso:

```
GET  /api/system/service/status  → { status: "running"|"stopped", uptime_secs, pid }
POST /api/system/service/restart → reinicia el proceso (graceful)
POST /api/system/service/stop    → detiene el servicio
```

El Shell Engineer implementa la UI en paralelo (CORE-256 compartido).
Coordinar el schema del response antes de implementar.

### Criterios
- [ ] `GET /api/system/service/status` retorna estado real del proceso
- [ ] `POST /api/system/service/restart` hace graceful restart
- [ ] Los endpoints requieren autenticación de admin (header Citadel)
- [ ] `cargo build --workspace` pasa

### Commit
```
feat(ank-http): CORE-256 service management endpoints — status, restart, stop
```

---

## CORE-225 + CORE-213 — Cleanup
**Branch:** `chore/core-225-213-cleanup`
**Prioridad:** Alta / Media

**CORE-225:** Cambiar el campo `license` en todos los `Cargo.toml` del workspace
a `"MIT"` si aún no está establecido.

**CORE-213:** En `key_pool.rs` o donde se llame `key_pool.load()` al arranque,
agregar log de error si falla:

```rust
if let Err(e) = key_pool.load().await {
    tracing::error!("key_pool.load() failed at startup: {}", e);
}
```

### Commits
```
chore(cargo): CORE-225 set license = "MIT" in all workspace Cargo.toml
fix(ank-core): CORE-213 log error when key_pool.load() fails at startup
```

---

## CORE-292 — Provider ollama_cloud
**Branch:** `feat/core-292-ollama-cloud-provider`
**Prioridad:** Alta — puede ir en paralelo con los anteriores

Ver ticket completo en `governance/Tickets/CORE-292.md`.

Nuevo provider `ollama_cloud` con URL remota configurable y allowlist SSRF
para evitar que apunte a IPs internas.

### Commit
```
feat(ank-core): CORE-292 ollama_cloud provider — remote URL + SSRF allowlist
```

---

**No correr tests. No pushear a main. Un PR por ticket.**

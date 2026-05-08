# BRIEFING — Kernel: WebSocket keepalive + deduplicación supervisores + Caddy

**Fecha:** 2026-05-07  
**Para:** Kernel Engineer (Claude Code)  
**Tickets:** CORE-279, CORE-281, CORE-280

---

## Prerequisito

Leer los tres tickets antes de implementar:
- `governance/Tickets/CORE-279.md`
- `governance/Tickets/CORE-281.md`
- `governance/Tickets/CORE-280.md`

---

## Branch

```
fix/core-279-281-280-keepalive-dedup-https
```

---

## Orden de implementación

### 1. CORE-279 — WebSocket ping keepalive (15 minutos)

**Archivo:** `kernel/crates/ank-http/src/ws/chat.rs`

Agregar `ping_interval` con `tokio::time::interval(30s)` al loop principal
de `handle_chat`. El arm del `tokio::select!` envía `Message::Ping(vec![])`
y hace `continue`. Agregar `Message::Pong(_) => continue` en el match de mensajes.

Ver código completo en `governance/Tickets/CORE-279.md`.

---

### 2. CORE-281 — Deduplicación + project_name en system prompt (30 minutos)

**Archivo:** `kernel/crates/ank-core/src/agents/orchestrator.rs`

Dos cambios:

**A.** En el arm `spawn_agent` para `project_supervisor`: antes de crear, buscar
en el árbol si ya existe un `ProjectSupervisor` con el mismo nombre en estado
no-terminal. Si existe, retornar `{"status":"already_active","agent_id":"..."}`.

**B.** Al construir los messages del LLM para un `ProjectSupervisor`, prepend
el header `[PROJECT]\nName: ...\nProject ID: ...\n` al system prompt.

**Archivo:** `kernel/config/agents/project_supervisor.md`

Agregar al inicio: instrucción explícita de que el proyecto ya está en el contexto
y no debe preguntar al usuario cuál es.

Ver código completo en `governance/Tickets/CORE-281.md`.

---

### 3. CORE-280 — Caddy HTTPS en el installer (45 minutos)

**Archivo:** `installer/install.sh`

Cuatro cambios en orden:

1. En `show_main_menu()`: agregar pregunta de HTTPS después de la selección de modo.
   Si elige HTTPS, pedir dominio y email. Mostrar requisitos (IP pública, puertos 80/443)
   y pedir confirmación antes de continuar.

2. Nueva función `install_caddy()`: instala Caddy via repositorio oficial de Caddy.

3. Nueva función `configure_caddy(domain, email)`: escribe `/etc/caddy/Caddyfile`
   con reverse proxy a `localhost:8000`. Habilita y reinicia Caddy.

4. En `install_native()`: si `SETUP_HTTPS=true`, llamar `install_caddy` y
   `configure_caddy` en lugar de `install_cloudflared`. Si `SETUP_HTTPS=false`,
   comportamiento actual sin cambios.

5. En `wait_and_show()`: mostrar URL HTTPS si está configurado.

Ver código completo en `governance/Tickets/CORE-280.md`.

---

## Verificación

```bash
cargo build --workspace          # para CORE-279 y CORE-281
bash -n installer/install.sh     # syntax check para CORE-280
shellcheck installer/install.sh  # lint (debe pasar sin errores SC2059)
```

---

## Commit y PR

**Commit message:**
```
fix(ank-http,agents,installer): CORE-279/281/280 WS keepalive, supervisor dedup, Caddy HTTPS
```

**PR title:**
```
fix: CORE-279/281/280 — WebSocket keepalive + supervisor dedup + Caddy HTTPS installer
```

**PR description:**
```
## CORE-279 — WebSocket keepalive
- Ping cada 30s en handle_chat para prevenir cierre por proxies idle
- Pong ignorado en el loop

## CORE-281 — Deduplicación de supervisores + project_name
- spawn_agent verifica supervisor existente antes de crear uno nuevo
- Header [PROJECT] inyectado en system prompt del ProjectSupervisor
- project_supervisor.md: instrucción explícita de no preguntar el nombre del proyecto

## CORE-280 — Caddy HTTPS en installer
- Opción de HTTPS en el menú del installer
- Instalación y configuración automática de Caddy con Let's Encrypt
- Si HTTPS activo: cloudflared no se instala
- Comportamiento actual preservado si el usuario elige sin HTTPS

## Verificación
cargo build --workspace ✅
shellcheck installer/install.sh ✅
```

**Target branch:** `main`

---

*Briefing creado por Arquitecto IA — 2026-05-07*

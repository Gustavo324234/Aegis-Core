# DISPATCH_EPIC_41.md — Plan de despacho Epic 41

> **Fecha:** 2026-04-22
> **Estado:** CORE-145 ✅ CORE-146 ✅ — Pendientes: CORE-147 y CORE-148

---

## PENDIENTE — 2 tickets, ambos al Kernel Engineer en paralelo

```
[Kernel]  CORE-147  Fix installer: eliminar TLS self-signed, instalar cloudflared
[Kernel]  CORE-148  Fix system prompt: tono natural, sin respuestas robóticas
```

---

## 🔴 CORE-147 — Fix installer TLS → Cloudflare

```
Sos el Kernel Engineer de Aegis Core.

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-147.md")
2. read_file("Aegis-Core", "installer/install.sh")
3. read_file("Aegis-Core", "installer/aegis")

CONTEXTO: install_cloudflared() ya existe en install.sh. Lo que falta:
- Eliminar setup_tls_automatic() y todas las referencias a ENABLE_TLS, cert.pem, key.pem
- El servidor sirve HTTP puro — Cloudflare Tunnel provee HTTPS externamente
- En wait_and_show(): PROTOCOL siempre "http"
- En el env file: no escribir AEGIS_TLS_CERT ni AEGIS_TLS_KEY
- En aegis CLI: eliminar tls-regen del case y help
- En ank-server/main.rs: log informativo "serving HTTP on :8000, use cloudflared for HTTPS"

GATE:
  shellcheck installer/install.sh installer/aegis
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

FLUJO GIT:
1. git checkout -b fix/core-147-remove-tls-use-cloudflare
2. git commit -m "fix(installer,ank-server): CORE-147 remove self-signed TLS — Cloudflare tunnel for HTTPS"
3. git push origin fix/core-147-remove-tls-use-cloudflare
4. Reportar PR

AL TERMINAR: Marcar CORE-147 como [DONE] en governance/TICKETS_MASTER.md

TAREA: Implementar el ticket CORE-147. Lee el ticket completo antes de empezar.
```

---

## ⚡ CORE-148 — Fix system prompt

```
Sos el Kernel Engineer de Aegis Core.

PROTOCOLO DE INICIO:
1. read_file("Aegis-Core", "governance/Tickets/CORE-148.md")
2. read_file("Aegis-Core", "kernel/crates/ank-core/src/chal/mod.rs")

CONTEXTO: CORE-145 ya está implementado — el onboarding pregunta nombre y
personalidad en el chat y guarda la Persona via set_persona(). El SYSTEM_PROMPT_MASTER
base NO debe tener nombre ni personalidad — solo reglas de comportamiento y tono.
La personalidad se inyecta dinámicamente via PERSONA_SECTION_TEMPLATE.

GATE:
  cargo fmt --all
  cargo build --workspace
  cargo clippy --workspace -- -D warnings -D clippy::unwrap_used -D clippy::expect_used

FLUJO GIT:
1. git checkout -b fix/core-148-system-prompt
2. git commit -m "fix(ank-core): CORE-148 system prompt — natural tone, no hardcoded personality"
3. git push origin fix/core-148-system-prompt
4. Reportar PR

AL TERMINAR: Marcar CORE-148 como [DONE] en governance/TICKETS_MASTER.md

TAREA: Implementar el ticket CORE-148. Lee el ticket completo antes de empezar.
```

---

*Actualizado: 2026-04-22 — CORE-145 y CORE-146 completados por Tavo*

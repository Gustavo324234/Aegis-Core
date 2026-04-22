# TICKETS_MASTER.md — Aegis Core

> Fuente de verdad única para todos los tickets del monorepo Aegis-Core.

---

## 🏗️ EPIC 32–40: Foundation → Connected Accounts — DONE ✅
Ver historial de tickets CORE-001 a CORE-143.

---

## 🛠️ Maintenance & Technical Debt

*   **[CORE-144]** Security: `rustls-pemfile` unmaintained (RUSTSEC-2025-0134) `[BLOCKED — upstream axum-server]`

---

## 🎯 EPIC 41: UX, Onboarding & Reliability
**Status:** IN PROGRESS — 2026-04-22

### ADR-044
> El onboarding de Persona ocurre en el chat, no en Settings.
> El agente pregunta nombre y estilo al primer mensaje.
> Settings → Identidad permite editarla después.

### ADR-045
> La conexión de la app mobile usa QR generado en la Shell web.
> El QR contiene la URL del tunnel de Cloudflare si está activo,
> o la IP local si no. Sin tipeo manual de IPs.

### ADR-046
> `aegis update` siempre verifica y regenera el certificado TLS si no existe
> o vence en menos de 30 días. Nuevo comando `aegis tls-regen` para
> regeneración manual sin update completo.

---

### Tickets

*   **[CORE-145]** Feature: Onboarding conversacional de Persona en el chat — nombre y estilo `[DONE — Shell Engineer + Kernel Engineer]`
*   **[CORE-146]** Feature: Conexión app por QR + acceso remoto via Cloudflare tunnel `[DONE — Kernel Engineer + Shell Engineer]`
*   **[CORE-147]** Fix: TLS no levanta tras `aegis update` — regeneración automática `[DONE — Kernel Engineer]`

### Orden de implementación
1. **CORE-147** — Fix crítico, autónomo, despachar primero
2. **CORE-145** — Onboarding en chat (Kernel + Shell en paralelo)
3. **CORE-146** — QR + tunnel (depende de que el servidor esté estable)

---

## 🔮 EPIC 33: Linux Distribution — PLANNED post-producción

---

## 🚀 SISTEMA STATUS — 2026-04-22

| Componente | Estado |
|---|---|
| Epic 32–40 | ✅ DONE |
| CORE-144: rustls-pemfile | ⏳ BLOCKED upstream |
| Epic 41: UX & Reliability | 🔄 IN PROGRESS — 2/3 |
| TLS en producción | ✅ SOLUCIONADO (CORE-147) |
| Onboarding Persona | ✅ IMPLEMENTADO KERNEL (CORE-145) |
| Conexión app por QR | ❌ NO IMPLEMENTADO — CORE-146 |

**Tickets pendientes: 1**
**Próximo: CORE-146**

---

*Última actualización: 2026-04-22 — Epic 41 creada.*

# Sponsor Page — Aegis OS

> **Contenido para configurar en GitHub Sponsors**
> Copiar en la descripción del perfil de sponsor cuando lo habilites.

---

## Descripción corta (para el perfil de GitHub Sponsors)

```
Building Aegis OS — a self-hosted cognitive operating system where AI agents run
as kernel-level processes. One Rust binary, no runtime dependencies, multi-tenant,
open source. Solo developer. Sponsorships go toward compute, API costs, and tooling.
```

---

## Descripción larga (para la página de sponsor)

### What is Aegis OS?

Aegis is a self-hosted cognitive operating system — a platform where AI agents run
as first-class kernel processes, with memory, scheduling, multi-tenancy, and tool
execution built in at the system level.

It is not a chatbot wrapper or a LangChain pipeline. It is a kernel-level runtime
for autonomous cognitive workloads, written in Rust, running as a single binary
with no external runtime dependencies.

**What makes it different:**
- LLMs treated as ALUs (compute units) under a deterministic scheduler — not oracles
- Hierarchical multi-agent orchestration with per-agent memory and context budgets
- Zero-Trust multi-tenant auth (Citadel Protocol) — multiple users, fully isolated
- Developer Workspace with integrated terminal, Git, and GitHub PR manager
- One-line install on any Linux server: `curl | sudo bash`

### Who builds this?

Solo developer. Gustavo Aversente, based in Córdoba, Argentina.
I've been building Aegis full-time, funded entirely out of pocket.

### What do sponsorships pay for?

| Item | Monthly cost |
|---|---|
| Claude Pro (architecture & code review AI) | $20 USD |
| API costs (OpenRouter, Anthropic — dogfooding Aegis itself) | ~$30 USD |
| VPS for CI and testing | ~$10 USD |
| **Total** | **~$60 USD/month** |

A better development machine is also on the roadmap — the current one limits
local model testing and build times significantly.

### Tiers (suggested)

| Tier | Amount | What it means |
|---|---|---|
| ☕ Coffee | $5/month | You keep the lights on |
| 🔧 Tooling | $15/month | Covers a month of API costs |
| 🚀 Sustainer | $30/month | Half a month of full infrastructure |
| 🏗️ Builder | $60/month | Full month of development infrastructure |
| 🌟 Patron | $100+/month | Named in CHANGELOG and README |

---

## Instrucciones para activar GitHub Sponsors

1. Ir a **github.com/Gustavo324234** → Settings → Sponsors
2. Completar el formulario de activación (requiere cuenta Stripe o cuenta bancaria)
3. Pegar la descripción larga de arriba en el campo "About"
4. Configurar los tiers con los montos sugeridos
5. Una vez aprobado (~1-2 días hábiles), el badge del README queda activo automáticamente

> **Nota:** GitHub Sponsors está disponible para Argentina desde 2023.
> Necesitás una cuenta bancaria para recibir pagos vía Stripe Connect.

---

*Arquitecto IA — 2026-04-28*

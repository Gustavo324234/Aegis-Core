# CORE-063 — governance: AEGIS_CONTEXT.md y AEGIS_MASTER_CODEX.md

**Épica:** 32 — Unified Binary
**Fase:** 7 — CI/CD y Governance
**Repo:** Aegis-Core — `governance/`
**Asignado a:** Arquitecto IA
**Prioridad:** 🟡 Media
**Estado:** TODO
**Depende de:** CORE-020 (para documentar la arquitectura final)

---

## Contexto

Los documentos de governance de Aegis-Core deben reflejar la arquitectura
del monorepo — no son una copia del legacy sino documentos propios.

**Referencia:** `Aegis-Governance/AEGIS_CONTEXT.md` y `AEGIS_MASTER_CODEX.md`

---

## Trabajo requerido

### `governance/AEGIS_CONTEXT.md`

Mapa arquitectónico completo de Aegis-Core:
- Descripción de cada crate (`ank-core`, `ank-http`, `ank-server`, etc.)
- Interfaces públicas (HTTP endpoints + gRPC RPCs)
- Diagrama de relaciones entre módulos
- Estado de cada componente
- ADRs activos (ADR-030 a ADR-033 + los heredados del legacy)
- Problemas conocidos y deuda técnica

### `governance/AEGIS_MASTER_CODEX.md`

Reglas universales para agentes IA en Aegis-Core:
- Mismas reglas SRE del legacy (Zero-Panic, Citadel, etc.)
- Actualizado para arquitectura de binario único
- Rol de cada agente en el monorepo
- Protocolo de tickets para Aegis-Core

---

## Criterios de aceptación

- [ ] `AEGIS_CONTEXT.md` describe correctamente la arquitectura post-Epic 32
- [ ] `AEGIS_MASTER_CODEX.md` tiene reglas coherentes con el monorepo
- [ ] Los documentos no contradicen `ARCHITECTURE.md` ni `CLAUDE.md`
- [ ] Los ADRs están numerados y actualizados

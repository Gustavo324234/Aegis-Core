# BRIEFING — Kernel: token legacy + honestidad chat_agent

**Fecha:** 2026-05-07  
**Para:** Kernel Engineer (Claude Code)  
**Tickets:** CORE-282, CORE-283

---

## Branch

```
fix/core-282-283-legacy-token-honesty
```

---

## Prerequisito

Leer los tickets antes de implementar:
- `governance/Tickets/CORE-282.md`
- `governance/Tickets/CORE-283.md`

---

## Orden de implementación

### 1. CORE-282 — Eliminar token legacy + interceptar fallback (30 min)

**Archivo A:** `kernel/config/agents/chat_agent.md`

Encontrar la sección "When to Spawn" que contiene ejemplos con
`[SYS_AGENT_SPAWN(role="...", name="...", scope="...")]`.

Reemplazar esos ejemplos por instrucciones de tool use nativo.
Agregar al final: `NEVER generate [SYS_AGENT_SPAWN(...)] tokens — use spawn_agent tool directly.`

Ver texto exacto en `governance/Tickets/CORE-282.md`.

**Archivo B:** `kernel/crates/ank-core/src/chal/mod.rs`

Buscar dónde se procesan los tokens de output del LLM antes de emitirlos
al WebSocket (el loop que emite `StreamItem::Token`).

Agregar un regex `LEGACY_SPAWN_RE` y chequeo: si el token contiene
`[SYS_AGENT_SPAWN(...)]`, interceptarlo, convertirlo a `AgentToolCall::Spawn`,
ejecutarlo silenciosamente, y hacer `continue` sin emitir el token al output.

Si el parse falla, descartar el token silenciosamente — nunca emitirlo al usuario.

Ver código completo en `governance/Tickets/CORE-282.md`.

---

### 2. CORE-283 — Honestidad del chat_agent (10 min)

**Archivo:** `kernel/config/agents/chat_agent.md`

Agregar sección "Cuándo NO prometer resultados" después de
"Gestión de Proyectos con Supervisores".

El cambio es solo en el archivo de instrucciones — sin cambios en Rust.

Ver texto exacto en `governance/Tickets/CORE-283.md`.

---

## Verificación

```bash
cargo build --workspace
```

Sin errores. Verificar que `LEGACY_SPAWN_RE` compila como `LazyLock<Regex>` sin warnings.

---

## Commit y PR

**Commit message:**
```
fix(chal,agents): CORE-282/283 interceptar token legacy SYS_AGENT_SPAWN, honestidad chat_agent
```

**PR title:**
```
fix: CORE-282/283 — token legacy interceptado + chat_agent honesto sobre estado de supervisores
```

**PR description:**
```
## CORE-282 — Token legacy [SYS_AGENT_SPAWN(...)]
- chat_agent.md: eliminadas instrucciones de token parser legacy, reemplazadas por tool use nativo
- chal/mod.rs: LEGACY_SPAWN_RE intercepta tokens legacy en el stream de output
- Tokens interceptados se convierten a AgentToolCall::Spawn y se ejecutan silenciosamente
- Si el parse falla, el token se descarta — nunca llega al usuario como texto

## CORE-283 — Honestidad del chat_agent
- chat_agent.md: nueva sección sobre cuándo NO prometer resultados
- El agente no repite promesas de "en breve te comparto" sin tener datos reales
- Responde honestamente cuando el supervisor no retornó información

## Verificación
cargo build --workspace ✅
```

**Target branch:** `main`

---

*Briefing creado por Arquitecto IA — 2026-05-07*

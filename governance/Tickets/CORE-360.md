# CORE-360: Multi-Tenant Key Scoping in KeyPool, Agent Tenant Propagation, and Sticky Cache Key Health Validation

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Asignado a:** Kernel Engineer

## Description
1. `KeyPool::has_key_for_model` and `KeyPool::has_paid_key` in `ank-core/src/router/key_pool.rs` checked `tenants.values().any(...)` across **all** tenants, ignoring `tenant_id`. Consequently, Tenant A's private keys caused `CognitiveRouter::decide` to select providers/models for Tenant B that failed at `resolve_key` (`HardwareFailure("Key became unavailable")`) or routed free-tier tenants onto paid-only models (`limit: 0` HTTP 429).
2. Sticky-routing cache hits in `CognitiveRouter::decide` re-checked circuit breakers (`is_provider_circuit_open`, `is_model_circuit_open`) but did not verify that `decision.key_id` remained active and unthrottled in `KeyPool`, nor did key mutations in `ank-http/src/routes/router_api.rs` invalidate sticky decisions.
3. `AgentOrchestrator::run_agent_loop` constructed `mock_pcb` without propagating `AgentNode.tenant_id`, causing subagent CMR decisions to always resolve against `"default"` instead of the owning tenant's key pool. Additionally, `resolve_backup_tenant_scope` in `router_api.rs` ignored the boolean result of `authenticate_tenant`.

## Acceptance Criteria
- [x] Add tenant-scoped `has_key_for_model_for_tenant`, `has_paid_key_for_tenant`, and `is_key_available_for_tenant` in `KeyPool` and wire them into `CognitiveRouter::decide`.
- [x] Re-validate `decision.key_id` availability on sticky-routing cache hits and invalidate sticky entries when global or tenant keys are added, updated, deleted, or imported.
- [x] Propagate `AgentNode.tenant_id` into `mock_pcb.tenant_id` in `AgentOrchestrator::run_agent_loop` (and use char-safe UTF-8 truncation on `AgentMessage::Query`), and enforce boolean verification in `resolve_backup_tenant_scope`.

# CORE-362: Accurate Per-Model Cooldowns, Key-Specific Fallback Rate-Limiting, and Complete CatalogSyncer Registration

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Asignado a:** Kernel Engineer

## Description
1. `ModelUsageTracker::model_cooldown_remaining_secs` only inspected `empty_responses` and ignored `model_unavailable` events, reporting `None` (`0s` in `/api/router/stats`) when a model's circuit was opened by a deterministic failure (401/403/tier-0), and `decide()` omitted per-model cooldowns from its `"retry in Ns"` error message.
2. In `chal/mod.rs`, HTTP-error key rotation called `get_available_key` instead of `get_available_key_excluding` (racing against the spawned `mark_key_rate_limited` task), and alternate/fallback drivers reused the primary key's `on_rate_limited` closure instead of binding each fallback/rotated key's own `key_id`. `execute_agent_loop` also lacked outcome telemetry and fallback-chain recovery.
3. `ModelCatalog::add_entry` did not deduplicate `model_id`, `ModelCatalog::find`/`update_tool_support` only matched exact strings (missing bare API IDs for prefixed catalog entries), `CatalogSyncer::sync_once` ignored `tenant_keys`, and `POST /api/router/sync` was a no-op stub.

## Acceptance Criteria
- [x] Compute remaining cooldown across both `empty_responses` and `model_unavailable` in `model_cooldown_remaining_secs` and surface per-model cooldowns in `decide()`.
- [x] Add `key_id` to `FallbackDecision`, use `get_available_key_excluding` for HTTP-error key rotation, bind per-key `on_rate_limited` callbacks in `chal/mod.rs`, and wire fallback/telemetry into `execute_agent_loop`.
- [x] Deduplicate `ModelCatalog::add_entry`, support bare/prefixed matching in `find` and `update_tool_support`, sync tenant keys' `active_models` in `CatalogSyncer::sync_once`, and invoke `catalog_syncer.sync_now()` in `POST /api/router/sync`.

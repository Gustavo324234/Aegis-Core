# CORE-356 — Modularización y Deduplicación de `router_api.rs`

- **Tipo:** refactor
- **Estado:** ✅ Done
- **Prioridad:** Media
- **Asignado a:** Kernel Engineer

## Contexto y Problema
`kernel/crates/ank-http/src/routes/router_api.rs` superó las 1,230 líneas mezclando la administración de claves del `KeyPool`, el descubrimiento de modelos (`probe_models`), la exportación/importación cifrada de claves y los endpoints de módulos de dominio del microkernel (`list_modules`, `enable_module`, `execute_module_tool`), además de duplicar la lógica de sincronización inmediata del catálogo y la resolución de autenticación Master/Tenant en backups.

## Solución Implementada
1. Extraer los endpoints de módulos de dominio (`list_modules`, `enable_module`, `execute_module_tool` y sus DTOs) a `kernel/crates/ank-http/src/routes/router_api/modules_handlers.rs`.
2. Extraer la lógica de descubrimiento y sondeo de modelos (`probe_models`, `auto_discover_models`, `is_keyless_provider`, `ProbeModelsRequest`, `ProbeModelsResponse`) a `kernel/crates/ank-http/src/routes/router_api/discovery_handlers.rs`.
3. Consolidar la sincronización de modelos tras alta/edición de claves en `sync_key_models_into_catalog` y la verificación de alcance Master/Tenant para exportación/importación en `resolve_backup_tenant_scope`.

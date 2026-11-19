# BRIEFING — Kernel Engineer: Herramientas Personales y Consolidación Real de Chronos

**Fecha:** 2026-11-09  
**Para:** Kernel Engineer  
**Épica:** EPIC 58 — Personal Assistant Experience, Zero-Friction UX & Community Onboarding  
**Tickets:** `CORE-375`, `CORE-376`, y el endpoint backend de `CORE-377`  
**Branch sugerido:** `feat/epic58-kernel-personal-assistant`

---

## Prerequisito

Leer antes de empezar:
1. `governance/Tickets/CORE-375.md`
2. `governance/Tickets/CORE-376.md`
3. `governance/Tickets/CORE-377.md`
4. Referencia funcional ya validada al 100% en Python: `tools/agent_sandbox/sandbox.py` (`PersonalMemoryStore` y `execute_personal_tool`).

**Regla estricta del repo:** `clippy::unwrap_used` está prohibido en CI. No usar `.unwrap()` ni `.expect()` en código de producción; usar propagación `?` o `unwrap_or_default()`.

---

## Orden de Implementación

### Paso 1: `CORE-375` (Parte A) — Enclave SQLCipher (`kernel/crates/ank-core/src/enclave/mod.rs`)

1. En `TenantDB::init_schema()` (alrededor de la línea 168, justo debajo de `CREATE TABLE IF NOT EXISTS reminders`), agregar la tabla `personal_facts`:
   ```sql
   CREATE TABLE IF NOT EXISTS personal_facts (
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       category TEXT NOT NULL DEFAULT 'general',
       fact TEXT NOT NULL,
       source TEXT NOT NULL DEFAULT 'chat',
       created_at DATETIME DEFAULT CURRENT_TIMESTAMP
   )
   ```
2. Agregar la función pública `pub fn normalize_expense_category(raw_category: &str, description: &str) -> String`:
   - Convertir `raw_category` y `description` a lowercase.
   - Si contiene `"super"`, `"súper"`, `"supermercado"`, `"carniceria"`, `"carnicería"`, `"verduleria"`, `"verdulería"`, `"almacen"`, `"almacén"`, `"restaurante"`, `"comida"`, `"delivery"`, `"panaderia"` $\rightarrow$ retornar `"comida".to_string()`.
   - Si contiene `"nafta"`, `"combustible"`, `"uber"`, `"cabify"`, `"taxi"`, `"sube"`, `"colectivo"`, `"peaje"`, `"estacionamiento"` $\rightarrow$ retornar `"transporte".to_string()`.
   - Si contiene `"luz"`, `"gas"`, `"agua"`, `"internet"`, `"alquiler"`, `"expensas"` $\rightarrow$ retornar `"hogar".to_string()`.
   - Si contiene `"farmacia"`, `"medico"`, `"médico"`, `"dentista"`, `"gimnasio"` $\rightarrow$ retornar `"salud".to_string()`.
   - Si no coincide y `raw_category` no está vacío, retornar `raw_category.trim().to_lowercase()`, o `"general".to_string()`.
3. En `impl TenantDB` (junto a `add_expense` / `get_expenses` en línea 460), implementar:
   - `pub fn add_personal_fact(&self, category: &str, fact: &str, source: &str) -> Result<bool>`: verifica con `SELECT COUNT(*) FROM personal_facts WHERE LOWER(fact) = LOWER(?1)` que no esté duplicado y lo inserta.
   - `pub fn get_personal_facts(&self, limit: u32) -> Result<Vec<serde_json::Value>>`.
   - `pub fn delete_personal_fact(&self, id: i64) -> Result<bool>`.
   - `pub fn query_expenses_summary(&self, category_filter: Option<&str>) -> Result<serde_json::Value>`: calcula `total`, `totals_by_category` (mapa JSON `categoria -> suma`) y `recent_entries` (últimos 15).
   - `pub fn complete_reminder(&self, match_text: &str) -> Result<usize>`: marca `status = 'completed'` donde `CAST(id AS TEXT) = ?1` o `LOWER(description) LIKE ?2`.
   - `pub fn build_personal_memory_prompt_block(&self) -> Result<String>`: formatea los hechos de `get_personal_facts(30)`, el resumen de `query_expenses_summary(None)` y los recordatorios pendientes en un bloque de texto compacto para el system prompt.

---

### Paso 2: `CORE-375` (Parte B) — ToolRegistry y Ejecución en `CognitiveHAL`

1. **Archivo:** `kernel/crates/ank-core/src/agents/tool_registry.rs`
   - En `ToolRegistry::definitions_for(AgentRole::ChatAgent)` (líneas 101-106), agregar:
     - `Self::log_expense_or_entry()`
     - `Self::query_ledger()`
     - `Self::remember_fact()`
     - `Self::manage_reminder()`
   - Definir las 4 funciones constructoras de `ToolDefinition` con los mismos parámetros JSON que en `tools/agent_sandbox/sandbox.py` (líneas 418-505).
   - Actualizar el test en `tool_registry.rs` (`test_chat_agent_gets_four_tools` $\rightarrow$ ahora son 8 tools para `ChatAgent`).

2. **Archivo:** `kernel/crates/ank-core/src/chal/mod.rs`
   - En `execute_tool_call_internal` (línea 2267), agregar los 4 brazos:
     - `"log_expense_or_entry"` $\rightarrow$ normaliza categoría con `normalize_expense_category`, abre `TenantDB::open(&tenant_id, session_key)`, llama a `db.add_expense(amount, description, Some(&cat))` y devuelve `db.query_expenses_summary(Some(&cat))`.
     - `"query_ledger"` $\rightarrow$ abre `TenantDB` y devuelve `db.query_expenses_summary(category_opt)`.
     - `"remember_fact"` $\rightarrow$ abre `TenantDB`, llama a `db.add_personal_fact(category, fact, "chat")` y devuelve JSON `{"status": "remembered", "fact": fact}`.
     - `"manage_reminder"` $\rightarrow$ soporta `"add"` (`db.add_reminder`), `"list"` (`db.get_reminders(20)`), y `"complete"` (`db.complete_reminder`).
   - En la construcción del system prompt del Chat Agent principal (`chal/mod.rs`), si el PCB tiene `tenant_id` y `session_key`, abrir `TenantDB` y anexar `db.build_personal_memory_prompt_block()` al final del system prompt.

3. **Archivo:** `kernel/config/agents/chat_agent.md`
   - Agregar las instrucciones de uso de las 4 herramientas personales instantáneas (`log_expense_or_entry`, `query_ledger`, `remember_fact`, `manage_reminder`) siguiendo `tools/agent_sandbox/prompts/v2_direct/chat_conductor.md`.

---

### Paso 3: `CORE-376` — Consolidación Real en `ChronosDaemon` (`kernel/crates/ank-core/src/chronos/mod.rs`)

1. En `kernel/crates/ank-core/src/chronos/mod.rs` (líneas 81-125):
   - Eliminar `let test_raw_text = "ACTIVITY LOG: ...";`.
   - Actualizar `build_consolidation_pcb(&self, tenant_id: &str, raw_text: &str) -> PCB` para pedir JSON con `{"summary": "...", "tags": [...], "facts": [{"category": "...", "fact": "..."}]}` y asignar `pcb.tenant_id = Some(tenant_id.to_string())`.
   - Agregar test unitario en `chronos::tests` verificando la generación del PCB con `tenant_id`.

---

### Paso 4: Endpoints HTTP para la UI (`CORE-375`, `CORE-376`, `CORE-377`)

1. Crear `kernel/crates/ank-http/src/routes/personal.rs` y montarlo en `kernel/crates/ank-http/src/routes/mod.rs` bajo `/api/personal`:
   - `GET /api/personal/summary`: autenticado con `CitadelAuthenticated`. Abre `TenantDB::open(&auth.tenant_id, &auth.session_key)` y devuelve `{ "facts": ..., "ledger": ..., "reminders": ... }`.
   - `DELETE /api/personal/facts/:id`: elimina un hecho por ID.
2. En `kernel/crates/ank-http/src/routes/auth.rs`:
   - Agregar `POST /api/auth/quick-personal-setup` (`CORE-377`): si `!enclave.admin_exists()` y `ip.is_loopback()`, inicializa el admin maestro, crea el tenant personal sin exigir cambio forzado de contraseña (`must_change_password = false`), guarda la API key en el router si fue provista y devuelve `{ "status": "READY", "tenant_id": username }`.

---

## Verificación Obligatoria antes de finalizar

```bash
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
cargo test -p ank-core --lib
cargo test -p ank-http --lib
```

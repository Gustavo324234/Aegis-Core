# CORE-375: Kernel Personal Assistant Tools (`log_expense_or_entry`, `query_ledger`, `remember_fact`, `manage_reminder`) en `ank-core`

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 58
- **Asignado a:** Kernel Engineer

---

## Description
> **Nota de trazabilidad (`CORE-396`):** Este ticket pertenece formalmente a `EPIC 58`. El commit histórico previo `f7bf48b`/`eb81280` que usó el alias `CORE-375` para el catálogo 2026 está canonizado en [`CORE-375-p5.md`](file:///e:/Aegis/Aegis-Core/governance/Tickets/CORE-375-p5.md).

Para que el binario principal (`ank-server`) ofrezca la experiencia de asistente personal de chat infinito validada en el Sandbox (`CORE-372`), el `ChatAgent` en `ank-core` necesita acceso directo a herramientas personales instantáneas que persistan en el enclave cifrado del tenant (`TenantDB`) sin necesidad de spawnear un subagente pesado para acciones cotidianas de <1 segundo.

## Archivos a Modificar
1. **`kernel/crates/ank-core/src/enclave/mod.rs`** (líneas 142-169 y 460-555):
   - `TenantDB::init_schema()` **ya tiene** las tablas `expenses` (línea 145) y `reminders` (línea 159), y los métodos `add_expense`, `get_expenses`, `add_reminder`, `get_reminders`.
   - Agregar tabla `personal_facts`:
     ```sql
     CREATE TABLE IF NOT EXISTS personal_facts (
         id INTEGER PRIMARY KEY AUTOINCREMENT,
         category TEXT NOT NULL DEFAULT 'general',
         fact TEXT NOT NULL,
         source TEXT NOT NULL DEFAULT 'chat',
         created_at DATETIME DEFAULT CURRENT_TIMESTAMP
     )
     ```
   - Agregar función de normalización semántica `pub fn normalize_expense_category(raw_category: &str, description: &str) -> String` que mapee términos cotidianos (`"super"`, `"súper"`, `"supermercado"`, `"carniceria"`, `"carnicería"`, `"verduleria"`, `"verdulería"`, `"almacen"`, `"restaurante"`, `"delivery"` -> `"comida"`; `"nafta"`, `"combustible"`, `"uber"`, `"taxi"`, `"sube"`, `"peaje"` -> `"transporte"`; `"luz"`, `"gas"`, `"internet"`, `"alquiler"`, `"expensas"` -> `"hogar"`; `"farmacia"`, `"medico"`, `"médico"`, `"gimnasio"` -> `"salud"`).
   - Agregar métodos en `TenantDB`:
     - `pub fn add_personal_fact(&self, category: &str, fact: &str, source: &str) -> Result<bool>` (evitando duplicados exactos `LOWER(fact)`).
     - `pub fn get_personal_facts(&self, limit: u32) -> Result<Vec<serde_json::Value>>`.
     - `pub fn query_expenses_summary(&self, category_filter: Option<&str>) -> Result<serde_json::Value>` (retorna `total`, `totals_by_category` y `recent_entries`).
     - `pub fn complete_reminder(&self, id_or_match: &str) -> Result<usize>`.
     - `pub fn build_personal_memory_prompt_block(&self) -> Result<String>` (construye el bloque `[MEMORIA A LARGO PLAZO DEL USUARIO]` + `[RESUMEN FINANCIERO Y RECORDATORIOS]` para inyectar en el system prompt del Chat Agent).

2. **`kernel/crates/ank-core/src/agents/tool_registry.rs`** (líneas 101-106):
   - Agregar las 4 `ToolDefinition`: `log_expense_or_entry()`, `query_ledger()`, `remember_fact()`, `manage_reminder()`.
   - Incluirlas en `ToolRegistry::definitions_for(AgentRole::ChatAgent)` (actualizando el test unitario de cantidad de tools de `ChatAgent`).

3. **`kernel/crates/ank-core/src/chal/mod.rs`** (`execute_tool_call_internal`, línea 2267):
   - Agregar los 4 brazos (`"log_expense_or_entry"`, `"query_ledger"`, `"remember_fact"`, `"manage_reminder"`) abriendo `TenantDB::open(&tenant_id, session_key)` igual que en `"approve_path"` (línea 2557).
   - Al armar el system prompt del Chat Agent principal, concatenar el resultado de `db.build_personal_memory_prompt_block()` para que el asistente recuerde hechos y totales en cada turno.

4. **`kernel/crates/ank-http/src/routes/`** (`personal.rs` + `mod.rs`):
   - Exponer endpoints autenticados con `CitadelAuthenticated`:
     - `GET /api/personal/summary` (retorna `{ facts, ledger_summary, reminders }`)
     - `POST /api/personal/facts` y `DELETE /api/personal/facts/:id`

## Acceptance Criteria
- [x] `cargo test -p ank-core --lib` pasa incluyendo nuevos tests unitarios para `normalize_expense_category`, `add_personal_fact` y `query_expenses_summary`.
- [x] Decirle al chat *"Agregá 10000 de súper"* ejecuta `log_expense_or_entry`, normaliza la categoría a `"comida"` y persiste en `expenses` dentro de `memory.db` del tenant.
- [x] `GET /api/personal/summary` devuelve los hechos, gastos agrupados por categoría y recordatorios pendientes del tenant autenticado.

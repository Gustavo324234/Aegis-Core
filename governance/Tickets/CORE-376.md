# CORE-376: Chronos Real Conversation Consolidation en `ank-core/src/chronos`

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 58
- **Asignado a:** Kernel Engineer

---

## Description
> **Nota de trazabilidad (`CORE-396`):** Este ticket pertenece formalmente a `EPIC 58`. La especificación histórica de Fase 5 (Tool Calling Nativo, commit `ffb4e62`) queda canonizada en [`CORE-376-p5.md`](file:///e:/Aegis/Aegis-Core/governance/Tickets/CORE-376-p5.md).

Actualmente `ChronosDaemon::run_step()` en `kernel/crates/ank-core/src/chronos/mod.rs` (líneas 114-118) utiliza un string de prueba estático (`let test_raw_text = "ACTIVITY LOG: User modified main.rs...";`) durante su ciclo de consolidación cognitiva. Debe conectarse al historial real de conversación para extraer hechos implícitos sobre el usuario (preferencias, rutinas, proyectos, restricciones) y almacenarlos automáticamente en la memoria de largo plazo (`personal_facts` + L3 Swap).

## Archivos a Modificar
1. **`kernel/crates/ank-core/src/chronos/mod.rs`** (líneas 81-125):
   - Actualizar `build_consolidation_pcb(&self, tenant_id: &str, raw_text: &str) -> PCB` con un prompt de extracción estructurada:
     ```text
     Analizá los siguientes fragmentos recientes de conversación con el usuario y extraé hechos personales implícitos o explícitos que valga la pena recordar a largo plazo (gustos, horarios, familia, salud, proyectos, preferencias).
     Respondé estrictamente en JSON:
     {"summary": "...", "tags": ["..."], "facts": [{"category": "preferencias|rutina|salud|trabajo|general", "fact": "..."}]}
     ```
   - En `run_step()`, reemplazar `test_raw_text` leyendo las conversaciones recientes de los tenants activos en `SessionHistoryCache` (o cola de mensajes pendientes de consolidar) y seteando `pcb.tenant_id = Some(tenant_id)`.
   - Al completarse el PCB de `ChronosConsolidator`, parsear el JSON retornado:
     1. Guardar el `summary` en `swap_manager` (L3 Semántico).
     2. Si el tenant tiene enclave abierto/accesible, insertar cada elemento de `facts` usando `add_personal_fact(category, fact, "chronos")` (creado en `CORE-375`).

2. **`kernel/crates/ank-http/src/routes/personal.rs`**:
   - Añadir endpoint `POST /api/personal/chronos/consolidate` (autenticado con `CitadelAuthenticated`) que dispare inmediatamente una consolidación sobre los últimos mensajes del tenant activo y retorne `{"status": "ok", "new_facts": [...]}`.

## Acceptance Criteria
- [x] Eliminar el `test_raw_text` hardcodeado de `kernel/crates/ank-core/src/chronos/mod.rs`.
- [x] Verificar con test unitario en `chronos::tests` que `build_consolidation_pcb` incluya el `tenant_id` y el texto real de conversación.
- [x] `POST /api/personal/chronos/consolidate` extrae hechos implícitos de la charla reciente y los guarda con `source = "chronos"`.

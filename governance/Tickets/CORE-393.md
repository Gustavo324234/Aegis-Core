# CORE-393: Sincronización de Documentación, Changelog, Arquitectura, Sidecar Docs y Gobernanza (`docs`, `governance/`)

- **Tipo:** docs
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 60
- **Asignado a:** Arquitecto IA

---

## Description
Actualizar de forma integral la documentación del repositorio y el tablero maestro de gobernanza para reflejar con precisión todos los componentes, endpoints, herramientas, scripts y fixes implementados tras EPIC 58 y EPIC 59.

## Archivos Creados / Modificados
1. **`CHANGELOG.md` y `ARCHITECTURE.md`**:
   - Documentada sección `[Unreleased]` (`Added`, `Changed`, `Fixed`) con Google Calendar/OAuth, WhatsApp Bridge/Anti-Spam, UX personal/historial visual, sandbox Windows y fixes de ReAct/SSE/Router; actualizada la topología de rutas en `ank-http`, módulos en `ank-core`, árbol `tools/` y tabla de Epics.
2. **`tools/whatsapp_bridge/README.md`** (Nuevo):
   - Documentación técnica del sidecar Baileys (`npm run check`, variables `WA_BRIDGE_*` y `AEGIS_WHATSAPP_*`, API HTTP, deduplicación `@lid`, alias `KV_CONTACT_ALIASES` y límites anti-spam).
3. **`README.md` y `README.es.md`**:
   - Actualizada la estructura del repositorio (`tools/whatsapp_bridge/`, `tools/antigravity_bridge/`, `run_sandbox.bat`) e hitos (EPIC 59 y EPIC 60).
4. **`governance/EPIC_59_...`, `governance/EPIC_60_...`, `governance/Tickets/CORE-387..393.md`, `governance/TICKETS_MASTER.md` y `tools/sync_tickets_master.py`**:
   - Creado EPIC 60 y los tickets `CORE-387`..`CORE-393`, alineado el checklist de EPIC 59 (`✅ Done`), actualizado `tools/sync_tickets_master.py` para contemplar EPIC 59 y EPIC 60, y validado `sync_tickets_master.py --report` con 0 discrepancias.

## Acceptance Criteria
- [x] `CHANGELOG.md`, `ARCHITECTURE.md`, `README.md`, `README.es.md` y `tools/whatsapp_bridge/README.md` reflejan exactamente los archivos, rutas y comportamientos verificados en el repositorio.
- [x] Todos los cambios cuentan con ticket `CORE-*` en `governance/Tickets/` bajo `EPIC 60`.
- [x] `python tools/sync_tickets_master.py --report` finaliza sin discrepancias (`exit 0`).

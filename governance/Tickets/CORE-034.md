# CORE-034 — shell/ui: componentes de providers y router CMR

**Épica:** 32 — Unified Binary
**Fase:** 4 — Web UI
**Repo:** Aegis-Core — `shell/ui/src/components/`
**Asignado a:** Shell Engineer
**Prioridad:** 🟡 Media
**Estado:** DONE
**Depende de:** CORE-031

---

## Contexto

Tab de providers y configuración del Cognitive Model Router.
Se porta desde el legacy (Epic 30 + Epic 26).

**Referencia:** `Aegis-Shell/ui/src/components/ProvidersTab.tsx`
y `Aegis-Shell/ui/src/components/RouterConfig/`

---

## Componentes a portar

### `ProvidersTab.tsx`
Tab principal de la sección Providers en el AdminDashboard.
Muestra providers activos como cards. Permite agregar nuevos providers
con carga automática de modelos via `POST /api/providers/models`.
Reemplaza las tabs Motor + Router del legacy.

### `RouterConfig/` — subdirectorio completo
Componentes del CMR:
- `GlobalKeyManager` — gestión de API keys globales
- `ModelCatalogViewer` — visualización del catálogo de modelos con scores
- `CatalogSyncStatus` — estado del CatalogSyncer

### `SettingsPanel.tsx`
Panel de configuración del usuario (task type, preferencias).

### `SirenConfigTab.tsx`
Tab de configuración de voz en AdminDashboard:
- Selector de provider (Voxtral local / ElevenLabs cloud / Mock)
- Input de API key para providers cloud
- Selector de voice_id
- Test de voz

---

## Criterios de aceptación

- [ ] `ProvidersTab` lista providers activos y permite agregar nuevos
- [ ] Al agregar un provider, los modelos se cargan automáticamente via `/api/providers/models`
- [ ] `GlobalKeyManager` puede agregar/listar/eliminar keys del KeyPool global
- [ ] `ModelCatalogViewer` muestra los modelos con sus task scores
- [ ] `SirenConfigTab` permite configurar y testear el motor de voz
- [ ] `npm run build` → 0 errores TypeScript

## Referencia

`Aegis-Shell/ui/src/components/ProvidersTab.tsx`
`Aegis-Shell/ui/src/components/RouterConfig/`
`Aegis-Shell/ui/src/components/SirenConfigTab.tsx`
`Aegis-Shell/ui/src/components/SettingsPanel.tsx`

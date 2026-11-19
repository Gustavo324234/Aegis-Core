# CORE-387: Observabilidad Proveedor/Modelo Activo, Tools de Proveedores/Módulos y Tablas GFM en Chat (`ank-core`, `shell/ui`)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 60
- **Asignado a:** Kernel Engineer + Shell Engineer

---

## Description
Al interactuar con el asistente, el usuario necesitaba ver en el header superior no solo el ID del modelo sino también el **proveedor activo**, consultar desde el chat qué modelos/módulos estaban disponibles o configurados (como OpenRouter o módulos del microkernel), y visualizar correctamente tablas comparativas sin perder el formato Markdown.

## Archivos Creados / Modificados
1. **`shell/ui/src/components/TelemetryDashboard.tsx`**:
   - Visualización conjunta de `lastRoutingInfo.provider` y `lastRoutingInfo.model_id` en el indicador superior de modelo activo.
2. **`shell/ui/src/components/ChatTerminal.tsx`**:
   - Integración de `remarkGfm` (`remark-gfm`) en `<ReactMarkdown>` para renderizar tablas GFM limpias dentro del chat.
   - Selector de modelos agrupado por proveedor.
3. **`kernel/crates/ank-core/src/agents/tool_registry.rs` y `kernel/crates/ank-core/src/chal/mod.rs`**:
   - Herramienta `manage_ai_provider` (`add`, `add_model`, `list`, `catalog` con `free_only`/`limit`, `remove`) y detección de prefijo de API key.
   - Herramienta `manage_modules` (`list`, `enable`, `disable`) para inspeccionar y gestionar módulos del microkernel y plugins activos desde el chat.
4. **`kernel/crates/ank-core/src/router/models.yaml`, `discovery.rs`, `shell/ui/src/components/ProvidersTab.tsx` y `ModelCatalogViewer.tsx`**:
   - Curación de catálogo (incl. Groq 2026 y free-tier), filtrado de modelos no-chat, auto-probe al editar y refresco manual de catálogo.

## Acceptance Criteria
- [x] El header superior muestra proveedor y modelo activo cuando existe `lastRoutingInfo`.
- [x] Las tablas Markdown generadas por el asistente se renderizan como tablas HTML válidas en `ChatTerminal.tsx`.
- [x] El chat dispone de `manage_ai_provider` y `manage_modules` registrados en `ToolRegistry` para consultar catálogo/módulos en vivo.

# Especificación y Tickets de Maduración Técnica: Aegis Cutting-Edge (Fase 5)

Este documento define la arquitectura y el desglose de tickets de desarrollo (canonizados en `governance/Tickets/` como `CORE-375-p5` a `CORE-379-p5`, usados originalmente en el roadmap como `CORE-376`..`CORE-379`) para elevar a Aegis al estado del arte (*cutting-edge*) como Sistema Operativo Cognitivo nativo.

> **Nota de reconciliación (`CORE-396`):** Para resolver la colisión histórica de numeración con `EPIC 58` (`governance/Tickets/CORE-375.md`..`CORE-379.md`), los entregables de esta Fase 5 quedan registrados formalmente bajo los IDs `CORE-375-p5` (`f7bf48b`, `eb81280`), `CORE-376-p5` (`ffb4e62`), `CORE-377-p5` (`77e6554`), `CORE-378-p5` (`7efd3a0`) y `CORE-379-p5`, preservando intactas las referencias de commits ya fusionados.

---

## CORE-376-p5 (Roadmap histórico `CORE-376`): Soporte Nativo de Tool Calling / Function Calling en Router Cognitivo

- **Estado:** ✅ Completado (PR / Branch: `feat/core-376-native-tool-calling`, commit `ffb4e62`)
- **Tipo:** Feature / Architecture
- **Crates / Componentes:** `ank-core::router`, `ank-core::pcb`, `ank-core::agents::tool_registry`
- **Prioridad:** Alta
- **Objetivo:**
  Permitir que los modelos invocados a través del [`CognitiveRouter`](file:///e:/Aegis/Aegis-Core/kernel/crates/ank-core/src/router/mod.rs) reciban esquemas de herramientas (JSON Schema) y devuelvan llamadas estructuradas (`tool_calls`), ejecutables con sandboxing criptográfico a través de Citadel.

### Criterios de Aceptación:
1. Definir la estructura unificada `ToolDefinition` y `ToolCall` en `ank-core::pcb`.
2. Adaptar serializadores/deserializadores para proveedores OpenAI-compatible, Anthropic (Claude tool use format) y Gemini Function Calling.
3. El bucle de ejecución del PCB debe permitir pausar la ejecución esperando el resultado de la herramienta y reanudar con el contexto actualizado.
4. Cobertura de pruebas unitarias con mock de llamadas a herramientas sin pánico (Zero-Panic policy).

---

## CORE-377-p5 (Roadmap histórico `CORE-377`): Integración del Protocolo MCP (Model Context Protocol)

- **Estado:** ✅ Completado (PR / Branch: `feat/core-377-mcp-integration`, commit `77e6554`)
- **Tipo:** Feature / Integration
- **Crates / Componentes:** `ank-mcp`, `ank-core::chal`, `ank-http::routes::mcp_api`, `ank-server`
- **Prioridad:** Alta
- **Objetivo:**
  Implementar cliente MCP nativo en Rust para conectar Aegis con servidores de herramientas y recursos externos (archivos locales, navegadores, terminales, GitHub, PostgreSQL, etc.) de forma transparente y estandarizada.

### Criterios de Aceptación:
1. Implementar cliente JSON-RPC 2.0 sobre transporte `stdio` y `sse` según la especificación MCP.
2. Endpoint o comando de configuración de servidores MCP en `aegis.yaml` / `Citadel`.
3. Descubrimiento dinámico de herramientas expuestas por servidores MCP registrados e inyección automática en el catálogo de herramientas del modelo.
4. Aislamiento de permisos: cada herramienta MCP debe validar permisos contra las políticas del tenant antes de ejecutarse.

---

## CORE-378-p5 (Roadmap histórico `CORE-378`): Motor de Memoria Semántica y RAG Local Embebido

- **Estado:** ✅ Completado (PR / Branch: `feat/core-378-semantic-memory-rag`, commit `7efd3a0`)
- **Tipo:** Feature / Storage & AI
- **Crates / Componentes:** `ank-core::chal::drivers::embeddings`, `ank-core::vcm`, `ank-http::routes::memory_api`
- **Prioridad:** Media
- **Objetivo:**
  Dotar a Aegis de memoria persistente episódica y contextual sin depender de servicios en la nube para embeddings ni bases vectoriales externas.

### Criterios de Aceptación:
1. Integrar runtime de inferencia de embeddings en Rust (vía `candle` o `ort`/ONNX Runtime con modelos compactos como `all-MiniLM-L6-v2` o `bge-small-en-v1.5`).
2. Almacenamiento vectorial embebido en disco (integración con motor vectorial embebido o SQLite-vec / LanceDB).
3. Indexación semántica automática de conversaciones pasadas y notas del usuario con particionado estricto por `tenant_id`.
4. Endpoint `/api/memory/search` y recuperación automática durante la preparación del PCB para inyección de contexto relevante.

---

## CORE-379-p5 (Roadmap histórico `CORE-379`): Modernización de Interfaz Web (UI/UX) y Generative Artifacts

- **Estado:** 🚧 En Progreso (selector de modelos, filtro `:free`, gestor de llaves y GFM ya integrados en `shell/ui`; pendiente KaTeX/Mermaid y panel de artefactos)

- **Tipo:** Enhancement / Frontend
- **Crates / Componentes:** `apps/aegis_os` (React + TypeScript + Tailwind)
- **Prioridad:** Media
- **Objetivo:**
  Mejorar la experiencia interactiva de usuario, soporte para renderizado enriquecido de respuestas y gestión visual del sistema.

### Criterios de Aceptación:
1. Renderizado de Markdown con syntax highlighting (Prism/Shiki), renderizado de LaTeX/KaTeX y visualización de diagramas Mermaid.
2. Panel lateral de "Artefactos" interactivos (código editable, vista previa HTML/SVG generada por el agente).
3. Selector visual de modelos con filtro de modelos gratuitos (`:free`), costos estimados y estado de salud de proveedores.
4. Gestor visual de llaves de API y configuración de enrutamiento (prioridad costo vs rendimiento).

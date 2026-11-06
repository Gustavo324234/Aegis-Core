# Especificación y Tickets de Maduración Técnica: Aegis Cutting-Edge (Fase 5)

Este documento define la arquitectura y el desglose de tickets de desarrollo (CORE-376 a CORE-379) para elevar a Aegis al estado del arte (*cutting-edge*) como Sistema Operativo Cognitivo nativo.

---

## CORE-376: Soporte Nativo de Tool Calling / Function Calling en Router Cognitivo

- **Estado:** ✅ Completado (PR / Branch: `feat/core-376-native-tool-calling`)
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

## CORE-377: Integración del Protocolo MCP (Model Context Protocol)

- **Tipo:** Feature / Integration
- **Crates / Componentes:** `ank-core::mcp`, `ank-server`
- **Prioridad:** Alta
- **Objetivo:**
  Implementar cliente MCP nativo en Rust para conectar Aegis con servidores de herramientas y recursos externos (archivos locales, navegadores, terminales, GitHub, PostgreSQL, etc.) de forma transparente y estandarizada.

### Criterios de Aceptación:
1. Implementar cliente JSON-RPC 2.0 sobre transporte `stdio` y `sse` según la especificación MCP.
2. Endpoint o comando de configuración de servidores MCP en `aegis.yaml` / `Citadel`.
3. Descubrimiento dinámico de herramientas expuestas por servidores MCP registrados e inyección automática en el catálogo de herramientas del modelo.
4. Aislamiento de permisos: cada herramienta MCP debe validar permisos contra las políticas del tenant antes de ejecutarse.

---

## CORE-378: Motor de Memoria Semántica y RAG Local Embebido

- **Tipo:** Feature / Storage & AI
- **Crates / Componentes:** `ank-core::memory`, `ank-core::vector`
- **Prioridad:** Media
- **Objetivo:**
  Dotar a Aegis de memoria persistente episódica y contextual sin depender de servicios en la nube para embeddings ni bases vectoriales externas.

### Criterios de Aceptación:
1. Integrar runtime de inferencia de embeddings en Rust (vía `candle` o `ort`/ONNX Runtime con modelos compactos como `all-MiniLM-L6-v2` o `bge-small-en-v1.5`).
2. Almacenamiento vectorial embebido en disco (integración con motor vectorial embebido o SQLite-vec / LanceDB).
3. Indexación semántica automática de conversaciones pasadas y notas del usuario con particionado estricto por `tenant_id`.
4. Endpoint `/api/memory/search` y recuperación automática durante la preparación del PCB para inyección de contexto relevante.

---

## CORE-379: Modernización de Interfaz Web (UI/UX) y Generative Artifacts

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

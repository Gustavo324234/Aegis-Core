# CORE-149 — Feat: Neuronal Memory (L3) & Semantic Retrieval

**Epic:** 41 — UX & Onboarding
**Repo:** Aegis-Core
**Tipo:** feat
**Prioridad:** Crítica
**Asignado a:** Kernel Engineer

---

## Problema

El sistema carecía de una "Base de Datos Neuronal" funcional. Aunque la infraestructura de almacenamiento vectorial existía (`LanceSwapManager`), no había un motor de embeddings activo ni un mecanismo automático de guardado y recuperación semántica. Esto hacía que Aegis "olvidara" el contexto de conversaciones pasadas fuera del buffer inmediato.

---

## Solución Implementada

### 1. Engine de Embeddings
Se implementó `CloudEmbeddingDriver` en `kernel/crates/ank-core/src/chal/drivers/embeddings.rs`.
- Soporta generación de vectores individuales y por lotes.
- Compatible con modelos de Cloud (OpenAI/Gemini/Proxy) configurados vía ENV.
- Re-ruta peticiones automáticamente al endpoint `/embeddings`.

### 2. Archivador Semántico Automático
Se integró en el bucle de ejecución principal de `ank-server/src/main.rs`.
- Cada interacción (User + Assistant) se captura al finalizar.
- Se genera un vector del par instrucción/respuesta.
- Se almacena de forma asíncrona en el `LanceSwapManager`.

### 3. Recuperación semántica en VCM
Se actualizó `VirtualContextManager::assemble_context` para:
- Recibir el `EmbeddingDriver`.
- Generar un vector de la consulta actual.
- Buscar los 5 fragmentos más similares en la base neuronal.
- Inyectar estos recuerdos en el prompt final bajo el tag `[Memory ID: ...]`.

---

## Criterios de aceptación

- [x] Los embeddings se generan correctamente desde la API Cloud.
- [x] Las interacciones se guardan en el workspace del tenant (`.aegis_swap/`).
- [x] El VCM recupera recuerdos relevantes basados en la consulta actual.
- [x] La integración es asíncrona y no penaliza la latencia de respuesta inicial.

---

## Commit message

```
feat(core): implement neuronal memory storage and semantic retrieval pipeline (CORE-149)
```

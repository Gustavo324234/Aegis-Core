# CORE-327 — Kernel: conversation_id en el PCB + sticky cache por conversación

**Tipo:** feat
**Prioridad:** Media
**Épica:** EPIC 51 — Model Intelligence
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

El sticky cache del CMR (CORE-319) se indexa por `(tenant, task_type, model_pref)`. Dos conversaciones paralelas del mismo tenant (dos pestañas de chat) comparten el mismo slot: cada una pisa la decisión de la otra y ambas pueden quedar flip-flopeando de modelo — exactamente lo que el sticky debía evitar. Era el descope documentado de CORE-324: el PCB no transportaba ninguna identidad de conversación estable (el `pid` es por turno, `session_key` es por login).

## Solución implementada

1. **`PCB.conversation_id: Option<String>`** (serde default — los PCBs persistidos viejos deserializan como None).
2. **`ws/chat.rs`**: cada conexión WebSocket de chat genera un UUID y lo estampa en todos los PCBs que origina — una conversación = una conexión, sin cambios de protocolo ni de UI.
3. **`StickyKey`** incluye `conversation_id`; flujos no conversacionales (agentes, tareas internas) siguen con None = clave por tenant, comportamiento previo intacto.
4. `/api/router/stats` expone el `conversation_id` de cada sticky decision (observabilidad).

## Criterios de aceptación

- [x] Dos conversaciones del mismo tenant obtienen slots sticky independientes (test `test_sticky_isolated_per_conversation`).
- [x] Flujos sin conversation_id conservan el comportamiento por tenant (suite previa en verde sin cambios de semántica).
- [x] PCBs persistidos con el esquema anterior deserializan sin error (serde default).
- [x] `cargo test -p ank-core --lib` en verde.

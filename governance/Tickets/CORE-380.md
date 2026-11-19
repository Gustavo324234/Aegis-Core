# CORE-380: Rediseño de `README.md` y `README.es.md` con Enfoque de Asistente Personal y Casos de Uso

- **Tipo:** docs
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 58
- **Asignado a:** Arquitecto IA

---

## Description
`README.md` y `README.es.md` presentaban el proyecto exclusivamente con terminología académica de sistemas operativos (*"LLMs as ALUs"*, *"Citadel Enclave"*, *"PinchBench"*), dificultando que visitantes nuevos en GitHub comprendan en los primeros 10 segundos que Aegis es un asistente personal autónomo con un único chat infinito, memoria a corto/largo plazo, gestión de gastos y agentes en segundo plano.

## Acceptance Criteria
- [x] Reescribir la apertura y secciones principales de `README.md` y `README.es.md` destacando la propuesta de valor como Asistente Personal de Chat Infinito con ejemplos concretos (finanzas semánticas, memoria de largo plazo + `Chronos`, tareas en segundo plano no bloqueantes).
- [x] Incluir una sección **"Try the Brain in 30 Seconds (Agent Sandbox)"** para que cualquier desarrollador pueda probar la inteligencia del asistente inmediatamente con Python antes de instalar o compilar el binario completo.
- [x] Preservar toda la documentación de instalación multiplataforma, arquitectura en Rust (`ank-server`, `ank-core`, `ank-http`), Citadel y benchmarks en secciones bien estructuradas.

# Aegis Core — Launch-Readiness Audit & Recorte del MVP Público

> **Estado:** DRAFT — para revisión del Arquitecto/Owner antes de canonizar.
> **Arquitecto IA · 2026-05-31**
> **Objetivo del primer lanzamiento:** Validar la tesis técnica — estrellas, credibilidad y atención de developers.

---

## 0. Veredicto (TL;DR)

La tesis de Aegis es de nivel referencia: *"LLMs como ALUs bajo un motor determinista, cognición a nivel kernel, un binario, cero dependencias de runtime"*. El relato y la disciplina de ingeniería (Rust, Zero-Panic, binario único, local-first) son coherentes entre sí. **Ese es el activo.**

El bloqueante para lanzar **no son features** — hay 15 epics en verde. El bloqueante es una **brecha de credibilidad**: hoy un visitante tiene 15 tildes "✅ Done" y **cero prueba pública** de que algo funcione. El público que querés conquistar (devs senior, escépticos por oficio) es exactamente el que más rechaza el "confiá en mí, está hecho". Si no cerramos esa brecha antes de publicar, el resultado más probable no es indiferencia: es el veredicto "vaporware con buen README", que es peor que no lanzar.

**Conclusión:** el trabajo de MVP no es *construir más*, es *probar lo que ya hay* y *recortar lo que no prueba la tesis*.

---

## 1. El público y la vara de éxito

| | |
|---|---|
| **Lector objetivo** | Ingeniero senior / builder evaluando críticamente. Llega vía HN, Reddit, X o un repo trending. |
| **Camino de éxito** | Aterriza → entiende la tesis en **30s** → ve **prueba** de que es real en **2 min** → estrella / comparte / clona para hurgar. |
| **Modo de fallo** | "Otra capa sobre LLMs", o "muchos checkmarks, ningún demo → vaporware". Cierra la pestaña. |
| **Moneda** | Credibilidad. Un dev no perdona el over-claiming; sí premia profundidad demostrada y honestidad de alcance. |

Todo el recorte de abajo se mide contra una sola pregunta: **¿esto hace que un ingeniero escéptico crea que la tesis es real y funciona?**

---

## 2. Hallazgo de integridad de governance (riesgo de credibilidad directo)

La fuente de verdad de estado **no es confiable hoy**, y es verificable:

- `TICKETS_MASTER.md` declara **EPIC 46 — Public Launch: ✅ Completa 100%**.
- El doc fuente `EPIC_46_PUBLIC_LAUNCH.md` tiene **todos sus tickets en 📥 Todo** y el checklist de completitud **entero sin tildar**.
- Los archivos del epic (CoC, SECURITY, issue templates, FUNDING) **sí existen** en el repo.

→ El trabajo se hizo, pero el tracking nunca se sincronizó, y **dos documentos de governance se contradicen**. Esto viola el pilar de **trazabilidad por tickets** del Codex. Síntomas relacionados: los fixes recientes del cognitive loop (spawn_agent / rate-limit, CORE-264 a 267) **no figuran** en el master.

**Por qué importa para este lanzamiento:** si un dev cruza el README ("Epic 46 Done") contra `governance/` y encuentra la contradicción, la credibilidad se evapora en un click. Un repo cuyo propio tracking se contradice proyecta exactamente el "100% que no es real".

**Acción recomendada (pre-launch):** pase de reconciliación de governance — sincronizar `TICKETS_MASTER.md` con el estado real de cada epic doc, registrar los CORE-264..267 faltantes, y degradar los "100%" que no resistan verificación.

---

## 3. Gap analysis — "Done" vs "Listo para ojos de developers"

| Área | Estado declarado | Realidad para un dev escéptico | ¿Bloquea validar la tesis? |
|---|---|---|---|
| Posicionamiento / README | Excelente | Genuinamente fuerte y diferenciado. No tocar. | No |
| Higiene OSS (MIT, CoC, SECURITY, issue templates) | Done | Presente y correcta. | No |
| **Prueba de que funciona** (demo / screenshots / video / GIF) | — | **Cero.** Para un producto con UI + agentes + voz, ningún visual = ningún proof. | **SÍ** |
| **Benchmarks** (PinchBench) | Existe el harness | **Ningún resultado publicado.** "Ruteo cognitivo" es claim sin evidencia. | **SÍ** |
| **Señal de calidad** (tests / coverage / CI verde de tests) | Zero-Panic en CI | README solo muestra badge de *build*. Sin señal de tests visible → flojo para credibilidad de un kernel en Rust. | **SÍ** |
| Postura de seguridad (Citadel, SQLCipher, Zero-Trust) | Claim central | Bien narrada, pero no visible/auditada (sin threat model, sin firmas). | Parcial |
| **Primeros 5 minutos / confianza en el install** | `curl \| sudo bash` | Ironía para un producto Zero-Trust: pipear script a root. Y post-install no se ve qué pasa. | **SÍ** |
| Amplitud vs profundidad | 15 epics Done | Superficie enorme, 1 mantenedor (+agentes). Dispersa la tesis y dispara "¿se mantiene o se pudre?". | **SÍ** (vía recorte) |
| Trazabilidad por tickets | Pilar Codex | Roto en la práctica (ver §2). | **SÍ** |
| Evidencia de uso real / dogfooding | — | "Done" ≠ "probado en uso". Sin relato de "corriendo en prod hace N semanas". | Parcial |

---

## 4. Bloqueantes reales del MVP (priorizados)

Lo que **debe ser cierto** antes de publicar para devs. Estos son las semillas de los tickets del futuro Epic 56 — **no se crean acá** hasta validar el recorte.

- **B1 — Asset de prueba (máxima palanca).** Un video corto (<3 min) + 3-4 screenshots/GIF en el README que muestren el loop real: chat maestro → spawn de sub-agente → tool use nativo → resultado, más el dashboard mostrando agentes como procesos. Un solo asset que convierte 15 claims en una demostración.
- **B2 — Un benchmark publicado.** Tabla de resultados de PinchBench, aunque sea chica, **honesta y reproducible**, que sustente "el ruteo cognitivo elige el modelo correcto". Convierte el claim en evidencia.
- **B3 — Señales de credibilidad en el README.** Badge real de tests/coverage + estado de CI de tests, y un deep-dive técnico breve (o link fuerte a `ARCHITECTURE.md`) con un **diagrama de secuencia del cognitive loop**. Los devs estrellan profundidad, no checkmarks.
- **B4 — Honestidad de alcance.** Etiquetar como *experimental / roadmap* la superficie no probada (móvil, voz/Siren, distro, maker-capability, módulos SDUI) en vez de "✅ Done". Así el núcleo que prueba la tesis se lee sólido y el resto se lee como futuro. El over-claiming es la vía más rápida a perder al dev.
- **B5 — Confianza en el primer arranque.** Para el público objetivo, ofrecer un camino que no sea `curl | sudo bash`: `docker compose up` o (ideal) un **demo hosted read-only** donde un dev toque Aegis sin instalar nada. Documentar checksums/firmas y opción no-root. Más un "qué ves después de instalar" de 5 líneas.
- **B6 — Reconciliación de governance (§2).** Sincronizar el estado real antes de que alguien lo cruce.

*(Opcional alto-impacto)* **B7 — Nota de dogfooding honesta:** "lo corro para mí hace N semanas; esto se rompió y se arregló". La autenticidad vende a devs mejor que cualquier feature.

---

## 5. Recorte del MVP — qué entra al frente / qué se esconde

Principio del recorte: **el MVP debe hacer que un dev diga "la idea de kernel-como-OS-para-agentes es real y funciona". Nada más necesita estar perfecto.**

| Núcleo que PRUEBA la tesis (ship + destacar) | Se mantiene pero se BAJA del frente (etiquetar experimental/roadmap) |
|---|---|
| Kernel determinista + cognitive loop (ReAct + tool use nativo) | App móvil (Expo / Orion ID) |
| Binario único, cero runtime deps (`ank-server`) | Voz / Siren Protocol (WebRTC) |
| Ruteo cognitivo + el benchmark de B2 | `distro/` (Linux inmutable) |
| Local-first / multi-tenant cifrado (Citadel) como narrativa | Maker-capability (sandbox JS autónomo) |
| Dashboard mostrando agentes como procesos (el "OS" hecho tangible) | Módulos SDUI / paneles dinámicos |

La amplitud no se borra — se **reordena**. Hoy juega en contra porque diluye la tesis y multiplica la superficie de riesgo sin probar el núcleo.

---

## 6. Lo que explícitamente NO bloquea el lanzamiento

Para proteger el foco (achicar scope, no expandirlo), **nada de esto debe demorar la validación de la tesis**:

- `distro/` (imagen Linux inmutable)
- Escalado de LanceDB L3 / optimización fina del cognitive loop
- Pulido de la app móvil
- Matriz amplia de providers
- Fase 3 de sincronización/replicación

Son post-MVP. Si aparecen como precondición, es scope creep.

---

## 7. Límite de esta auditoría

Esta auditoría evaluó el **estado declarado** (governance docs) y la **cara pública** (README, estructura). **No es una auditoría de código.** No verifiqué que el binario haga lo que cada ticket dice.

Dado que la tesis *es* el cognitive loop, se recomienda un **pase de verificación acotado al núcleo** antes de publicar: confirmar contra el binario real (principio "verify the deployed binary": `strings` sobre el binario compilado para configs embebidas vía `rust-embed`) que el loop ReAct + tool use + spawn de agentes funciona end-to-end como se declara. Ese pase es el seguro contra la misma ilusión de completitud que motiva este documento.

---

## 8. Próximo paso (gated)

Una vez que el Owner valide este recorte:

1. Crear **EPIC 56 — Public MVP / Thesis Validation** en governance.
2. Convertir B1–B6 (y B7 si entra) en **tickets individuales** en `governance/Tickets/CORE-NNN.md`, cada uno con criterios de aceptación verificables.
3. Sincronizar `TICKETS_MASTER.md`.

**No se crea el epic ni los tickets hasta validar §5.** Alineación antes de expandir scope.

---

*Arquitecto IA — 2026-05-31 — DRAFT v1*

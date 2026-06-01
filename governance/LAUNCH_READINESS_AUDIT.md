# Aegis Core — Launch-Readiness Audit & Recorte del MVP Público

> **Estado:** DRAFT — para revisión del Arquitecto/Owner antes de canonizar.
> **Arquitecto IA · 2026-05-31**
> **Objetivo del primer lanzamiento:** Validar la tesis técnica — estrellas, credibilidad y atención de developers.

> **Decisiones registradas en esta sesión:**
> 1. **Objetivo del launch:** validar la tesis con developers (estrellas / credibilidad), no captar usuarios finales aún.
> 2. **Arquitectura de producto:** el OS cognitivo real (distro) es el **norte estratégico y flagship de v2**; el **overlay/satélite se shippea primero** como MVP (ver §2).

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

## 2. Arquitectura de producto: dos modos de entrega y el norte

Decisión registrada (2026-05-31). Mismo kernel, dos entregas:

| Modo | Qué es | Rol | Cuándo |
|---|---|---|---|
| **Aegis OS (distro)** | Imagen NixOS mínima, inmutable, bare-metal con `ank-server` como servicio de sistema de primera clase | **Flagship** — el OS real, máxima expresión de la tesis | **v2 (norte)** |
| **Aegis overlay / satélite** | Instalación sobre un SO existente (Linux/macOS/Windows), como ya hacen los satélites mobile | **Rampa de entrada** — runtime self-hosted | **MVP (ahora)** |

"Satélite" ya es vocabulario del proyecto en mobile: los satélites orbitan un core; el distro *es* el core, los overlays orbitan o corren standalone. No es un compromiso — es arquitectura de producto intencional.

**Estado del distro:** `distro/README.md` lo marca **INITIATED & DESIGNED** — NixOS declarativo, root de solo lectura (`ro`), LUKS2 full-disk, SQLCipher dual-encryption, swap dentro del volumen cifrado. Diseño serio y Codex-aligned; *fortalece* Citadel. **Pero diseñado ≠ shippeado:** falta imagen firmada + canal OTA, ISO/instalador real (hoy el flujo es NixOS-minimal manual), matriz de hardware/drivers, y la decisión de shell on-device (kiosk Wayland + shell React) vs UI web headless.

**Convergencia clave:** una imagen inmutable, firmada, con updates atómicos A/B vuelve irrelevante el `curl | sudo bash`. El distro *es* la solución definitiva al problema de confianza del install — por eso los arreglos del install overlay (B5) son **interinos**: resuelven el on-ramp ahora; el distro lo cierra de raíz en v2.

**Secuencia (por qué este orden):** la tesis es idéntica corra como overlay o bare-metal → no se necesita el distro para validarla. Se valida con el overlay MVP, se junta audiencia/credibilidad, y el distro se lanza como **segundo golpe (v2)** ante gente que ya cree en la tesis. Mientras tanto sigue horneándose en background sin bloquear el launch. Al revés, es tirar el artefacto más difícil al vacío.

---

## 3. Hallazgo de integridad de governance (riesgo de credibilidad directo)

La fuente de verdad de estado **no es confiable hoy**, y es verificable:

- `TICKETS_MASTER.md` declara **EPIC 46 — Public Launch: ✅ Completa 100%**.
- El doc fuente `EPIC_46_PUBLIC_LAUNCH.md` tiene **todos sus tickets en 📥 Todo** y el checklist de completitud **entero sin tildar**.
- Los archivos del epic (CoC, SECURITY, issue templates, FUNDING) **sí existen** en el repo.

→ El trabajo se hizo, pero el tracking nunca se sincronizó, y **dos documentos de governance se contradicen**. Esto viola el pilar de **trazabilidad por tickets** del Codex. Síntomas relacionados: los fixes recientes del cognitive loop (spawn_agent / rate-limit, CORE-264 a 267) **no figuran** en el master.

**Por qué importa para este lanzamiento:** si un dev cruza el README ("Epic 46 Done") contra `governance/` y encuentra la contradicción, la credibilidad se evapora en un click. Un repo cuyo propio tracking se contradice proyecta exactamente el "100% que no es real".

**Acción recomendada (pre-launch):** pase de reconciliación de governance — sincronizar `TICKETS_MASTER.md` con el estado real de cada epic doc, registrar los CORE-264..267 faltantes, y degradar los "100%" que no resistan verificación.

---

## 4. Gap analysis — "Done" vs "Listo para ojos de developers"

| Área | Estado declarado | Realidad para un dev escéptico | ¿Bloquea validar la tesis? |
|---|---|---|---|
| Posicionamiento / README | Excelente | Genuinamente fuerte y diferenciado. No tocar. | No |
| Higiene OSS (MIT, CoC, SECURITY, issue templates) | Done | Presente y correcta. | No |
| **Prueba de que funciona** (demo / screenshots / video / GIF) | — | **Cero.** Para un producto con UI + agentes + voz, ningún visual = ningún proof. | **SÍ** |
| **Benchmarks** (PinchBench) | Existe el harness | **Ningún resultado publicado.** "Ruteo cognitivo" es claim sin evidencia. | **SÍ** |
| **Señal de calidad** (tests / coverage / CI verde de tests) | Zero-Panic en CI | README solo muestra badge de *build*. Sin señal de tests visible → flojo para credibilidad de un kernel en Rust. | **SÍ** |
| **Firma de plugins** (Citadel) | Plugins Wasm firmados (ed25519) | El installer setea `AEGIS_ALLOW_INSECURE_PLUGINS=1` por defecto → **firma desactivada en lo que se shippea**. Contradice el claim Zero-Trust. | **SÍ** |
| Postura de seguridad (Citadel, SQLCipher, Zero-Trust) | Claim central | Bien narrada, pero no visible/auditada (sin threat model, sin firmas de release). | Parcial |
| **Primeros 5 minutos / confianza en el install** | `curl \| sudo bash` (baja `nightly`) | Ironía para un producto Zero-Trust: pipear script no verificado a root, y la *nightly* por default. Post-install no se ve qué pasa. | **SÍ** |
| Amplitud vs profundidad | 15 epics Done | Superficie enorme, 1 mantenedor (+agentes). Dispersa la tesis y dispara "¿se mantiene o se pudre?". | **SÍ** (vía recorte) |
| Trazabilidad por tickets | Pilar Codex | Roto en la práctica (ver §3). | **SÍ** |
| Evidencia de uso real / dogfooding | — | "Done" ≠ "probado en uso". Sin relato de "corriendo en prod hace N semanas". | Parcial |

---

## 5. Bloqueantes reales del MVP (priorizados)

Lo que **debe ser cierto** antes de publicar para devs. Son las semillas de los tickets del futuro Epic 56 — **no se crean acá** hasta validar el recorte (§6).

- **B1 — Asset de prueba (máxima palanca) · Alta.** Un video corto (<3 min) + 3-4 screenshots/GIF en el README que muestren el loop real: chat maestro → spawn de sub-agente → tool use nativo → resultado, más el dashboard mostrando agentes como procesos. Un solo asset que convierte 15 claims en una demostración.
- **B2 — Un benchmark publicado · Alta.** Tabla de resultados de PinchBench, aunque sea chica, **honesta y reproducible**, que sustente "el ruteo cognitivo elige el modelo correcto". Convierte el claim en evidencia.
- **B3 — Señales de credibilidad en el README · Alta.** Badge real de tests/coverage + estado de CI de tests, y un deep-dive técnico breve (o link fuerte a `ARCHITECTURE.md`) con un **diagrama de secuencia del cognitive loop**. Los devs estrellan profundidad, no checkmarks.
- **B4 — Honestidad de alcance · Alta.** Etiquetar como *experimental / roadmap* la superficie no probada (móvil, voz/Siren, distro, maker-capability, módulos SDUI) en vez de "✅ Done". Así el núcleo que prueba la tesis se lee sólido y el resto se lee como futuro. El over-claiming es la vía más rápida a perder al dev.
- **B5 — Confianza en el primer arranque del overlay (interino) · Alta.** Para el público objetivo:
  - (a) un **demo hosted read-only**, o surfacear el modo **Docker que ya existe** en el installer, para tocar Aegis sin instalar;
  - (b) default a un **tag de release fijo**, no `nightly`;
  - (c) **`SHA256SUMS` por asset + verificación** en el installer (fail-closed); idealmente firmas (Sigstore/cosign o GitHub artifact attestations);
  - (d) documentar el **dos-pasos read-first** (`curl -o install.sh …; less install.sh; sudo bash install.sh`) como camino recomendado;
  - (e) opción **rootless** user-scoped (el daemon ya corre como usuario `aegis` no-root con hardening systemd — eso está bien y se mantiene).
  - Más un "qué ves después de instalar" de 5 líneas. *Nota: el distro (v2) cierra esto de raíz; esto es para el on-ramp.*
- **B6 — Cerrar la firma de plugins · CRÍTICO (Zero-Trust).** El install escribe `AEGIS_ALLOW_INSECURE_PLUGINS=1` por defecto porque todavía no existe el comando de keygen (lo admite el propio comentario; el backfill de upgrade lo perpetúa). Acción: shippear `aegis keygen` (par ed25519), generarlo en el primer arranque, setear `AEGIS_PLUGIN_ROOT_KEY`, y **sacar el flag inseguro del default**. Sin esto, "Citadel Zero-Trust en cada capa" es un claim falso que un dev encuentra leyendo el script. Es exactamente el workaround "me funciona a mí" que el proyecto dice rechazar.
- **B7 — Reconciliación de governance (§3) · Alta.** Sincronizar el estado real antes de que alguien lo cruce.

*(Opcional alto-impacto)* **B8 — Nota de dogfooding honesta · Media.** "Lo corro para mí hace N semanas; esto se rompió y se arregló". La autenticidad vende a devs mejor que cualquier feature.

---

## 6. Recorte del MVP — qué entra al frente / qué se esconde

Principio del recorte: **el MVP debe hacer que un dev diga "la idea de kernel-como-OS-para-agentes es real y funciona". Nada más necesita estar perfecto.**

| Núcleo que PRUEBA la tesis (ship + destacar) | Se mantiene pero se BAJA del frente |
|---|---|
| Kernel determinista + cognitive loop (ReAct + tool use nativo) | App móvil (Expo / Orion ID) — experimental |
| Binario único, cero runtime deps (`ank-server`) | Voz / Siren Protocol (WebRTC) — experimental |
| Ruteo cognitivo + el benchmark de B2 | Maker-capability (sandbox JS autónomo) — experimental |
| Local-first / multi-tenant cifrado (Citadel) como narrativa | Módulos SDUI / paneles dinámicos — experimental |
| Dashboard mostrando agentes como procesos (el "OS" hecho tangible) | **`distro/` Aegis OS → flagship de v2 (norte):** se comunica como roadmap, no como entregable del MVP (§2) |

La amplitud no se borra — se **reordena**. Hoy juega en contra porque diluye la tesis y multiplica la superficie de riesgo sin probar el núcleo.

**Nota:** la firma de plugins (B6) es parte del *núcleo honesto*: aunque no se "muestra" como feature, debe estar correcta para que el claim Citadel del núcleo no sea falso.

---

## 7. Lo que explícitamente NO bloquea el lanzamiento

Para proteger el foco (achicar scope, no expandirlo), **nada de esto debe demorar la validación de la tesis**:

- `distro/` / Aegis OS bare-metal → **es el norte (flagship v2), no un bloqueante del MVP** (§2).
- Escalado de LanceDB L3 / optimización fina del cognitive loop.
- Pulido de la app móvil.
- Matriz amplia de providers.
- Fase 3 de sincronización/replicación.

Son post-MVP. Si aparecen como precondición, es scope creep.

---

## 8. Límite de esta auditoría

Esta auditoría evaluó el **estado declarado** (governance docs) y la **cara pública** (README, estructura, `install.sh`, `distro/README.md`). **No es una auditoría de código.** No verifiqué que el binario haga lo que cada ticket dice.

Dado que la tesis *es* el cognitive loop, se recomienda un **pase de verificación acotado al núcleo** antes de publicar: confirmar contra el binario real (principio "verify the deployed binary": `strings` sobre el binario compilado para configs embebidas vía `rust-embed`) que el loop ReAct + tool use + spawn de agentes funciona end-to-end como se declara. Ese pase es el seguro contra la misma ilusión de completitud que motiva este documento.

---

## 9. Próximo paso (gated)

Una vez que el Owner valide el recorte (§6):

1. Crear **EPIC 56 — Public MVP / Thesis Validation** en governance.
2. Convertir **B1–B7** (y B8 si entra) en **tickets individuales** en `governance/Tickets/CORE-NNN.md`, cada uno con criterios de aceptación verificables.
3. Sincronizar `TICKETS_MASTER.md`.

El distro **no genera epic ahora** (es norte v2); su finalización se planifica post-validación como epic flagship aparte.

**No se crea el epic ni los tickets hasta validar §6.** Alineación antes de expandir scope.

---

*Arquitecto IA — 2026-05-31 — DRAFT v2*

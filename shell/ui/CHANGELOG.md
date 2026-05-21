# Changelog

## [1.22.1](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.22.0...shell-ui-v1.22.1) (2026-05-21)


### Bug Fixes

* **router,agents:** smoke-test hardening — supervisor delivery, tenant isolation, 429 & fallback ([#318](https://github.com/Gustavo324234/Aegis-Core/issues/318)) ([cf7af84](https://github.com/Gustavo324234/Aegis-Core/commit/cf7af842b65f2bdce45a94c921b72c8afe2d3524))

## [1.22.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.21.1...shell-ui-v1.22.0) (2026-05-21)


### Features

* **agents,ui:** per-project autonomous mode + configurable HTTP port ([#316](https://github.com/Gustavo324234/Aegis-Core/issues/316)) ([989a26e](https://github.com/Gustavo324234/Aegis-Core/commit/989a26e4f91ec8c6ee5088b682ed63e8ebfaf7d5))

## [1.21.1](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.21.0...shell-ui-v1.21.1) (2026-05-21)


### Bug Fixes

* **router,agents:** smoke-test routing, agent lifecycle & observability ([#314](https://github.com/Gustavo324234/Aegis-Core/issues/314)) ([7182b9c](https://github.com/Gustavo324234/Aegis-Core/commit/7182b9c6171a12b890bf8faadb758ef83e2017ed))

## [1.21.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.20.3...shell-ui-v1.21.0) (2026-05-20)


### Features

* **ui:** supervisor questions as a modal in the main chat (+ fix Q2 block) ([#307](https://github.com/Gustavo324234/Aegis-Core/issues/307)) ([7042537](https://github.com/Gustavo324234/Aegis-Core/commit/7042537b9a034405492c8ec522568732ad7e1a08))

## [1.20.3](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.20.2...shell-ui-v1.20.3) (2026-05-19)


### Bug Fixes

* **core:** robust model routing — key rotation, ollama protocol, gemini quota ([#303](https://github.com/Gustavo324234/Aegis-Core/issues/303)) ([2fad9d7](https://github.com/Gustavo324234/Aegis-Core/commit/2fad9d7ae29ceb223206720d05dd6f13f70221d2))

## [1.20.2](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.20.1...shell-ui-v1.20.2) (2026-05-18)


### Bug Fixes

* smoke-test bugs — onboarding parser, meta-token leak, voice echo, scoring, vocab ([#301](https://github.com/Gustavo324234/Aegis-Core/issues/301)) ([560f0f9](https://github.com/Gustavo324234/Aegis-Core/commit/560f0f97d59bf2103a0462cb87ec78786853846e))

## [1.20.1](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.20.0...shell-ui-v1.20.1) (2026-05-18)


### Bug Fixes

* smoke-test readiness — Gemini tool shape, modern model scores, Anthropic preset, UI errors ([#297](https://github.com/Gustavo324234/Aegis-Core/issues/297)) ([0451fb8](https://github.com/Gustavo324234/Aegis-Core/commit/0451fb871a49a8af993c89eb8b6d2180af8e0c4c))

## [1.20.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.19.2...shell-ui-v1.20.0) (2026-05-17)


### Features

* **ui:** CORE-245 admin provider toggle — enable/disable without deleting ([#290](https://github.com/Gustavo324234/Aegis-Core/issues/290)) ([e68c0fd](https://github.com/Gustavo324234/Aegis-Core/commit/e68c0fde43e2d8253d53b30db718cfabb7fbedb5))


### Bug Fixes

* **ui:** CORE-299 align chat timeout with ReAct loop — 120s + progressive loading indicator ([#287](https://github.com/Gustavo324234/Aegis-Core/issues/287)) ([ecd6f6a](https://github.com/Gustavo324234/Aegis-Core/commit/ecd6f6a512a2747c40e3c07365c484f53d9348c3))
* **ui:** CORE-301 AgentTreeWidget — differentiate empty vs error state, retry with backoff ([#289](https://github.com/Gustavo324234/Aegis-Core/issues/289)) ([deb8909](https://github.com/Gustavo324234/Aegis-Core/commit/deb8909e2aca6e9b00cb7e7ca284081b57b63f4e))

## [1.19.2](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.19.1...shell-ui-v1.19.2) (2026-05-17)


### Bug Fixes

* **ui:** CORE-294 correct BenchScore color thresholds to match spec ([#286](https://github.com/Gustavo324234/Aegis-Core/issues/286)) ([f741913](https://github.com/Gustavo324234/Aegis-Core/commit/f741913569e22172e8163bd0bf05ef47c6032e58))

## [1.19.1](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.19.0...shell-ui-v1.19.1) (2026-05-17)


### Bug Fixes

* **ui:** accept structured error events + render model_selected and warning ([#282](https://github.com/Gustavo324234/Aegis-Core/issues/282)) ([00628b4](https://github.com/Gustavo324234/Aegis-Core/commit/00628b46a17e1c0e1d69ed782b6151c429f6b92b))

## [1.19.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.18.0...shell-ui-v1.19.0) (2026-05-15)


### Features

* **voice:** CORE-302 + speaker verification + wake word activation by agent name ([#274](https://github.com/Gustavo324234/Aegis-Core/issues/274)) ([5f7ad96](https://github.com/Gustavo324234/Aegis-Core/commit/5f7ad96ff453b0b21d132e241a1340ca9e779663))

## [1.18.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.17.0...shell-ui-v1.18.0) (2026-05-14)


### Features

* **ui:** CORE-300 add model selector to chat input bar ([#271](https://github.com/Gustavo324234/Aegis-Core/issues/271)) ([c0e3090](https://github.com/Gustavo324234/Aegis-Core/commit/c0e309015c3456807bd6e57376bb20cba580e45f))

## [1.17.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.16.2...shell-ui-v1.17.0) (2026-05-14)


### Features

* **ui:** CORE-300 add model selector to chat input bar ([#269](https://github.com/Gustavo324234/Aegis-Core/issues/269)) ([82f23d8](https://github.com/Gustavo324234/Aegis-Core/commit/82f23d8761bcac18eddf60f0900dfcda766d653b))

## [1.16.2](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.16.1...shell-ui-v1.16.2) (2026-05-13)


### Bug Fixes

* **ui:** CORE-295 preserve stt_provider and stt_api_key when saving voice config ([#253](https://github.com/Gustavo324234/Aegis-Core/issues/253)) ([fa108fc](https://github.com/Gustavo324234/Aegis-Core/commit/fa108fcc8492652db48c01765be5ddb7fbc81a1d))

## [1.16.1](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.16.0...shell-ui-v1.16.1) (2026-05-09)


### Bug Fixes

* **ui:** CORE-292 add ollama_cloud to enginePresets and ProvidersTab icon grid ([#249](https://github.com/Gustavo324234/Aegis-Core/issues/249)) ([8845954](https://github.com/Gustavo324234/Aegis-Core/commit/88459541b000b78f403a016346fe6fcaed471cdb))

## [1.16.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.15.2...shell-ui-v1.16.0) (2026-05-09)


### Features

* **ui:** CORE-294 add benchmark score column and provider badges to CatalogViewer ([#247](https://github.com/Gustavo324234/Aegis-Core/issues/247)) ([cf611b8](https://github.com/Gustavo324234/Aegis-Core/commit/cf611b838f6ab0fa0c9e0ca24242281a0e9b2753))

## [1.15.2](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.15.1...shell-ui-v1.15.2) (2026-05-08)


### Bug Fixes

* **shell:** CORE-284 botón de reply al supervisor conectado al endpoint real ([#243](https://github.com/Gustavo324234/Aegis-Core/issues/243)) ([7edbece](https://github.com/Gustavo324234/Aegis-Core/commit/7edbece18f8925f7d8c85b04c0a645d322d15717))

## [1.15.1](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.15.0...shell-ui-v1.15.1) (2026-05-08)


### Bug Fixes

* **shell:** CORE-278 TTS en modo texto, toggle de voz simplificado ([#241](https://github.com/Gustavo324234/Aegis-Core/issues/241)) ([3110645](https://github.com/Gustavo324234/Aegis-Core/commit/31106454e21149d0a9d95197a717c6266c5bb1df))

## [1.15.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.14.0...shell-ui-v1.15.0) (2026-05-07)


### Features

* **shell:** CORE-269/274/270 AgentInbox store, badge, thread UI ([#231](https://github.com/Gustavo324234/Aegis-Core/issues/231)) ([0da0d3b](https://github.com/Gustavo324234/Aegis-Core/commit/0da0d3bcd414f41e825d2c726ec18d6d591f4a65))

## [1.14.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.13.0...shell-ui-v1.14.0) (2026-05-03)


### Features

* **shell:** CORE-246 Tenant — visualización de modelos en tab Motor ([#201](https://github.com/Gustavo324234/Aegis-Core/issues/201)) ([6dbf015](https://github.com/Gustavo324234/Aegis-Core/commit/6dbf01578666124ba294338ccc563c61abd4b9ea))

## [1.13.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.12.0...shell-ui-v1.13.0) (2026-05-03)


### Features

* **shell:** CORE-250 + CORE-251 ApiCostWidget y Chronos widget con datos reales ([#197](https://github.com/Gustavo324234/Aegis-Core/issues/197)) ([b4d42db](https://github.com/Gustavo324234/Aegis-Core/commit/b4d42dbd9af7a39757e5db7f9dd98efe4ec7d04d))

## [1.12.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.11.0...shell-ui-v1.12.0) (2026-05-03)


### Features

* **shell:** CORE-249 Kanban con proyectos reales — elimina MOCK_TICKETS ([#195](https://github.com/Gustavo324234/Aegis-Core/issues/195)) ([f4fbb3d](https://github.com/Gustavo324234/Aegis-Core/commit/f4fbb3d0981337fe96246dc5cb33c478355b44ac))

## [1.11.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.10.7...shell-ui-v1.11.0) (2026-05-03)


### Features

* **shell:** CORE-248 + CORE-252 Shell Observability — chat feedback y dashboard header ([#193](https://github.com/Gustavo324234/Aegis-Core/issues/193)) ([ea613ec](https://github.com/Gustavo324234/Aegis-Core/commit/ea613ec4cbc1cbeb5134f360d90efe58893c8733))

## [1.10.7](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.10.6...shell-ui-v1.10.7) (2026-05-02)


### Bug Fixes

* **ank-server:** CORE-241 send user-facing response after AgentToolCall execution ([#185](https://github.com/Gustavo324234/Aegis-Core/issues/185)) ([e6e4c5e](https://github.com/Gustavo324234/Aegis-Core/commit/e6e4c5efa27fcf7b681105eae583a6205120cf09))

## [1.10.6](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.10.5...shell-ui-v1.10.6) (2026-04-29)


### Bug Fixes

* **install:** agents config deploy ([#165](https://github.com/Gustavo324234/Aegis-Core/issues/165)) ([#166](https://github.com/Gustavo324234/Aegis-Core/issues/166)) ([905ca1b](https://github.com/Gustavo324234/Aegis-Core/commit/905ca1bb787f59a141114b538bda5daa014874dd))

## [1.10.5](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.10.4...shell-ui-v1.10.5) (2026-04-29)


### Bug Fixes

* allow Ollama/Custom providers without API key in admin panel ([#163](https://github.com/Gustavo324234/Aegis-Core/issues/163)) ([c5a37fd](https://github.com/Gustavo324234/Aegis-Core/commit/c5a37fdb88868610b722d7fd957051e76367139f))

## [1.10.4](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.10.3...shell-ui-v1.10.4) (2026-04-29)


### Bug Fixes

* **shell:** preserve stored API key when edit form submits empty api_key ([#161](https://github.com/Gustavo324234/Aegis-Core/issues/161)) ([1ad2110](https://github.com/Gustavo324234/Aegis-Core/commit/1ad2110c143374c6eca044626f9e3a510e03bd1a))

## [1.10.3](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.10.2...shell-ui-v1.10.3) (2026-04-29)


### Bug Fixes

* **shell:** Core 230 233 shell bugs ([#159](https://github.com/Gustavo324234/Aegis-Core/issues/159)) ([687f03e](https://github.com/Gustavo324234/Aegis-Core/commit/687f03e861abdcfc74f36a35f75befbf1887c3c9))

## [1.10.2](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.10.1...shell-ui-v1.10.2) (2026-04-28)


### Bug Fixes

* **shell:** CORE-212 provider gemini en KeyManager y visibilidad de modelos en CatalogViewer ([#148](https://github.com/Gustavo324234/Aegis-Core/issues/148)) ([08deb8a](https://github.com/Gustavo324234/Aegis-Core/commit/08deb8a35f9a2d4cf42c2c97d1b91d0696d2bdab))

## [1.10.1](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.10.0...shell-ui-v1.10.1) (2026-04-28)


### Bug Fixes

* **shell:** CORE-211 manejo graceful de errores en fetchActiveProjects y connectAgentStream ([#146](https://github.com/Gustavo324234/Aegis-Core/issues/146)) ([40df4f8](https://github.com/Gustavo324234/Aegis-Core/commit/40df4f86f7ca8fae17be04fd4558ded7add2c6f7))

## [1.10.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.9.0...shell-ui-v1.10.0) (2026-04-27)


### Features

* **router:** free-tier rate limiting, Gemini 3.x catalog and JS sandbox fixes ([#138](https://github.com/Gustavo324234/Aegis-Core/issues/138)) ([392cc69](https://github.com/Gustavo324234/Aegis-Core/commit/392cc697d76c7b4de75f473a9b3ea9fc94a178cd))
* **shell:** dashboard tree view ([#140](https://github.com/Gustavo324234/Aegis-Core/issues/140)) ([9788d1f](https://github.com/Gustavo324234/Aegis-Core/commit/9788d1fea31e695ac9cef9888c3e1e92171ef744))

## [1.9.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.8.1...shell-ui-v1.9.0) (2026-04-27)


### Features

* **shell:** CORE-202/203/204 agent activity panel + dashboard tree view ([#136](https://github.com/Gustavo324234/Aegis-Core/issues/136)) ([8ffc69f](https://github.com/Gustavo324234/Aegis-Core/commit/8ffc69f5abdd7b7fefc03ef2d81d8418087d8c21))

## [1.8.1](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.8.0...shell-ui-v1.8.1) (2026-04-26)


### Bug Fixes

* **shell:** CORE-184 modo conversación — sirenWs persistente + TTS loop sin botón ([#128](https://github.com/Gustavo324234/Aegis-Core/issues/128)) ([a0abeb0](https://github.com/Gustavo324234/Aegis-Core/commit/a0abeb05efcc62c26320e07e72d39e15d7a4da3e))

## [1.8.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.7.0...shell-ui-v1.8.0) (2026-04-25)


### Features

* **shell:** CORE-183 input mode selector — text / audio / conversation con TTS loop ([#126](https://github.com/Gustavo324234/Aegis-Core/issues/126)) ([046d2a9](https://github.com/Gustavo324234/Aegis-Core/commit/046d2a904920ed632e617f8bc210ab0071eadc62))

## [1.7.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.6.0...shell-ui-v1.7.0) (2026-04-25)


### Features

* **voice:** dual STT — Browser WebSpeech + Groq Cloud + Whisper Local ([#121](https://github.com/Gustavo324234/Aegis-Core/issues/121)) ([9c91fcb](https://github.com/Gustavo324234/Aegis-Core/commit/9c91fcbedd2c00a5dd8980811c356f1b77e55c78))

## [1.6.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.5.0...shell-ui-v1.6.0) (2026-04-25)


### Features

* **voice:** ElevenLabs TTS driver + Whisper STT model manager ([#119](https://github.com/Gustavo324234/Aegis-Core/issues/119)) ([ef26da9](https://github.com/Gustavo324234/Aegis-Core/commit/ef26da954c5f62c8bdd84c1474f1177aa0389a82))

## [1.5.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.4.0...shell-ui-v1.5.0) (2026-04-25)


### Features

* **epic-44:** Developer Workspace — terminal, code viewer, git bridge, PR manager ([#117](https://github.com/Gustavo324234/Aegis-Core/issues/117)) ([9ca9a10](https://github.com/Gustavo324234/Aegis-Core/commit/9ca9a10f3e4b03812f9c19caf31fb52d27f5e884))

## [1.4.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.3.0...shell-ui-v1.4.0) (2026-04-24)


### Features

* **agents:** Epic 43 — Hierarchical Multi-Agent Orchestration ([#115](https://github.com/Gustavo324234/Aegis-Core/issues/115)) ([6b640a7](https://github.com/Gustavo324234/Aegis-Core/commit/6b640a7f9ab53a7aa5f8111f00e8f6d8db8e9f59))

## [1.3.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.2.0...shell-ui-v1.3.0) (2026-04-24)


### Features

* **core-153:** implement dashboard with kanban and financial widgets ([#108](https://github.com/Gustavo324234/Aegis-Core/issues/108)) ([dbbcf37](https://github.com/Gustavo324234/Aegis-Core/commit/dbbcf377641086d4c40b39c3b9ba62eff9ea8087))

## [1.2.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.1.2...shell-ui-v1.2.0) (2026-04-22)


### Features

* **ank-server,installer:** CORE-146 Cloudflare tunnel + connection-i… ([#90](https://github.com/Gustavo324234/Aegis-Core/issues/90)) ([e8be602](https://github.com/Gustavo324234/Aegis-Core/commit/e8be60263bc0d4d8a4ab3fc9175badfb0982887c))
* **shell:** CORE-145 rename Persona tab to Identidad + reset hint ([#88](https://github.com/Gustavo324234/Aegis-Core/issues/88)) ([52ac202](https://github.com/Gustavo324234/Aegis-Core/commit/52ac202d0a0f9d62e73146fa0d613e872a4ed4db))

## [1.1.2](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.1.1...shell-ui-v1.1.2) (2026-04-22)


### Bug Fixes

* **core,http,installer:** music prompt always injected + TLS vars on … ([#81](https://github.com/Gustavo324234/Aegis-Core/issues/81)) ([141bc16](https://github.com/Gustavo324234/Aegis-Core/commit/141bc16c1303655eeb999a62305c0ffc82026ee2))

## [1.1.1](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.1.0...shell-ui-v1.1.1) (2026-04-21)


### Bug Fixes

* **shell:** music_play and music_control events read data directly, not data.payload ([#79](https://github.com/Gustavo324234/Aegis-Core/issues/79)) ([cc9706e](https://github.com/Gustavo324234/Aegis-Core/commit/cc9706e59573493fbddb488067248c4ee949145c))

## [1.1.0](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.0.8...shell-ui-v1.1.0) (2026-04-21)


### Features

* **core:** Epic 38-39-40 — Agent Persona, Music, Connected Accounts ([#76](https://github.com/Gustavo324234/Aegis-Core/issues/76)) ([b4ceb7d](https://github.com/Gustavo324234/Aegis-Core/commit/b4ceb7d77884109570e07fbf0577d88a113c4842))

## [1.0.8](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.0.7...shell-ui-v1.0.8) (2026-04-21)


### Bug Fixes

* **shell,ank-core:** CORE-126 CORE-128 layout fix and system prompt rewrite ([#66](https://github.com/Gustavo324234/Aegis-Core/issues/66)) ([313400a](https://github.com/Gustavo324234/Aegis-Core/commit/313400aa583034b7f2d7e48d9547171a20c6e2a1))

## [1.0.7](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.0.6...shell-ui-v1.0.7) (2026-04-21)


### Bug Fixes

* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([b7e76ae](https://github.com/Gustavo324234/Aegis-Core/commit/b7e76ae4486db4071105e274114e942657779502))

## [1.0.6](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.0.5...shell-ui-v1.0.6) (2026-04-20)


### Bug Fixes

* **ank-core:** CORE-092 fix silent cloud errors and implement provide… ([#52](https://github.com/Gustavo324234/Aegis-Core/issues/52)) ([a236021](https://github.com/Gustavo324234/Aegis-Core/commit/a2360213a99cd0ff582ab58d1c632b80a4754fd6))

## [1.0.5](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.0.4...shell-ui-v1.0.5) (2026-04-20)


### Bug Fixes

* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([46e0c6d](https://github.com/Gustavo324234/Aegis-Core/commit/46e0c6d1cae15666c1fcea9c283cc41a4cad2e11))
* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([2a5c5ea](https://github.com/Gustavo324234/Aegis-Core/commit/2a5c5ead0c92cf31dc22155037441f622f0e52e9))
* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([43b4f39](https://github.com/Gustavo324234/Aegis-Core/commit/43b4f39929e49f5644ccf3b0c0342189cb29d922))
* **ci:** trigger nightly build ([#44](https://github.com/Gustavo324234/Aegis-Core/issues/44)) ([2a5c5ea](https://github.com/Gustavo324234/Aegis-Core/commit/2a5c5ead0c92cf31dc22155037441f622f0e52e9))
* **ci:** trigger nightly build ([#45](https://github.com/Gustavo324234/Aegis-Core/issues/45)) ([46e0c6d](https://github.com/Gustavo324234/Aegis-Core/commit/46e0c6d1cae15666c1fcea9c283cc41a4cad2e11))

## [1.0.4](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.0.3...shell-ui-v1.0.4) (2026-04-17)


### Bug Fixes

* **ank-http:** ConnectInfo missing — use into_make_service_with_connect_info ([#36](https://github.com/Gustavo324234/Aegis-Core/issues/36)) ([cde193f](https://github.com/Gustavo324234/Aegis-Core/commit/cde193fb49e283660c7350c716222508db3fb4b0))

## [1.0.3](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.0.2...shell-ui-v1.0.3) (2026-04-17)


### Bug Fixes

* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([75e990b](https://github.com/Gustavo324234/Aegis-Core/commit/75e990b54c255f676d2853c1e0ffa530e91ea298))

## [1.0.2](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.0.1...shell-ui-v1.0.2) (2026-04-14)


### Bug Fixes

* **shell:** store version 2 + BootstrapSetup correct endpoint + engine CitadelAuthenticated ([#22](https://github.com/Gustavo324234/Aegis-Core/issues/22)) ([357b28e](https://github.com/Gustavo324234/Aegis-Core/commit/357b28efe195fb35a7bfe7eb2217db275110ff45))

## [1.0.1](https://github.com/Gustavo324234/Aegis-Core/compare/shell-ui-v1.0.0...shell-ui-v1.0.1) (2026-04-14)


### Bug Fixes

* **shell:** BootstrapSetup wrong endpoint /api/admin vs /api/auth ([#20](https://github.com/Gustavo324234/Aegis-Core/issues/20)) ([36ce121](https://github.com/Gustavo324234/Aegis-Core/commit/36ce12116a861ee928df62d0ab5e48a53f375d6d))

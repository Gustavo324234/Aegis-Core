# Changelog

## [0.1.32](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.31...ank-core-v0.1.32) (2026-05-01)


### Bug Fixes

* **ank-core:** CORE-240 wire ToolRegistry into CloudProxyDriver request ([#178](https://github.com/Gustavo324234/Aegis-Core/issues/178)) ([14924de](https://github.com/Gustavo324234/Aegis-Core/commit/14924de0ecda9d0bc939059cc2927d7fadeed82a))

## [0.1.31](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.30...ank-core-v0.1.31) (2026-05-01)


### Features

* **ank-core:** EPIC-47 agent protocol v2 — tool use dispatch ([#176](https://github.com/Gustavo324234/Aegis-Core/issues/176)) ([94b4909](https://github.com/Gustavo324234/Aegis-Core/commit/94b4909e0aa1afdd8c6c90554cd05271a2bbc6f9))

## [0.1.30](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.29...ank-core-v0.1.30) (2026-04-30)


### Bug Fixes

* **ank-core:** CORE-239 bare model_id strips provider prefix in CognitiveRouter ([#174](https://github.com/Gustavo324234/Aegis-Core/issues/174)) ([3852b04](https://github.com/Gustavo324234/Aegis-Core/commit/3852b0451c280f85a9df204ac673dac12b7f6c65))

## [0.1.29](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.28...ank-core-v0.1.29) (2026-04-30)


### Bug Fixes

* **ank-core:** key model id prefix mismatch ([#170](https://github.com/Gustavo324234/Aegis-Core/issues/170)) ([c39f98c](https://github.com/Gustavo324234/Aegis-Core/commit/c39f98c476577cf9a5bf39aa603ec6fd33f7f2d8))
* **syscalls:** resolver UUID vacío en SYS_AGENT_SPAWN y anti-alucinac… ([#171](https://github.com/Gustavo324234/Aegis-Core/issues/171)) ([b01dc63](https://github.com/Gustavo324234/Aegis-Core/commit/b01dc635078bb4832d693e4709cce914f0635f8a))

## [0.1.28](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.27...ank-core-v0.1.28) (2026-04-29)


### Bug Fixes

* **instructions:** silence WARN when agents config dir is absent in p… ([#168](https://github.com/Gustavo324234/Aegis-Core/issues/168)) ([854e3ee](https://github.com/Gustavo324234/Aegis-Core/commit/854e3ee322d585242d1b67ab4272526b297c52ae))

## [0.1.27](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.26...ank-core-v0.1.27) (2026-04-29)


### Bug Fixes

* **install:** agents config deploy ([#165](https://github.com/Gustavo324234/Aegis-Core/issues/165)) ([#166](https://github.com/Gustavo324234/Aegis-Core/issues/166)) ([905ca1b](https://github.com/Gustavo324234/Aegis-Core/commit/905ca1bb787f59a141114b538bda5daa014874dd))

## [0.1.26](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.25...ank-core-v0.1.26) (2026-04-29)


### Bug Fixes

* **shell:** preserve stored API key when edit form submits empty api_key ([#161](https://github.com/Gustavo324234/Aegis-Core/issues/161)) ([1ad2110](https://github.com/Gustavo324234/Aegis-Core/commit/1ad2110c143374c6eca044626f9e3a510e03bd1a))

## [0.1.25](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.24...ank-core-v0.1.25) (2026-04-29)


### Bug Fixes

* **ank-core:** CORE-227 unificar sintaxis de spawn al formato SYS_AGE… ([#155](https://github.com/Gustavo324234/Aegis-Core/issues/155)) ([670759d](https://github.com/Gustavo324234/Aegis-Core/commit/670759d9ed58b74dde0604b155fe7e4fe117638b))
* **ank-server:** CORE-228 conectar AgentOrchestrator al SyscallExecutor ([#158](https://github.com/Gustavo324234/Aegis-Core/issues/158)) ([6376e7e](https://github.com/Gustavo324234/Aegis-Core/commit/6376e7e98fe948e286ac87100594c2d18348be2b))

## [0.1.24](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.23...ank-core-v0.1.24) (2026-04-29)


### Bug Fixes

* **ank-core:** CORE-226 Chat Agent carga chat_agent.md via Instructio… ([#154](https://github.com/Gustavo324234/Aegis-Core/issues/154)) ([3726986](https://github.com/Gustavo324234/Aegis-Core/commit/37269866183bfd48aab6b83fd8af84662c21883c))

## [0.1.23](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.22...ank-core-v0.1.23) (2026-04-28)


### Bug Fixes

* **ank-http:** CORE-209 CORE-210 montar ws/agents, agregar /api/agents/projects y fix chat_agent fallback ([#145](https://github.com/Gustavo324234/Aegis-Core/issues/145)) ([581b543](https://github.com/Gustavo324234/Aegis-Core/commit/581b543348496c781d070a6bdaf4a4b6c0f2b848))

## [0.1.22](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.21...ank-core-v0.1.22) (2026-04-27)


### Features

* **router:** free-tier rate limiting, Gemini 3.x catalog and JS sandbox fixes ([#138](https://github.com/Gustavo324234/Aegis-Core/issues/138)) ([392cc69](https://github.com/Gustavo324234/Aegis-Core/commit/392cc697d76c7b4de75f473a9b3ea9fc94a178cd))
* **shell:** dashboard tree view ([#140](https://github.com/Gustavo324234/Aegis-Core/issues/140)) ([9788d1f](https://github.com/Gustavo324234/Aegis-Core/commit/9788d1fea31e695ac9cef9888c3e1e92171ef744))

## [0.1.21](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.20...ank-core-v0.1.21) (2026-04-27)


### Features

* **ank-core:** EPIC-45 Cognitive Agent Architecture — n-ary agent tree, orchestrator, persistence, CMR per-agent [#133](https://github.com/Gustavo324234/Aegis-Core/issues/133) ([#134](https://github.com/Gustavo324234/Aegis-Core/issues/134)) ([8968405](https://github.com/Gustavo324234/Aegis-Core/commit/8968405576eefa9fabb71fc286bf40f9f04eb5cc))

## [0.1.20](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.19...ank-core-v0.1.20) (2026-04-26)


### Features

* **ank-core:** CORE-186 EspeakEngine — TTS local con espeak-ng sin API key ([#130](https://github.com/Gustavo324234/Aegis-Core/issues/130)) ([3676cdc](https://github.com/Gustavo324234/Aegis-Core/commit/3676cdc0612b0d942e91a9585e291a020a12e30e))
* **ank-http:** CORE-185 TTS pipeline en WebSocket Siren — sintetizar respuesta y enviar chunks ([#132](https://github.com/Gustavo324234/Aegis-Core/issues/132)) ([e8f5d01](https://github.com/Gustavo324234/Aegis-Core/commit/e8f5d01470ec1a818c895a86af1a4ce82d57abbd))

## [0.1.19](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.18...ank-core-v0.1.19) (2026-04-25)


### Bug Fixes

* **ank-core:** CORE-181 MakerExecutor — wrap IIFE para top-level return + stub require con error descriptivo ([#123](https://github.com/Gustavo324234/Aegis-Core/issues/123)) ([7326d1f](https://github.com/Gustavo324234/Aegis-Core/commit/7326d1f812ec430aaa778efb0fb7cb5e27391042))

## [0.1.18](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.17...ank-core-v0.1.18) (2026-04-25)


### Features

* **voice:** dual STT — Browser WebSpeech + Groq Cloud + Whisper Local ([#121](https://github.com/Gustavo324234/Aegis-Core/issues/121)) ([9c91fcb](https://github.com/Gustavo324234/Aegis-Core/commit/9c91fcbedd2c00a5dd8980811c356f1b77e55c78))

## [0.1.17](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.16...ank-core-v0.1.17) (2026-04-25)


### Features

* **voice:** ElevenLabs TTS driver + Whisper STT model manager ([#119](https://github.com/Gustavo324234/Aegis-Core/issues/119)) ([ef26da9](https://github.com/Gustavo324234/Aegis-Core/commit/ef26da954c5f62c8bdd84c1474f1177aa0389a82))

## [0.1.16](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.15...ank-core-v0.1.16) (2026-04-25)


### Features

* **epic-44:** Developer Workspace — terminal, code viewer, git bridge, PR manager ([#117](https://github.com/Gustavo324234/Aegis-Core/issues/117)) ([9ca9a10](https://github.com/Gustavo324234/Aegis-Core/commit/9ca9a10f3e4b03812f9c19caf31fb52d27f5e884))

## [0.1.15](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.14...ank-core-v0.1.15) (2026-04-24)


### Features

* **agents:** Epic 43 — Hierarchical Multi-Agent Orchestration ([#115](https://github.com/Gustavo324234/Aegis-Core/issues/115)) ([6b640a7](https://github.com/Gustavo324234/Aegis-Core/commit/6b640a7f9ab53a7aa5f8111f00e8f6d8db8e9f59))

## [0.1.14](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.13...ank-core-v0.1.14) (2026-04-24)


### Bug Fixes

* **ws:** stream tokens to WebSocket event_broker during inference ([#113](https://github.com/Gustavo324234/Aegis-Core/issues/113)) ([beb0285](https://github.com/Gustavo324234/Aegis-Core/commit/beb0285e6b88a4b3c2c3b0c2340e6ced62ce74db))

## [0.1.13](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.12...ank-core-v0.1.13) (2026-04-24)


### Features

* **core-154:** implement multi-agent supervisor/worker orchestration ([#110](https://github.com/Gustavo324234/Aegis-Core/issues/110)) ([9abd10f](https://github.com/Gustavo324234/Aegis-Core/commit/9abd10f9154bd88247434e3eb2f626b834d60638))

## [0.1.12](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.11...ank-core-v0.1.12) (2026-04-23)


### Features

* **core-152:** implement domain plugins for ledger and chronos ([#106](https://github.com/Gustavo324234/Aegis-Core/issues/106)) ([c06f408](https://github.com/Gustavo324234/Aegis-Core/commit/c06f408d18e49fc946bf74d93026eb68b075978b))

## [0.1.11](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.10...ank-core-v0.1.11) (2026-04-22)


### Bug Fixes

* **core,http,installer:** music prompt always injected + TLS vars on … ([#81](https://github.com/Gustavo324234/Aegis-Core/issues/81)) ([141bc16](https://github.com/Gustavo324234/Aegis-Core/commit/141bc16c1303655eeb999a62305c0ffc82026ee2))

## [0.1.10](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.9...ank-core-v0.1.10) (2026-04-21)


### Features

* **core:** Epic 38-39-40 — Agent Persona, Music, Connected Accounts ([#76](https://github.com/Gustavo324234/Aegis-Core/issues/76)) ([b4ceb7d](https://github.com/Gustavo324234/Aegis-Core/commit/b4ceb7d77884109570e07fbf0577d88a113c4842))

## [0.1.9](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.8...ank-core-v0.1.9) (2026-04-21)


### Bug Fixes

* **ank-core:** CORE-128 rewrite system prompt ([#65](https://github.com/Gustavo324234/Aegis-Core/issues/65)) ([0cd7746](https://github.com/Gustavo324234/Aegis-Core/commit/0cd77468502b22dcd1eb0224fce463d090d8ca8f))

## [0.1.8](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.7...ank-core-v0.1.8) (2026-04-20)


### Bug Fixes

* **ank-core:** CORE-123 remove USER_PROCESS_INSTRUCTION tag from prompt ([#58](https://github.com/Gustavo324234/Aegis-Core/issues/58)) ([571ef8b](https://github.com/Gustavo324234/Aegis-Core/commit/571ef8bbcff687eadcc5219f66284567eca61021))

## [0.1.7](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.6...ank-core-v0.1.7) (2026-04-20)


### Bug Fixes

* **ank-core,installer:** CORE-121 CORE-122 ([#56](https://github.com/Gustavo324234/Aegis-Core/issues/56)) ([35dea95](https://github.com/Gustavo324234/Aegis-Core/commit/35dea954612095ccc3442033f72228853e0e8b41))

## [0.1.6](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.5...ank-core-v0.1.6) (2026-04-20)


### Bug Fixes

* **ank-core:** CORE-092 fix silent cloud errors and implement provide… ([#52](https://github.com/Gustavo324234/Aegis-Core/issues/52)) ([a236021](https://github.com/Gustavo324234/Aegis-Core/commit/a2360213a99cd0ff582ab58d1c632b80a4754fd6))

## [0.1.5](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.4...ank-core-v0.1.5) (2026-04-20)


### Bug Fixes

* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([95e2807](https://github.com/Gustavo324234/Aegis-Core/commit/95e28074f7fdcb43ac1ee598ba6dec5353e57ad1))
* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([46e0c6d](https://github.com/Gustavo324234/Aegis-Core/commit/46e0c6d1cae15666c1fcea9c283cc41a4cad2e11))
* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([2a5c5ea](https://github.com/Gustavo324234/Aegis-Core/commit/2a5c5ead0c92cf31dc22155037441f622f0e52e9))
* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([43b4f39](https://github.com/Gustavo324234/Aegis-Core/commit/43b4f39929e49f5644ccf3b0c0342189cb29d922))
* **ci:** trigger nightly build ([#44](https://github.com/Gustavo324234/Aegis-Core/issues/44)) ([2a5c5ea](https://github.com/Gustavo324234/Aegis-Core/commit/2a5c5ead0c92cf31dc22155037441f622f0e52e9))
* **ci:** trigger nightly build ([#45](https://github.com/Gustavo324234/Aegis-Core/issues/45)) ([46e0c6d](https://github.com/Gustavo324234/Aegis-Core/commit/46e0c6d1cae15666c1fcea9c283cc41a4cad2e11))

## [0.1.4](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.3...ank-core-v0.1.4) (2026-04-17)


### Bug Fixes

* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([75e990b](https://github.com/Gustavo324234/Aegis-Core/commit/75e990b54c255f676d2853c1e0ffa530e91ea298))

## [0.1.3](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.2...ank-core-v0.1.3) (2026-04-14)


### Bug Fixes

* **ank-core:** CORE-090 consume setup token only after successful ini… ([#18](https://github.com/Gustavo324234/Aegis-Core/issues/18)) ([16f6b97](https://github.com/Gustavo324234/Aegis-Core/commit/16f6b97dff99fde83cf0e45c7ec7a6e97056a342))

## [0.1.2](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.1...ank-core-v0.1.2) (2026-04-14)


### Bug Fixes

* **ank-core:** CORE-090 WAL checkpoint race in initialize_master ([#16](https://github.com/Gustavo324234/Aegis-Core/issues/16)) ([21915d5](https://github.com/Gustavo324234/Aegis-Core/commit/21915d55ba9d28767521b497855107a8c2a16ced))

## [0.1.1](https://github.com/Gustavo324234/Aegis-Core/compare/ank-core-v0.1.0...ank-core-v0.1.1) (2026-04-14)


### Bug Fixes

* **ank-core:** CORE-090 WAL checkpoint race in initialize_master ([#16](https://github.com/Gustavo324234/Aegis-Core/issues/16)) ([21915d5](https://github.com/Gustavo324234/Aegis-Core/commit/21915d55ba9d28767521b497855107a8c2a16ced))

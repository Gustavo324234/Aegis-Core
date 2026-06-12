# Changelog



























## [0.1.83](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.82...ank-http-v0.1.83) (2026-06-12)


### Features

* **core:** CMR hardening + scoring v3/v3.1, tracker persistence, stats endpoint and stabilization criticals (CORE-319..325) ([#333](https://github.com/Gustavo324234/Aegis-Core/issues/333)) ([a4707e0](https://github.com/Gustavo324234/Aegis-Core/commit/a4707e0b1d5f864a3ac6c1061d18700755c004cf))

## [0.1.82](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.81...ank-http-v0.1.82) (2026-06-04)


### Features

* **core:** implement encrypted key backup, restore provider persistence, fix windows SCM handshake, and resolve security audits ([#331](https://github.com/Gustavo324234/Aegis-Core/issues/331)) ([ff6b056](https://github.com/Gustavo324234/Aegis-Core/commit/ff6b05621d53ec12a765dbe083ab45a5e0784b99))

## [0.1.80](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.79...ank-http-v0.1.80) (2026-05-31)


### Features

* **security:** isolate agent logs and traces by tenant and implement secure logs tab ([#327](https://github.com/Gustavo324234/Aegis-Core/issues/327)) ([b9b1223](https://github.com/Gustavo324234/Aegis-Core/commit/b9b1223cde1a308cc5050fe2587ae8be8ac89d83))

## [0.1.79](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.78...ank-http-v0.1.79) (2026-05-28)


### Features

* **ux:** implement responsive layouts and secure tenant logs viewer ([#325](https://github.com/Gustavo324234/Aegis-Core/issues/325)) ([037aae9](https://github.com/Gustavo324234/Aegis-Core/commit/037aae9f12120c2e000ff37a0134c1872c845a2e))

## [0.1.78](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.77...ank-http-v0.1.78) (2026-05-27)


### Features

* complete epics 47-55 realignment, mobile Orion ([#322](https://github.com/Gustavo324234/Aegis-Core/issues/322)) ([1f1c37e](https://github.com/Gustavo324234/Aegis-Core/commit/1f1c37edf4f4b7789b5dd34b69765f41a1baed12))

## [0.1.77](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.76...ank-http-v0.1.77) (2026-05-24)


### Features

* **core:** integrate script sandbox, WebRTC voice migration, and provider key pooling ([#320](https://github.com/Gustavo324234/Aegis-Core/issues/320)) ([1932637](https://github.com/Gustavo324234/Aegis-Core/commit/19326379e2817ad7d6c20d368b29cebb92e5e58d))

## [0.1.76](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.75...ank-http-v0.1.76) (2026-05-21)


### Bug Fixes

* **router,agents:** smoke-test hardening — supervisor delivery, tenant isolation, 429 & fallback ([#318](https://github.com/Gustavo324234/Aegis-Core/issues/318)) ([cf7af84](https://github.com/Gustavo324234/Aegis-Core/commit/cf7af842b65f2bdce45a94c921b72c8afe2d3524))

## [0.1.75](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.74...ank-http-v0.1.75) (2026-05-21)


### Features

* **agents,ui:** per-project autonomous mode + configurable HTTP port ([#316](https://github.com/Gustavo324234/Aegis-Core/issues/316)) ([989a26e](https://github.com/Gustavo324234/Aegis-Core/commit/989a26e4f91ec8c6ee5088b682ed63e8ebfaf7d5))

## [0.1.74](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.73...ank-http-v0.1.74) (2026-05-21)


### Bug Fixes

* **router,agents:** smoke-test routing, agent lifecycle & observability ([#314](https://github.com/Gustavo324234/Aegis-Core/issues/314)) ([7182b9c](https://github.com/Gustavo324234/Aegis-Core/commit/7182b9c6171a12b890bf8faadb758ef83e2017ed))

## [0.1.72](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.71...ank-http-v0.1.72) (2026-05-20)


### Bug Fixes

* **router,enclave:** resilient routing + tenant DB recovery after password reset ([#310](https://github.com/Gustavo324234/Aegis-Core/issues/310)) ([e1110e8](https://github.com/Gustavo324234/Aegis-Core/commit/e1110e8ccf1602b90cc97ed6a5c3648071870147))

## [0.1.69](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.68...ank-http-v0.1.69) (2026-05-18)


### Bug Fixes

* smoke-test bugs — onboarding parser, meta-token leak, voice echo, scoring, vocab ([#301](https://github.com/Gustavo324234/Aegis-Core/issues/301)) ([560f0f9](https://github.com/Gustavo324234/Aegis-Core/commit/560f0f97d59bf2103a0462cb87ec78786853846e))

## [0.1.68](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.67...ank-http-v0.1.68) (2026-05-18)


### Features

* end-to-end provider flow ready for smoke test (discovery, normalization, cooldown) ([#299](https://github.com/Gustavo324234/Aegis-Core/issues/299)) ([75d1ec7](https://github.com/Gustavo324234/Aegis-Core/commit/75d1ec7dcd8fee98b71d65784b004f1f9fe0d889))

## [0.1.67](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.66...ank-http-v0.1.67) (2026-05-18)


### Bug Fixes

* smoke-test readiness — Gemini tool shape, modern model scores, Anthropic preset, UI errors ([#297](https://github.com/Gustavo324234/Aegis-Core/issues/297)) ([0451fb8](https://github.com/Gustavo324234/Aegis-Core/commit/0451fb871a49a8af993c89eb8b6d2180af8e0c4c))

## [0.1.66](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.65...ank-http-v0.1.66) (2026-05-17)


### Features

* **core:** stability pass — cancel propagation, VCM symlink guard, scheduler GC ([#292](https://github.com/Gustavo324234/Aegis-Core/issues/292)) ([618c58b](https://github.com/Gustavo324234/Aegis-Core/commit/618c58b4e8f255e7c0ee18a77fed6cf0d9a1e9e0))

## [0.1.64](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.63...ank-http-v0.1.64) (2026-05-17)


### Features

* **router:** auto-discover provider models when a key is added ([#280](https://github.com/Gustavo324234/Aegis-Core/issues/280)) ([010b562](https://github.com/Gustavo324234/Aegis-Core/commit/010b562d5cb53d4179d2a00075e688761162bc76))

## [0.1.63](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.62...ank-http-v0.1.63) (2026-05-17)


### Bug Fixes

* re-trigger release-please after squash merge of [#277](https://github.com/Gustavo324234/Aegis-Core/issues/277) ([#278](https://github.com/Gustavo324234/Aegis-Core/issues/278)) ([4d00193](https://github.com/Gustavo324234/Aegis-Core/commit/4d001936728e576e4bc5917f02b704af89de0ed0))

## [0.1.62](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.61...ank-http-v0.1.62) (2026-05-15)


### Features

* **voice:** CORE-302 + speaker verification + wake word activation by agent name ([#274](https://github.com/Gustavo324234/Aegis-Core/issues/274)) ([5f7ad96](https://github.com/Gustavo324234/Aegis-Core/commit/5f7ad96ff453b0b21d132e241a1340ca9e779663))

## [0.1.61](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.60...ank-http-v0.1.61) (2026-05-14)


### Features

* **ui:** CORE-300 add model selector to chat input bar ([#271](https://github.com/Gustavo324234/Aegis-Core/issues/271)) ([c0e3090](https://github.com/Gustavo324234/Aegis-Core/commit/c0e309015c3456807bd6e57376bb20cba580e45f))

## [0.1.60](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.59...ank-http-v0.1.60) (2026-05-14)


### Features

* **ank-core:** CORE-298 sync OpenRouter free models on key registration ([#267](https://github.com/Gustavo324234/Aegis-Core/issues/267)) ([49ac9c7](https://github.com/Gustavo324234/Aegis-Core/commit/49ac9c7775a6f8754f730d904adebb2f19179eec))

## [0.1.59](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.58...ank-http-v0.1.59) (2026-05-14)


### Features

* **ank-core:** CORE-299 add model_override to PCB and WebSocket chat handler ([#265](https://github.com/Gustavo324234/Aegis-Core/issues/265)) ([21db419](https://github.com/Gustavo324234/Aegis-Core/commit/21db419ba06f835274dd5ccc582339f484f335df))

## [0.1.58](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.57...ank-http-v0.1.58) (2026-05-13)


### Features

* **ank-http:** CORE-256 service management endpoints — status, restart, stop ([#256](https://github.com/Gustavo324234/Aegis-Core/issues/256)) ([85baa1c](https://github.com/Gustavo324234/Aegis-Core/commit/85baa1c571daa05b48f020355d59d91593f20497))

## [0.1.56](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.55...ank-http-v0.1.56) (2026-05-09)


### Features

* **ui:** CORE-294 add benchmark score column and provider badges to CatalogViewer ([#247](https://github.com/Gustavo324234/Aegis-Core/issues/247)) ([cf611b8](https://github.com/Gustavo324234/Aegis-Core/commit/cf611b838f6ab0fa0c9e0ca24242281a0e9b2753))

## [0.1.55](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.54...ank-http-v0.1.55) (2026-05-09)


### Features

* **ank-core:** CORE-292 — ollama_cloud provider (remote API + SSRF allowlist) ([#245](https://github.com/Gustavo324234/Aegis-Core/issues/245)) ([eacdada](https://github.com/Gustavo324234/Aegis-Core/commit/eacdada0042833c2558786104ffae15875f2a2d5))

## [0.1.52](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.51...ank-http-v0.1.52) (2026-05-08)


### Bug Fixes

* **ank-http,agents,installer:** CORE-279/281/280 WS keepalive, supervisor dedup, Caddy HTTPS ([#235](https://github.com/Gustavo324234/Aegis-Core/issues/235)) ([9450c7d](https://github.com/Gustavo324234/Aegis-Core/commit/9450c7dd3249a7945765dfedc1d0ca7b7c958719))

## [0.1.51](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.50...ank-http-v0.1.51) (2026-05-07)


### Features

* **ank-core,ank-http:** CORE-271/272 — respuesta directa a supervisor + get_project_ledger ([#233](https://github.com/Gustavo324234/Aegis-Core/issues/233)) ([8d62b35](https://github.com/Gustavo324234/Aegis-Core/commit/8d62b357897b7833c53caa56c637690369e2a70d))

## [0.1.50](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.49...ank-http-v0.1.50) (2026-05-07)


### Features

* **ank-core,ank-http:** CORE-276/277/268 — approved paths + web_search + AgentEvents WebSocket ([#229](https://github.com/Gustavo324234/Aegis-Core/issues/229)) ([56e8f79](https://github.com/Gustavo324234/Aegis-Core/commit/56e8f7955d4423f71280310c45b5b5882205e9b1))

## [0.1.45](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.44...ank-http-v0.1.45) (2026-05-05)


### Features

* **ank-core:** CORE-260 PCB message_history + cache de sesión en WebSocket ([#210](https://github.com/Gustavo324234/Aegis-Core/issues/210)) ([da6a627](https://github.com/Gustavo324234/Aegis-Core/commit/da6a62764704dff902f55ccd00edca7100983e37))


### Bug Fixes

* **installer:** windows and app ([#208](https://github.com/Gustavo324234/Aegis-Core/issues/208)) ([b8be87b](https://github.com/Gustavo324234/Aegis-Core/commit/b8be87b98796d50ec7018bcfb958526de5d46b17))

## [0.1.44](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.43...ank-http-v0.1.44) (2026-05-03)


### Features

* **kernel:** CORE-247 endpoint GET /api/chat/history ([#199](https://github.com/Gustavo324234/Aegis-Core/issues/199)) ([97142d0](https://github.com/Gustavo324234/Aegis-Core/commit/97142d0a35e308c016efad3c62314aa957d5bd62))

## [0.1.43](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.42...ank-http-v0.1.43) (2026-05-03)

## [0.1.42](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.41...ank-http-v0.1.42) (2026-05-02)

## [0.1.41](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.40...ank-http-v0.1.41) (2026-05-02)


### Bug Fixes

* **ank-server:** CORE-241 send user-facing response after AgentToolCall execution ([#185](https://github.com/Gustavo324234/Aegis-Core/issues/185)) ([e6e4c5e](https://github.com/Gustavo324234/Aegis-Core/commit/e6e4c5efa27fcf7b681105eae583a6205120cf09))

## [0.1.40](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.39...ank-http-v0.1.40) (2026-05-02)

## [0.1.39](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.38...ank-http-v0.1.39) (2026-05-02)

## [0.1.38](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.37...ank-http-v0.1.38) (2026-05-01)

## [0.1.37](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.36...ank-http-v0.1.37) (2026-05-01)

## [0.1.36](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.35...ank-http-v0.1.36) (2026-04-30)

## [0.1.35](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.34...ank-http-v0.1.35) (2026-04-30)

## [0.1.34](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.33...ank-http-v0.1.34) (2026-04-29)

## [0.1.33](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.32...ank-http-v0.1.33) (2026-04-29)


### Bug Fixes

* **install:** agents config deploy ([#165](https://github.com/Gustavo324234/Aegis-Core/issues/165)) ([#166](https://github.com/Gustavo324234/Aegis-Core/issues/166)) ([905ca1b](https://github.com/Gustavo324234/Aegis-Core/commit/905ca1bb787f59a141114b538bda5daa014874dd))

## [0.1.32](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.31...ank-http-v0.1.32) (2026-04-29)


### Bug Fixes

* allow Ollama/Custom providers without API key in admin panel ([#163](https://github.com/Gustavo324234/Aegis-Core/issues/163)) ([c5a37fd](https://github.com/Gustavo324234/Aegis-Core/commit/c5a37fdb88868610b722d7fd957051e76367139f))

## [0.1.31](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.30...ank-http-v0.1.31) (2026-04-29)


### Bug Fixes

* **shell:** preserve stored API key when edit form submits empty api_key ([#161](https://github.com/Gustavo324234/Aegis-Core/issues/161)) ([1ad2110](https://github.com/Gustavo324234/Aegis-Core/commit/1ad2110c143374c6eca044626f9e3a510e03bd1a))

## [0.1.30](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.29...ank-http-v0.1.30) (2026-04-29)

## [0.1.29](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.28...ank-http-v0.1.29) (2026-04-29)

## [0.1.28](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.27...ank-http-v0.1.28) (2026-04-28)


### Bug Fixes

* **ank-http:** CORE-209 CORE-210 montar ws/agents, agregar /api/agents/projects y fix chat_agent fallback ([#143](https://github.com/Gustavo324234/Aegis-Core/issues/143)) ([78ac6d1](https://github.com/Gustavo324234/Aegis-Core/commit/78ac6d15836be0874135490bf6a689dc7abe23ea))

## [0.1.27](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.26...ank-http-v0.1.27) (2026-04-27)


### Features

* **router:** free-tier rate limiting, Gemini 3.x catalog and JS sandbox fixes ([#138](https://github.com/Gustavo324234/Aegis-Core/issues/138)) ([392cc69](https://github.com/Gustavo324234/Aegis-Core/commit/392cc697d76c7b4de75f473a9b3ea9fc94a178cd))
* **shell:** dashboard tree view ([#140](https://github.com/Gustavo324234/Aegis-Core/issues/140)) ([9788d1f](https://github.com/Gustavo324234/Aegis-Core/commit/9788d1fea31e695ac9cef9888c3e1e92171ef744))

## [0.1.26](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.25...ank-http-v0.1.26) (2026-04-27)

## [0.1.25](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.24...ank-http-v0.1.25) (2026-04-26)


### Features

* **ank-http:** CORE-185 TTS pipeline en WebSocket Siren — sintetizar respuesta y enviar chunks ([#132](https://github.com/Gustavo324234/Aegis-Core/issues/132)) ([e8f5d01](https://github.com/Gustavo324234/Aegis-Core/commit/e8f5d01470ec1a818c895a86af1a4ce82d57abbd))

## [0.1.24](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.23...ank-http-v0.1.24) (2026-04-25)

## [0.1.23](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.22...ank-http-v0.1.23) (2026-04-25)


### Features

* **voice:** dual STT — Browser WebSpeech + Groq Cloud + Whisper Local ([#121](https://github.com/Gustavo324234/Aegis-Core/issues/121)) ([9c91fcb](https://github.com/Gustavo324234/Aegis-Core/commit/9c91fcbedd2c00a5dd8980811c356f1b77e55c78))

## [0.1.22](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.21...ank-http-v0.1.22) (2026-04-25)


### Features

* **voice:** ElevenLabs TTS driver + Whisper STT model manager ([#119](https://github.com/Gustavo324234/Aegis-Core/issues/119)) ([ef26da9](https://github.com/Gustavo324234/Aegis-Core/commit/ef26da954c5f62c8bdd84c1474f1177aa0389a82))

## [0.1.21](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.20...ank-http-v0.1.21) (2026-04-25)


### Features

* **epic-44:** Developer Workspace — terminal, code viewer, git bridge, PR manager ([#117](https://github.com/Gustavo324234/Aegis-Core/issues/117)) ([9ca9a10](https://github.com/Gustavo324234/Aegis-Core/commit/9ca9a10f3e4b03812f9c19caf31fb52d27f5e884))

## [0.1.20](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.19...ank-http-v0.1.20) (2026-04-24)


### Features

* **agents:** Epic 43 — Hierarchical Multi-Agent Orchestration ([#115](https://github.com/Gustavo324234/Aegis-Core/issues/115)) ([6b640a7](https://github.com/Gustavo324234/Aegis-Core/commit/6b640a7f9ab53a7aa5f8111f00e8f6d8db8e9f59))

## [0.1.19](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.18...ank-http-v0.1.19) (2026-04-24)

## [0.1.18](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.17...ank-http-v0.1.18) (2026-04-24)


### Features

* **core-154:** implement multi-agent supervisor/worker orchestration ([#110](https://github.com/Gustavo324234/Aegis-Core/issues/110)) ([9abd10f](https://github.com/Gustavo324234/Aegis-Core/commit/9abd10f9154bd88247434e3eb2f626b834d60638))

## [0.1.17](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.16...ank-http-v0.1.17) (2026-04-23)

## [0.1.16](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.15...ank-http-v0.1.16) (2026-04-22)


### Features

* **ank-server,installer:** CORE-146 Cloudflare tunnel + connection-i… ([#90](https://github.com/Gustavo324234/Aegis-Core/issues/90)) ([e8be602](https://github.com/Gustavo324234/Aegis-Core/commit/e8be60263bc0d4d8a4ab3fc9175badfb0982887c))

## [0.1.15](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.14...ank-http-v0.1.15) (2026-04-22)


### Bug Fixes

* **core,http,installer:** music prompt always injected + TLS vars on … ([#81](https://github.com/Gustavo324234/Aegis-Core/issues/81)) ([141bc16](https://github.com/Gustavo324234/Aegis-Core/commit/141bc16c1303655eeb999a62305c0ffc82026ee2))

## [0.1.14](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.13...ank-http-v0.1.14) (2026-04-21)


### Features

* **core:** Epic 38-39-40 — Agent Persona, Music, Connected Accounts ([#76](https://github.com/Gustavo324234/Aegis-Core/issues/76)) ([b4ceb7d](https://github.com/Gustavo324234/Aegis-Core/commit/b4ceb7d77884109570e07fbf0577d88a113c4842))

## [0.1.13](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.12...ank-http-v0.1.13) (2026-04-21)


### Bug Fixes

* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([b7e76ae](https://github.com/Gustavo324234/Aegis-Core/commit/b7e76ae4486db4071105e274114e942657779502))

## [0.1.12](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.11...ank-http-v0.1.12) (2026-04-20)

## [0.1.11](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.10...ank-http-v0.1.11) (2026-04-20)

## [0.1.10](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.9...ank-http-v0.1.10) (2026-04-20)


### Bug Fixes

* **ank-core:** CORE-092 fix silent cloud errors and implement provide… ([#52](https://github.com/Gustavo324234/Aegis-Core/issues/52)) ([a236021](https://github.com/Gustavo324234/Aegis-Core/commit/a2360213a99cd0ff582ab58d1c632b80a4754fd6))

## [0.1.9](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.8...ank-http-v0.1.9) (2026-04-20)


### Bug Fixes

* **ank-http:** subscribe to broadcast channel before scheduler dispat… ([#50](https://github.com/Gustavo324234/Aegis-Core/issues/50)) ([302c25c](https://github.com/Gustavo324234/Aegis-Core/commit/302c25cad8a80ebeeeb27b40a04bbf64d94af64f))

## [0.1.8](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.7...ank-http-v0.1.8) (2026-04-20)


### Bug Fixes

* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([46e0c6d](https://github.com/Gustavo324234/Aegis-Core/commit/46e0c6d1cae15666c1fcea9c283cc41a4cad2e11))
* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([2a5c5ea](https://github.com/Gustavo324234/Aegis-Core/commit/2a5c5ead0c92cf31dc22155037441f622f0e52e9))
* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([43b4f39](https://github.com/Gustavo324234/Aegis-Core/commit/43b4f39929e49f5644ccf3b0c0342189cb29d922))
* **ci:** trigger nightly build ([#44](https://github.com/Gustavo324234/Aegis-Core/issues/44)) ([2a5c5ea](https://github.com/Gustavo324234/Aegis-Core/commit/2a5c5ead0c92cf31dc22155037441f622f0e52e9))
* **ci:** trigger nightly build ([#45](https://github.com/Gustavo324234/Aegis-Core/issues/45)) ([46e0c6d](https://github.com/Gustavo324234/Aegis-Core/commit/46e0c6d1cae15666c1fcea9c283cc41a4cad2e11))

## [0.1.7](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.6...ank-http-v0.1.7) (2026-04-17)


### Bug Fixes

* **ank-http:** ConnectInfo missing — use into_make_service_with_connect_info ([#36](https://github.com/Gustavo324234/Aegis-Core/issues/36)) ([cde193f](https://github.com/Gustavo324234/Aegis-Core/commit/cde193fb49e283660c7350c716222508db3fb4b0))

## [0.1.6](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.5...ank-http-v0.1.6) (2026-04-17)


### Bug Fixes

* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([75e990b](https://github.com/Gustavo324234/Aegis-Core/commit/75e990b54c255f676d2853c1e0ffa530e91ea298))

## [0.1.5](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.4...ank-http-v0.1.5) (2026-04-14)


### Bug Fixes

* **shell:** store version 2 + BootstrapSetup correct endpoint + engine CitadelAuthenticated ([#22](https://github.com/Gustavo324234/Aegis-Core/issues/22)) ([357b28e](https://github.com/Gustavo324234/Aegis-Core/commit/357b28efe195fb35a7bfe7eb2217db275110ff45))

## [0.1.4](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.3...ank-http-v0.1.4) (2026-04-14)


### Bug Fixes

* **ank-core:** CORE-090 consume setup token only after successful ini… ([#18](https://github.com/Gustavo324234/Aegis-Core/issues/18)) ([16f6b97](https://github.com/Gustavo324234/Aegis-Core/commit/16f6b97dff99fde83cf0e45c7ec7a6e97056a342))

## [0.1.3](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.2...ank-http-v0.1.3) (2026-04-14)


### Features

* implement authentication and system status API routes ([#9](https://github.com/Gustavo324234/Aegis-Core/issues/9)) ([1c02576](https://github.com/Gustavo324234/Aegis-Core/commit/1c02576de6b0e1cab29fe7a5c42f1c9e8b46047c))

## [0.1.1](https://github.com/Gustavo324234/Aegis-Core/compare/ank-http-v0.1.0...ank-http-v0.1.1) (2026-04-11)


### Features

* implement authentication and system status API routes ([#9](https://github.com/Gustavo324234/Aegis-Core/issues/9)) ([1c02576](https://github.com/Gustavo324234/Aegis-Core/commit/1c02576de6b0e1cab29fe7a5c42f1c9e8b46047c))

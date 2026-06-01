# Changelog



















































## [0.1.85](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.84...ank-server-v0.1.85) (2026-06-01)


### Bug Fixes

* **core:** resolve infinite synthesis loop, secure onboarding SHA256, and reconcile governance. ([#329](https://github.com/Gustavo324234/Aegis-Core/issues/329)) ([48041f0](https://github.com/Gustavo324234/Aegis-Core/commit/48041f0ae346ed6f1c9c7ff52e0d83c6aa12644d))

## [0.1.84](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.83...ank-server-v0.1.84) (2026-05-31)


### Features

* **security:** isolate agent logs and traces by tenant and implement secure logs tab ([#327](https://github.com/Gustavo324234/Aegis-Core/issues/327)) ([b9b1223](https://github.com/Gustavo324234/Aegis-Core/commit/b9b1223cde1a308cc5050fe2587ae8be8ac89d83))

## [0.1.82](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.81...ank-server-v0.1.82) (2026-05-27)


### Features

* complete epics 47-55 realignment, mobile Orion ([#322](https://github.com/Gustavo324234/Aegis-Core/issues/322)) ([1f1c37e](https://github.com/Gustavo324234/Aegis-Core/commit/1f1c37edf4f4b7789b5dd34b69765f41a1baed12))

## [0.1.80](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.79...ank-server-v0.1.80) (2026-05-21)


### Bug Fixes

* **router,agents:** smoke-test hardening — supervisor delivery, tenant isolation, 429 & fallback ([#318](https://github.com/Gustavo324234/Aegis-Core/issues/318)) ([cf7af84](https://github.com/Gustavo324234/Aegis-Core/commit/cf7af842b65f2bdce45a94c921b72c8afe2d3524))

## [0.1.79](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.78...ank-server-v0.1.79) (2026-05-21)


### Features

* **agents,ui:** per-project autonomous mode + configurable HTTP port ([#316](https://github.com/Gustavo324234/Aegis-Core/issues/316)) ([989a26e](https://github.com/Gustavo324234/Aegis-Core/commit/989a26e4f91ec8c6ee5088b682ed63e8ebfaf7d5))

## [0.1.73](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.72...ank-server-v0.1.73) (2026-05-18)


### Bug Fixes

* smoke-test bugs — onboarding parser, meta-token leak, voice echo, scoring, vocab ([#301](https://github.com/Gustavo324234/Aegis-Core/issues/301)) ([560f0f9](https://github.com/Gustavo324234/Aegis-Core/commit/560f0f97d59bf2103a0462cb87ec78786853846e))

## [0.1.65](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.64...ank-server-v0.1.65) (2026-05-14)


### Features

* **ui:** CORE-300 add model selector to chat input bar ([#271](https://github.com/Gustavo324234/Aegis-Core/issues/271)) ([c0e3090](https://github.com/Gustavo324234/Aegis-Core/commit/c0e309015c3456807bd6e57376bb20cba580e45f))

## [0.1.62](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.61...ank-server-v0.1.62) (2026-05-13)


### Bug Fixes

* **ank-core:** CORE-257 TunnelManager — skip retry loop if cloudflared binary not found ([#258](https://github.com/Gustavo324234/Aegis-Core/issues/258)) ([5d446da](https://github.com/Gustavo324234/Aegis-Core/commit/5d446da2fd4a51617f483cd82ad9132f172cb476))

## [0.1.58](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.57...ank-server-v0.1.58) (2026-05-08)


### Bug Fixes

* **agents,installer:** CORE-285/286/287/288/289 — orchestrator fixes + provider config ([#239](https://github.com/Gustavo324234/Aegis-Core/issues/239)) ([c5f388e](https://github.com/Gustavo324234/Aegis-Core/commit/c5f388e4c1423a1f27b9f0f8b703c51edf36c9d5))

## [0.1.52](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.51...ank-server-v0.1.52) (2026-05-06)


### Bug Fixes

* **ank-core:** CORE-267 mark_rate_limited al recibir 429 en CloudProxyDriver ([#226](https://github.com/Gustavo324234/Aegis-Core/issues/226)) ([8d2aec4](https://github.com/Gustavo324234/Aegis-Core/commit/8d2aec4f196efbec337bfd3c8e2c72fc9a3d07f8))
* **ank-server:** CORE-266 — Windows SCM handshake via --service flag ([#224](https://github.com/Gustavo324234/Aegis-Core/issues/224)) ([8b1e1a4](https://github.com/Gustavo324234/Aegis-Core/commit/8b1e1a4ed33d3398dac19f4ff8d111a1ac9c0d2e))

## [0.1.51](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.50...ank-server-v0.1.51) (2026-05-06)


### Bug Fixes

* **ank-server:** CORE-265 — load aegis.env on startup for Windows service compatibility ([#218](https://github.com/Gustavo324234/Aegis-Core/issues/218)) ([796a664](https://github.com/Gustavo324234/Aegis-Core/commit/796a6640028ef4744ecc5539fcfb3528ac564bae))

## [0.1.49](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.48...ank-server-v0.1.49) (2026-05-05)


### Features

* **ank-core:** CORE-261 bucle ReAct en CognitiveHAL + limpieza StreamInterceptor ([#211](https://github.com/Gustavo324234/Aegis-Core/issues/211)) ([7deb854](https://github.com/Gustavo324234/Aegis-Core/commit/7deb8546fadc8a9df5313d808dbf61309c722aaa))
* **ank-core:** CORE-262 AgentOrchestrator — inferencia LLM real en run_agent_loop ([#212](https://github.com/Gustavo324234/Aegis-Core/issues/212)) ([7a672d8](https://github.com/Gustavo324234/Aegis-Core/commit/7a672d8f40d6094ea16e4d03e04e2c681cbb2203))

## [0.1.48](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.47...ank-server-v0.1.48) (2026-05-03)

## [0.1.47](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.46...ank-server-v0.1.47) (2026-05-03)


### Bug Fixes

* **ank-server:** CORE-244 HAL Runner — StatusUpdate en path de error ([#191](https://github.com/Gustavo324234/Aegis-Core/issues/191)) ([f59b152](https://github.com/Gustavo324234/Aegis-Core/commit/f59b152bf0fa737b86128dd39d40834f4b46cd98))

## [0.1.46](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.45...ank-server-v0.1.46) (2026-05-03)

## [0.1.45](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.44...ank-server-v0.1.45) (2026-05-02)

## [0.1.44](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.43...ank-server-v0.1.44) (2026-05-02)


### Bug Fixes

* **ank-server:** CORE-241 send user-facing response after AgentToolCall execution ([#185](https://github.com/Gustavo324234/Aegis-Core/issues/185)) ([e6e4c5e](https://github.com/Gustavo324234/Aegis-Core/commit/e6e4c5efa27fcf7b681105eae583a6205120cf09))

## [0.1.43](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.42...ank-server-v0.1.43) (2026-05-02)

## [0.1.42](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.41...ank-server-v0.1.42) (2026-05-02)


### Bug Fixes

* **ank-core:** CORE-241 filter __TOOL_CALL__ output before frontend delivery fix(ank-core): CORE-242 exclude MAKER_INSTRUCTIONS from Chat Agent prompt ([#180](https://github.com/Gustavo324234/Aegis-Core/issues/180)) ([0ca5df3](https://github.com/Gustavo324234/Aegis-Core/commit/0ca5df3417cb69c49d009a79607d59daa47699d0))

## [0.1.41](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.40...ank-server-v0.1.41) (2026-05-01)

## [0.1.40](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.39...ank-server-v0.1.40) (2026-05-01)

## [0.1.39](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.38...ank-server-v0.1.39) (2026-04-30)

## [0.1.38](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.37...ank-server-v0.1.38) (2026-04-30)

## [0.1.37](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.36...ank-server-v0.1.37) (2026-04-29)

## [0.1.36](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.35...ank-server-v0.1.36) (2026-04-29)


### Bug Fixes

* **install:** agents config deploy ([#165](https://github.com/Gustavo324234/Aegis-Core/issues/165)) ([#166](https://github.com/Gustavo324234/Aegis-Core/issues/166)) ([905ca1b](https://github.com/Gustavo324234/Aegis-Core/commit/905ca1bb787f59a141114b538bda5daa014874dd))

## [0.1.35](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.34...ank-server-v0.1.35) (2026-04-29)

## [0.1.34](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.33...ank-server-v0.1.34) (2026-04-29)

## [0.1.33](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.32...ank-server-v0.1.33) (2026-04-29)


### Bug Fixes

* **ank-server:** CORE-228 conectar AgentOrchestrator al SyscallExecutor ([#158](https://github.com/Gustavo324234/Aegis-Core/issues/158)) ([6376e7e](https://github.com/Gustavo324234/Aegis-Core/commit/6376e7e98fe948e286ac87100594c2d18348be2b))

## [0.1.32](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.31...ank-server-v0.1.32) (2026-04-29)

## [0.1.31](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.30...ank-server-v0.1.31) (2026-04-28)

## [0.1.30](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.29...ank-server-v0.1.30) (2026-04-27)


### Features

* **router:** free-tier rate limiting, Gemini 3.x catalog and JS sandbox fixes ([#138](https://github.com/Gustavo324234/Aegis-Core/issues/138)) ([392cc69](https://github.com/Gustavo324234/Aegis-Core/commit/392cc697d76c7b4de75f473a9b3ea9fc94a178cd))
* **shell:** dashboard tree view ([#140](https://github.com/Gustavo324234/Aegis-Core/issues/140)) ([9788d1f](https://github.com/Gustavo324234/Aegis-Core/commit/9788d1fea31e695ac9cef9888c3e1e92171ef744))

## [0.1.29](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.28...ank-server-v0.1.29) (2026-04-27)

## [0.1.28](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.27...ank-server-v0.1.28) (2026-04-26)

## [0.1.27](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.26...ank-server-v0.1.27) (2026-04-25)

## [0.1.26](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.25...ank-server-v0.1.26) (2026-04-25)

## [0.1.25](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.24...ank-server-v0.1.25) (2026-04-25)

## [0.1.24](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.23...ank-server-v0.1.24) (2026-04-25)


### Features

* **epic-44:** Developer Workspace — terminal, code viewer, git bridge, PR manager ([#117](https://github.com/Gustavo324234/Aegis-Core/issues/117)) ([9ca9a10](https://github.com/Gustavo324234/Aegis-Core/commit/9ca9a10f3e4b03812f9c19caf31fb52d27f5e884))

## [0.1.23](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.22...ank-server-v0.1.23) (2026-04-24)


### Features

* **agents:** Epic 43 — Hierarchical Multi-Agent Orchestration ([#115](https://github.com/Gustavo324234/Aegis-Core/issues/115)) ([6b640a7](https://github.com/Gustavo324234/Aegis-Core/commit/6b640a7f9ab53a7aa5f8111f00e8f6d8db8e9f59))

## [0.1.22](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.21...ank-server-v0.1.22) (2026-04-24)


### Bug Fixes

* **ws:** stream tokens to WebSocket event_broker during inference ([#113](https://github.com/Gustavo324234/Aegis-Core/issues/113)) ([beb0285](https://github.com/Gustavo324234/Aegis-Core/commit/beb0285e6b88a4b3c2c3b0c2340e6ced62ce74db))

## [0.1.21](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.20...ank-server-v0.1.21) (2026-04-24)


### Features

* **core-154:** implement multi-agent supervisor/worker orchestration ([#110](https://github.com/Gustavo324234/Aegis-Core/issues/110)) ([9abd10f](https://github.com/Gustavo324234/Aegis-Core/commit/9abd10f9154bd88247434e3eb2f626b834d60638))

## [0.1.20](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.19...ank-server-v0.1.20) (2026-04-23)

## [0.1.19](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.18...ank-server-v0.1.19) (2026-04-23)


### Bug Fixes

* **installer,ank-server:** CORE-147 improve cloudflared installation path and error logging ([#98](https://github.com/Gustavo324234/Aegis-Core/issues/98)) ([7a2dfaf](https://github.com/Gustavo324234/Aegis-Core/commit/7a2dfafbc702a4d03ee78bea19964586ca5c61d3))

## [0.1.18](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.17...ank-server-v0.1.18) (2026-04-22)


### Bug Fixes

* **ank-server:** CORE-147 enforce internal HTTP and fix cloudflared path ([#95](https://github.com/Gustavo324234/Aegis-Core/issues/95)) ([08dd48e](https://github.com/Gustavo324234/Aegis-Core/commit/08dd48e29a3d7a2e3d7b3c0f638cf95eeea45f44))

## [0.1.17](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.16...ank-server-v0.1.17) (2026-04-22)


### Features

* **ank-server,installer:** CORE-146 Cloudflare tunnel + connection-i… ([#90](https://github.com/Gustavo324234/Aegis-Core/issues/90)) ([e8be602](https://github.com/Gustavo324234/Aegis-Core/commit/e8be60263bc0d4d8a4ab3fc9175badfb0982887c))

## [0.1.16](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.15...ank-server-v0.1.16) (2026-04-22)


### Bug Fixes

* **installer:** CORE-147 aegis update regenerates TLS cert + tls-rege… ([#84](https://github.com/Gustavo324234/Aegis-Core/issues/84)) ([3144e3c](https://github.com/Gustavo324234/Aegis-Core/commit/3144e3c8d328f042b8d9bf56bd018591fa389705))

## [0.1.15](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.14...ank-server-v0.1.15) (2026-04-22)


### Bug Fixes

* **core,http,installer:** music prompt always injected + TLS vars on … ([#81](https://github.com/Gustavo324234/Aegis-Core/issues/81)) ([141bc16](https://github.com/Gustavo324234/Aegis-Core/commit/141bc16c1303655eeb999a62305c0ffc82026ee2))

## [0.1.14](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.13...ank-server-v0.1.14) (2026-04-21)


### Features

* **core:** Epic 38-39-40 — Agent Persona, Music, Connected Accounts ([#76](https://github.com/Gustavo324234/Aegis-Core/issues/76)) ([b4ceb7d](https://github.com/Gustavo324234/Aegis-Core/commit/b4ceb7d77884109570e07fbf0577d88a113c4842))

## [0.1.13](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.12...ank-server-v0.1.13) (2026-04-21)

## [0.1.12](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.11...ank-server-v0.1.12) (2026-04-20)

## [0.1.11](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.10...ank-server-v0.1.11) (2026-04-20)


### Bug Fixes

* **ank-core,installer:** CORE-121 CORE-122 ([#56](https://github.com/Gustavo324234/Aegis-Core/issues/56)) ([35dea95](https://github.com/Gustavo324234/Aegis-Core/commit/35dea954612095ccc3442033f72228853e0e8b41))

## [0.1.10](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.9...ank-server-v0.1.10) (2026-04-20)


### Bug Fixes

* **ank-core:** CORE-092 fix silent cloud errors and implement provide… ([#52](https://github.com/Gustavo324234/Aegis-Core/issues/52)) ([a236021](https://github.com/Gustavo324234/Aegis-Core/commit/a2360213a99cd0ff582ab58d1c632b80a4754fd6))

## [0.1.9](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.8...ank-server-v0.1.9) (2026-04-20)

## [0.1.8](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.7...ank-server-v0.1.8) (2026-04-20)


### Bug Fixes

* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([46e0c6d](https://github.com/Gustavo324234/Aegis-Core/commit/46e0c6d1cae15666c1fcea9c283cc41a4cad2e11))
* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([2a5c5ea](https://github.com/Gustavo324234/Aegis-Core/commit/2a5c5ead0c92cf31dc22155037441f622f0e52e9))
* **ci:** trigger nightly build ([#44](https://github.com/Gustavo324234/Aegis-Core/issues/44)) ([2a5c5ea](https://github.com/Gustavo324234/Aegis-Core/commit/2a5c5ead0c92cf31dc22155037441f622f0e52e9))
* **ci:** trigger nightly build ([#45](https://github.com/Gustavo324234/Aegis-Core/issues/45)) ([46e0c6d](https://github.com/Gustavo324234/Aegis-Core/commit/46e0c6d1cae15666c1fcea9c283cc41a4cad2e11))

## [0.1.7](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.6...ank-server-v0.1.7) (2026-04-17)

## [0.1.6](https://github.com/Gustavo324234/Aegis-Core/compare/ank-server-v0.1.5...ank-server-v0.1.6) (2026-04-17)


### Bug Fixes

* **ank-http:** CORE-107 correct clippy errors - utoipa schema and type mismatches ([75e990b](https://github.com/Gustavo324234/Aegis-Core/commit/75e990b54c255f676d2853c1e0ffa530e91ea298))

# Aegis OS — Dogfooding & Real-World Operations Notes

> **Version:** 1.0.0
> **Target:** Transparency, SRE Resilience, and Real-World Usage Experience

---

## 1. Overview & Setup

Aegis OS is not designed solely as a theoretical framework; it is actively deployed and executed daily by its core maintainers as the primary personal cognitive operating system ("CIO of personal life").

This document provides transparent, unvarnished insight into our production dogfooding environment, hardware profiles, operational workloads, encountered failures, and architectural remediations.

---

## 2. Infrastructure & Deployment Topology

Our primary dogfooding environment consists of two dedicated nodes:

### Primary Workstation (Windows 11 Pro / x86_64)
* **CPU:** AMD Ryzen 9 7950X (16 Cores / 32 Threads)
* **RAM:** 64 GB DDR5-6000
* **Storage:** PCIe Gen4 NVMe (SQLCipher Encrypted Enclave)
* **Runtime:** `ank-server` running as a background Windows Service (`AegisOS`) managed via SCM and the `aegis` CLI.
* **Inference Profile:** Hybrid (Local Ollama for syntactical/privacy tasks + OpenRouter/Anthropic/Google Cloud for heavy reasoning).

### Secondary Home Server (Ubuntu 24.04 LTS / Linux x86_64)
* **CPU:** Intel Core i7-13700K
* **RAM:** 32 GB DDR4
* **Runtime:** Native Systemd Daemon (`aegis.service`) with `aegis-connect-relay` active for persistent WebSocket tunneling to Orion ID.

---

## 3. Daily Workload & Real-World Metrics

Over months of daily operations, Aegis OS processes an average of:

* **Daily User Interactions:** 45 - 120 agent thread exchanges per day across developer workspace tasks, project ledgers, and context queries.
* **Multi-Agent Task Trees:** ~15 autonomous sub-agent tree spawns daily (`AgentOrchestrator`) handling code refactoring, Git commits, and documentation updates.
* **Ledger & Memory Growth:** ~2,500 local SQLCipher database transactions per week tracking task completions, financial entries, and project states.
* **Routing Efficiency:** **78.4%** of routine queries (formatting, status checks, L1 memory lookups) are handled by local Ollama models (`qwen2.5-coder` / `llama3.1-8b`) or low-cost Flash models, reducing external API costs by over 80%.

---

## 4. Real-World Failures & SRE Hardening

Dogfooding in real environments exposed critical edge cases that directly shaped the architecture of Aegis Core:

### Incident 1: Infinite Synthesis Loops (`CORE-288`)
* **Problem:** During early multi-agent supervisor testing, a sub-agent reporting back to its parent supervisor re-triggered a synthesis event, causing a cyclical response cascade that consumed rate limits.
* **Fix:** Implemented single-report idempotency in `AgentOrchestrator` and boundary limits on synthesis events (`synthesis_done` auto-closes the agent communication channel).

### Incident 2: Database Lockups on Keep-Alive (`CORE-189`)
* **Problem:** A 126-second session ping interval was reopening the SQLCipher database handle repeatedly, triggering high disk I/O and occasional lock contention on Windows.
* **Fix:** Introduced sticky session context caching (`SessionHistoryCache`) and connection pooling for the Citadel Enclave.

### Incident 3: Non-Deterministic Model Tool Calls (`CORE-303`)
* **Problem:** Certain cloud models returned slightly malformed JSON block wrappers during tool execution, causing standard parsers to fail.
* **Fix:** Developed **Defensive Cognitive Loops & Boundary Autocorrection** (`CORE-303`), enabling `ank-core` to self-correct tool execution payloads before throwing runtime errors.

---

## 5. Conclusion

Running Aegis OS on a daily basis has validated our core philosophy: **LLMs are ALUs, not oracles.** Stability comes from deterministic kernel scheduling, Zero-Panic error boundaries in Rust, and local-first data sovereignty.

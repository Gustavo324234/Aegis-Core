# Aegis OS — PinchBench Evaluation, Cognitive Efficiency & 2026 Model Matrix

> **Version:** 2.0.0
> **Target:** CMR v4 Adaptive Complexity, Cache-First Context & Empirical 2026 Model Matrix (`EPIC 62`)

---

## 1. Executive Summary

Aegis OS features **CMR v4 (Cognitive Model Router v4)** paired with a **Cache-First + Dynamic Tool Gating** context builder. Rather than relying on static vendor claims or flooding every chat turn with unused schemas, Aegis routes turns dynamically according to `ChatComplexity` (`Quick` / `Standard` / `DeepReasoning`) and empirical **PinchBench** tool-use scores blended continuously with task weights (`0.70 * discrete_score + 0.30 * benchmark_pct`).

---

## 2. Empirical 2026 Benchmark Matrix (`models.yaml`)

Verified against live OpenRouter pricing and **PinchBench** multi-turn agent evaluations:

### Paid Frontier & Ultra-Value Stack

| Model ID | PinchBench | Input / Output (1M) | Cache Read (1M) | Context | Optimal Role (`CMR v4`) |
|---|---:|---:|---:|---:|---|
| `anthropic/claude-opus-4.6` | **96.30%** | $5.00 / $25.00 | $0.50 | 1.0M | Critical Architecture / Complex Refactoring |
| `anthropic/claude-opus-4.8` | **94.49%** | $5.00 / $25.00 | $0.50 | 1.0M | Frontier Reasoning / Hard Planning |
| `qwen/qwen3.7-max` | **93.44%** | $1.475 / $4.425 | — | 262k | High-Accuracy Planning & Tool Orchestration |
| `deepseek/deepseek-v4-flash` | **91.46%** | $0.09 / $0.18 | $0.018 | 1.0M | **#1 Cost-Efficiency Champion** (`Standard` & `DeepReasoning`) |
| `openai/gpt-5.6-luna` | **90.79%** | $0.20 / $1.20 | $0.02 | 1.05M | High-Reliability Assistant & Coding |
| `qwen/qwen3.6-flash` | **89.14%** | $0.188 / $1.125 | — | 1.0M | Long-Context Analysis & Structured Extraction |
| `qwen/qwen3.7-flash` | **86.50%** | $0.03 / $0.13 | — | 262k | High-Volume Background Workers |

### 100% Free Multi-Provider Stack (`tools` Verified)

| Model ID | Provider | PinchBench | Context | Optimal Role (`CMR v4`) |
|---|---|---:|---:|---|
| `nvidia/nemotron-3-ultra-550b-a55b:free` | OpenRouter (`:free`) | **90.58%** | 1.0M | Free `DeepReasoning` & Complex Planning |
| `gemini-3.8-flash` | Google AI Studio (Free Tier) | **89.20%** | 1.0M | Default Free Primary Assistant |
| `qwen/qwen3.8-27b` | Groq (Free Tier) | **86.40%** | 131k | Ultra-Fast Free Reasoning & Tool Execution |
| `openai/gpt-oss-120b` | Groq (Free Tier) | **84.50%** | 131k | Fast Free Failover & General Chat |
| `google/gemma-4-31b-it:free` | OpenRouter (`:free`) | **84.10%** | 131k | OpenRouter Zero-Cost Tool Calling |
| `gemini-3.5-flash-lite` | Google AI Studio (Free Tier) | **82.40%** | 1.0M | Ultra-Low-Latency `Quick` Turns |

---

## 3. How CMR v4 + Cache-First Context Work Together

1. **Dynamic Tool Gating (`CORE-408`):** `chal/context_builder.rs` (`select_chat_tool_names`) inspects the active turn + short follow-up window to bind only domain-relevant tool JSON schemas (`0` schemas on pure conceptual turns, typically `1–3` on expenses/calendar/WhatsApp), shrinking prompt input by **60%–75%** and freeing tight free-tier TPM budgets (e.g., Groq 8k TPM).
2. **Cache-First System Prefix (`CORE-408`):** `messages[0]` (`System`) stays byte-identical across turns and minutes; volatile timestamps and selective personal memory (`build_contextual_personal_memory_block`) are injected right before the final `User` turn so Anthropic, OpenAI, DeepSeek, and Gemini KV-caches hit reliably.
3. **Adaptive `ChatComplexity` (`CORE-409`):** Classifies turns into `Quick`, `Standard`, or `DeepReasoning` (`pcb.rs`), isolates sticky cache keys per complexity tier, removes the legacy `TaskType::Chat` quality penalty on deep questions, and scales `max_tokens` (up to `4096`) for reasoning models without truncating visible answers.
4. **Continuous PinchBench Blending:** Effective quality merges task suitability (`0.70`) with normalized `benchmark_pct` (`0.30`), so a **91.46%** model decisively outranks an **80%** model even when both sit in the same discrete tier.

---

## 4. How to Reproduce & Sync Benchmarks Locally

### Prerequisites
* Python 3.10+ (`pip install -r tools/requirements.txt`)

### Step 1: Dry-Run Inspection
```bash
python tools/update_models.py --dry-run
```

### Step 2: Sync Prices & PinchBench Scores (`benchmark_pct`)
```bash
python tools/update_models.py --only-scores
```

### Step 3: Validate Kernel Compilation & Routing Tests
```bash
cargo check --workspace
cargo test -p ank-core -- router::tests
```

---

## 5. References & Tooling
* Benchmark Synchronizer Script: [tools/update_models.py](../tools/update_models.py)
* Model Catalog Configuration: [kernel/crates/ank-core/src/router/models.yaml](../kernel/crates/ank-core/src/router/models.yaml)
* Adaptive Scoring & Context Builder: [kernel/crates/ank-core/src/router/scoring.rs](../kernel/crates/ank-core/src/router/scoring.rs), [kernel/crates/ank-core/src/chal/context_builder.rs](../kernel/crates/ank-core/src/chal/context_builder.rs)

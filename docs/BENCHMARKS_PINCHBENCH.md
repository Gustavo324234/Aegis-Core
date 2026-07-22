# Aegis OS — PinchBench Evaluation & Reproducibility Guide

> **Version:** 1.0.0
> **Target:** Empirically Reproducible Benchmarks and Cognitive Model Routing (CMR v3)

---

## 1. Executive Summary

Aegis OS features an **Asymmetric Cognitive Model Router (CMR v3)** that routes incoming agent tasks to optimal language models based on real-time empirical benchmarks. 

Rather than relying on static vendor claims, Aegis uses evaluation data from **PinchBench** — an industry-standard benchmark suite measuring multi-turn tool-use success rates, latency per token, and token processing costs.

---

## 2. Empirical Benchmark Matrix

The table below reflects the task-aware scoring matrix integrated into `kernel/crates/ank-core/src/router/models.yaml`:

| Model ID | Success Rate | Time to First Token (TTFT) | Avg Latency | Cost / 1M Input | Cost / 1M Output | Optimal Task Profile (`TaskType`) |
|---|---|---|---|---|---|---|
| `anthropic/claude-opus-4.8-fast` | **94.49%** | ~0.8s | ~159s | $15.00 | $75.00 | `L2 Reasoning` / Complex Refactoring / DAG Compile |
| `google/gemini-3.1-flash-lite` | **80.50%** | ~0.2s | ~15s | $0.075 | $0.30 | `L1 Syntactic` / UI Layout Checks / Fast Lookups |
| `openai/gpt-5.4-nano` | **77.26%** | ~0.3s | ~12s | $0.05 | $0.20 | `L1 Syntactic` / Parameter Extraction / JSON Formatting |
| `ollama/qwen2.5-coder:32b` (Local) | **72.10%** | ~0.1s | ~45s | Free | Free | `Local Privacy` / High-Security File Parsing |
| `ollama/llama3.1-8b` (Local) | **47.44%** | ~0.1s | ~20s | Free | Free | Offline Fallback / Lightweight Summaries |

---

## 3. How CMR v3 Schedules Tasks

When `ank-core` receives an agent instruction, the scheduler evaluates:

$$\text{Score}(m, t) = w_s \cdot S(m) - w_l \cdot L(m) - w_c \cdot C(m) + H(m, t)$$

Where:
* $S(m)$ is the model's PinchBench success rate.
* $L(m)$ is the EWMA (Exponentially Weighted Moving Average) of recent latency.
* $C(m)$ is the cost factor per million tokens for task $t$.
* $H(m, t)$ is the half-open circuit breaker penalty if rate limits (HTTP 429) were detected.

This dynamic scoring cuts average task costs by up to **80%** while preserving high accuracy on critical code generation tasks.

---

## 4. How to Reproduce Benchmarks Locally

Any developer can verify or update the benchmark metrics using the included synchronization tool in `tools/`:

### Prerequisites
* Python 3.10+
* Installed requirements: `pip install -r tools/requirements.txt`

### Step 1: Run Dry-Run Inspection
To inspect current prices from OpenRouter and scores from PinchBench without altering local configuration:

```bash
python tools/update_models.py --dry-run
```

### Step 2: Update Task Scores Only
To pull the latest PinchBench tool-use evaluations into `kernel/crates/ank-core/src/router/models.yaml`:

```bash
python tools/update_models.py --only-scores
```

### Step 3: Validate Kernel Compilation
Verify that the updated YAML catalog compiles cleanly into the Rust binary:

```bash
cargo check --workspace
```

---

## 5. References & Tooling
* Benchmark Synchronizer Script: [tools/update_models.py](file:///e:/Aegis/Aegis-Core/tools/update_models.py)
* Model Catalog Configuration: `kernel/crates/ank-core/src/router/models.yaml`
* Router Test Suite: `kernel/crates/ank-core/src/router/tests.rs`

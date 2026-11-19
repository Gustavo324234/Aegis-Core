# 🧪 Aegis Agent Sandbox — Personal Assistant Simulator

The **Aegis Agent Sandbox** (`tools/agent_sandbox/`) is a zero-dependency Python simulator and benchmark harness designed to test, iterate, and perfect **Aegis Personal Assistant** intelligence in seconds—without needing to recompile the entire Rust (`ank-core`) and React (`shell/ui`) stack on every prompt or tool change.

---

## ✨ What It Simulates

1. **Single Infinite Chat (`Chat Conductor`):** One natural conversation thread that remembers context across sessions.
2. **Short-Term & Long-Term Memory (`PersonalMemoryStore`):**
   - **Short-Term:** Sliding window of recent conversation turns persisted in `workspace/aegis_memory.json`.
   - **Long-Term (`facts`):** Personal preferences, routines, health/dietary restrictions, and project facts injected dynamically into every turn.
   - **Implicit Consolidation (`Chronos`):** Background analysis of recent conversations to extract facts the user mentioned in passing without explicitly saying *"remember this"*.
3. **Semantic Personal Ledger & Reminders:**
   - Instant tools (`log_expense_or_entry`, `query_ledger`, `remember_fact`, `manage_reminder`).
   - Natural semantic inference: saying *"Agregá $10.000 de súper"* or *"Sumá $4.500 de carnicería"* automatically normalizes both into the **`comida`** category and reports the running category total.
4. **Non-Blocking Background Workers (`spawn_agent`):**
   - Heavy tasks (coding scripts, reading/writing files, executing commands, web search) run on background threads (`threading.Thread`) in interactive mode so you can keep chatting while the worker executes and notifies you with `🔔 [SEGUNDO PLANO COMPLETADO]`.
5. **Multi-Key Rotation & Provider Fallback:**
   - Automatically loads all `OPENROUTER_API_KEY*` and `GEMINI_API_KEY*` keys from the root `.env` (or uses local `ollama`) and rotates seamlessly on HTTP 429/503 rate limits.

---

## 🚀 Quick Start (30 Seconds)

From the root of the repository:

```bash
# 1. Interactive Personal Assistant Chat Mode
python tools/agent_sandbox/sandbox.py

# 2. Run the Automated 5-Scenario Benchmark Suite
python tools/agent_sandbox/sandbox.py --test
```

On Windows, you can also double-click **`run_sandbox.bat`** in the root directory, which launches both the isolated test web server (`http://localhost:8090`, using `--server-only` to skip the CLI) and the interactive CLI simulator.

### Provider Options

```bash
# Default: OpenRouter (with automatic fallback to Gemini if configured in .env)
python tools/agent_sandbox/sandbox.py --provider openrouter

# Google Gemini Direct API
python tools/agent_sandbox/sandbox.py --provider gemini

# 100% Free Local Inference via Ollama
python tools/agent_sandbox/sandbox.py --provider ollama --model qwen2.5:7b
```

---

## 💬 Interactive Commands

Inside the interactive chat, you can talk naturally or use these slash commands to inspect the assistant's internal state:

| Command | Description |
| :--- | :--- |
| `/memoria` | Inspect all long-term facts, reminders, and `Chronos` consolidations |
| `/gastos` | View your personal ledger table and category totals (`comida`, `transporte`, etc.) |
| `/chronos` | Trigger `Chronos` consolidation to extract implicit facts from recent dialogue |
| `/jobs` | Check the live status of background workers spawned by Aegis |
| `/workspace` | List files created by background workers in `tools/agent_sandbox/workspace/` |
| `/view <file>` | Print the contents of a file inside the sandbox workspace |
| `/clear` | Clear short-term chat history while preserving long-term facts and expenses |
| `/verbose` | Toggle detailed JSON tool-call inspection on/off |
| `/log` | Show the path to the full `.jsonl` execution trace of the current session |
| `/exit` | Exit the simulator |

---

## 📂 Directory Structure

```text
tools/agent_sandbox/
├── sandbox.py                     # Main simulator, PersonalMemoryStore, LLM router & CLI
├── scenarios.json                 # Automated Personal Assistant benchmark suite (5 core scenarios)
├── prompts/
│   ├── v1_hierarchical/           # Legacy 3-tier hierarchy prompts (for A/B comparison)
│   └── v2_direct/                 # Active Personal Assistant prompts:
│       ├── chat_conductor.md      # Main personal assistant prompt & semantic rules
│       └── worker_specialist.md   # Background execution worker prompt
├── workspace/                     # Isolated sandbox filesystem + aegis_memory.json (gitignored)
└── logs/                          # Detailed .jsonl session traces (gitignored)
```

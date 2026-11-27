use crate::pcb::{ChatComplexity, RoutingPolicy, TaskType};
use crate::router::{catalog::ModelEntry, rate_tracker::ModelOutcomes};

/// Context passed into the CMR v4 candidate scorer.
pub(crate) struct ScoreCtx<'a> {
    pub prompt: &'a str,
    pub max_cost: f64,
    pub max_latency: f64,
    pub observed_latency: Option<u32>,
    pub recent_errors: u32,
    pub estimated_tokens: usize,
    pub routing_policy: RoutingPolicy,
    pub chat_complexity: ChatComplexity,
    /// CORE-320: merged success/failure outcomes (catalog + API id spaces).
    pub outcomes: ModelOutcomes,
}

/// Computes the multi-factor routing score for a candidate model (CMR v4).
pub(crate) fn compute_model_score(
    entry: &ModelEntry,
    task_type: TaskType,
    ctx: &ScoreCtx<'_>,
) -> f64 {
    let (prompt, max_cost, max_latency, observed_latency, recent_errors) = (
        ctx.prompt,
        ctx.max_cost,
        ctx.max_latency,
        ctx.observed_latency,
        ctx.recent_errors,
    );

    // ── 1. Quality (continuous benchmark + task score) ──────────────
    let base_quality = entry.normalized_quality_for(task_type, ctx.chat_complexity);
    let content_boost = detect_content_type(prompt, task_type);
    let quality = (base_quality * (1.0 + content_boost)).min(1.0);

    // ── 2. Cost (task-weighted) ─────────────────────────────────────
    let total_cost = task_weighted_cost(entry, task_type);
    let cost_inv = if max_cost > 0.0 {
        (1.0 - (total_cost / max_cost)).clamp(0.0, 1.0)
    } else {
        1.0
    };

    // ── 3. Speed ────────────────────────────────────────────────────
    let effective_latency = observed_latency.unwrap_or(entry.avg_latency_ms.unwrap_or(2000)) as f64;
    let speed_inv = if max_latency > 0.0 {
        1.0 - (effective_latency / max_latency).min(1.0)
    } else {
        1.0
    };

    // ── 4. Context fit ──────────────────────────────────────────────
    let estimated_tokens = ctx.estimated_tokens;
    let context_fit = if entry.context_window as usize > estimated_tokens * 4 {
        1.0
    } else if entry.context_window as usize > estimated_tokens * 2 {
        0.7
    } else if entry.context_window as usize > estimated_tokens {
        0.3
    } else {
        0.0
    };

    // ── 5. Error penalty ────────────────────────────────────────────
    let error_penalty = (recent_errors as f64 * 0.10).min(0.30);

    // ── 6. Oversize penalty only on quick/trivial chat prompts ──────
    let oversize_penalty = {
        let lower_id = entry.model_id.to_lowercase();
        let is_giant = ["671b", "405b", "236b", "180b", "120b", "70b"]
            .iter()
            .any(|tag| lower_id.contains(tag));
        let is_chat_trivial = matches!(task_type, TaskType::Chat)
            && matches!(ctx.chat_complexity, ChatComplexity::Quick)
            && prompt.len() < 400;
        if is_giant && is_chat_trivial {
            0.30
        } else {
            0.0
        }
    };

    // ── 7. Policy, Task-aware, and Complexity-aware weights ─────────
    let (w_quality, w_cost, w_speed, w_fit) = match ctx.routing_policy {
        RoutingPolicy::CostOptimized => (0.15, 0.60, 0.10, 0.15),
        RoutingPolicy::QualityOptimized => (0.70, 0.05, 0.10, 0.15),
        RoutingPolicy::LatencyOptimized => (0.15, 0.10, 0.60, 0.15),
        RoutingPolicy::Balanced => match task_type {
            TaskType::Chat => match ctx.chat_complexity {
                ChatComplexity::Quick => (0.25, 0.25, 0.40, 0.10),
                ChatComplexity::Standard => (0.40, 0.25, 0.25, 0.10),
                ChatComplexity::DeepReasoning => (0.65, 0.15, 0.10, 0.10),
            },
            TaskType::Code | TaskType::Planning => (0.55, 0.15, 0.15, 0.15),
            TaskType::Analysis => (0.55, 0.15, 0.15, 0.15),
            _ => (0.40, 0.25, 0.20, 0.15),
        },
    };

    let raw = quality * w_quality + cost_inv * w_cost + speed_inv * w_speed + context_fit * w_fit;

    // ── 8. Tiny Model Penalty (Asymmetric Offloading) ───────────────
    let tiny_penalty_factor = if entry.is_local {
        let lower_id = entry.model_id.to_lowercase();
        let is_tiny = ["1b", "2b", "3b"].iter().any(|tag| lower_id.contains(tag));
        let is_heavy_task = matches!(
            task_type,
            TaskType::Code | TaskType::Planning | TaskType::Analysis
        ) || (matches!(task_type, TaskType::Chat)
            && matches!(ctx.chat_complexity, ChatComplexity::DeepReasoning));
        let is_complex_prompt = prompt.chars().count() > 300;
        if is_tiny && (is_heavy_task || is_complex_prompt) {
            0.10
        } else {
            1.0
        }
    } else {
        1.0
    };

    // ── 9. Complexity-aware Chat vs Subagents tiering ───────────────
    let chat_subagent_factor = {
        let is_chat = matches!(task_type, TaskType::Chat);
        let is_backup = entry.is_local
            || entry.provider == "ollama_cloud"
            || (entry.cost_input_per_mtok == 0.0 && entry.model_id != "openrouter/auto")
            || entry.model_id.ends_with(":free");
        let is_chat_primary = is_backup
            || (entry.free_tier_eligible
                && entry.cost_input_per_mtok <= 0.5
                && !entry.model_id.contains("pro")
                && entry.model_id != "openrouter/auto");
        let effective_tools = entry.supports_tools
            && entry.tool_use_support != crate::router::catalog::ToolUseSupport::Degraded;

        if is_chat {
            if entry.model_id == "openrouter/auto" {
                0.05
            } else {
                match ctx.chat_complexity {
                    ChatComplexity::Quick => {
                        if is_chat_primary {
                            2.0
                        } else {
                            0.05
                        }
                    }
                    ChatComplexity::Standard => {
                        if is_chat_primary || entry.cost_input_per_mtok <= 0.60 {
                            1.6
                        } else if entry.cost_input_per_mtok <= 3.0 {
                            0.9
                        } else {
                            0.20
                        }
                    }
                    ChatComplexity::DeepReasoning => {
                        let high_intel = entry.benchmark_pct.is_some_and(|p| p >= 88.0)
                            || entry.task_scores.analysis >= 5
                            || entry.supports_reasoning;
                        if entry.is_local
                            && (entry.model_id.to_lowercase().contains("1b")
                                || entry.model_id.to_lowercase().contains("3b"))
                        {
                            0.15
                        } else if high_intel {
                            if entry.cost_input_per_mtok <= 3.5 {
                                1.6
                            } else {
                                1.25
                            }
                        } else if entry.task_scores.analysis <= 3 {
                            0.45
                        } else {
                            0.9
                        }
                    }
                }
            }
        } else if is_backup {
            if effective_tools {
                0.5
            } else {
                0.01
            }
        } else if entry.tool_use_support == crate::router::catalog::ToolUseSupport::Degraded {
            0.05
        } else {
            2.0
        }
    };

    // ── 10. Observed reliability (CORE-320) ─────────────────────────
    let reliability_factor = {
        let total = ctx.outcomes.successes + ctx.outcomes.failures;
        match ctx.outcomes.success_rate() {
            Some(rate) if total >= 3 => 0.5 + 0.5 * rate,
            _ => 1.0,
        }
    };

    (raw * (1.0 - error_penalty)
        * (1.0 - oversize_penalty)
        * tiny_penalty_factor
        * chat_subagent_factor
        * reliability_factor)
        .max(0.0)
}

/// Blended per-Mtok cost of a model for a given task type (CORE-324).
pub(crate) fn task_weighted_cost(entry: &ModelEntry, task: TaskType) -> f64 {
    let out_weight = match task {
        TaskType::Code | TaskType::Planning | TaskType::Creative => 3.0,
        _ => 1.0,
    };
    entry.cost_input_per_mtok.max(0.0) + entry.cost_output_per_mtok.max(0.0) * out_weight
}

/// Analyses the prompt with lexical signals and returns a boost (0.0–0.30)
/// when the detected content type matches the declared `task_type`.
pub(crate) fn detect_content_type(prompt: &str, task_type: TaskType) -> f64 {
    let lower = prompt.to_lowercase();

    let code_signals = [
        "```",
        "fn ",
        "def ",
        "function ",
        "import ",
        "class ",
        "let ",
        "const ",
        "var ",
        "=>",
        "{}",
    ];
    let code_score: f64 = code_signals.iter().filter(|s| lower.contains(*s)).count() as f64
        / code_signals.len() as f64;

    let analysis_signals = [
        "analiza",
        "analyze",
        "compare",
        "compara",
        "diferencia",
        "¿por qué",
        "why",
        "explica",
        "explain",
        "cuál es mejor",
    ];
    let analysis_score: f64 = analysis_signals
        .iter()
        .filter(|s| lower.contains(*s))
        .count() as f64
        / analysis_signals.len() as f64;

    let planning_signals = [
        "plan",
        "roadmap",
        "pasos",
        "steps",
        "cómo hacer",
        "how to",
        "estrategia",
        "strategy",
        "prioridad",
    ];
    let planning_score: f64 = planning_signals
        .iter()
        .filter(|s| lower.contains(*s))
        .count() as f64
        / planning_signals.len() as f64;

    let boost = match task_type {
        TaskType::Code => code_score,
        TaskType::Analysis => analysis_score,
        TaskType::Planning => planning_score,
        _ => 0.0,
    };

    (boost * 0.30_f64).min(0.30)
}

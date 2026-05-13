#!/usr/bin/env python3
"""
update_models.py — Aegis Model Catalog Synchronizer
====================================================
Actualiza kernel/crates/ank-core/src/router/models.yaml con:
  - Precios reales desde OpenRouter API (público, sin key)
  - Task scores derivados de PinchBench (JSON embebido en self.__next_f chunks)

Uso:
  python tools/update_models.py                # sincronización completa
  python tools/update_models.py --only-prices  # solo precios
  python tools/update_models.py --only-scores  # solo scores
  python tools/update_models.py --dry-run      # muestra diff sin escribir
  python tools/update_models.py --add-new      # agrega modelos nuevos encontrados

Dependencias:
  pip install requests pyyaml
"""

import argparse
import json
import math
import re
import sys
from copy import deepcopy
from pathlib import Path

import requests
import yaml

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------

SCRIPT_DIR  = Path(__file__).parent
REPO_ROOT   = SCRIPT_DIR.parent
MODELS_YAML = REPO_ROOT / "kernel" / "crates" / "ank-core" / "src" / "router" / "models.yaml"

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

OPENROUTER_MODELS_URL = "https://openrouter.ai/api/v1/models"
PINCHBENCH_SITE_URL   = "https://pinchbench.com/"

# Mapeo categorías PinchBench → TaskType Aegis
PINCHBENCH_CATEGORY_MAP = {
    "code":               "coding",
    "code devops":        "coding",
    "core agent":         "chat",
    "agent":              "chat",
    "data analysis":      "analysis",
    "data":               "analysis",
    "log analysis":       "analysis",
    "research":           "analysis",
    "research knowledge": "analysis",
    "file ops":           "extraction",
    "productivity":       "planning",
    "meeting analysis":   "summarization",
    "writing":            "summarization",
    "creative":           "chat",
    "security":           "analysis",
}

TASK_FIELDS = ["chat", "coding", "planning", "analysis", "summarization", "extraction"]


# ---------------------------------------------------------------------------
# Score conversion
# ---------------------------------------------------------------------------

def success_rate_to_score(rate: float) -> int:
    """Convierte success rate (0-100) a score Aegis (1-5)."""
    if rate <= 0:
        return 1
    return min(5, max(1, math.ceil(rate / 20.0)))


# ---------------------------------------------------------------------------
# Model ID normalization
# ---------------------------------------------------------------------------

def normalize_model_id(model_id: str) -> str:
    """
    "anthropic/claude-sonnet-4-6" → "claude-sonnet-4-6"
    "claude-sonnet-4-6"           → "claude-sonnet-4-6"
    """
    if "/" in model_id:
        return model_id.split("/", 1)[1].lower()
    return model_id.lower()


# ---------------------------------------------------------------------------
# OpenRouter — precios reales
# ---------------------------------------------------------------------------

def fetch_openrouter_prices(verbose: bool = True) -> dict:
    if verbose:
        print("📡  Fetching OpenRouter model list...")

    try:
        resp = requests.get(OPENROUTER_MODELS_URL, timeout=15)
        resp.raise_for_status()
    except requests.RequestException as e:
        print(f"⚠️  OpenRouter fetch failed: {e}", file=sys.stderr)
        return {}

    prices = {}
    for model in resp.json().get("data", []):
        mid     = model.get("id", "")
        pricing = model.get("pricing", {})
        try:
            input_mtok  = round(float(pricing.get("prompt",     0) or 0) * 1_000_000, 4)
            output_mtok = round(float(pricing.get("completion", 0) or 0) * 1_000_000, 4)
        except (ValueError, TypeError):
            continue

        if input_mtok == 0 and output_mtok == 0:
            continue

        entry = {"input": input_mtok, "output": output_mtok}
        prices[normalize_model_id(mid)] = entry
        prices[mid.lower()]             = entry

    if verbose:
        unique = len({k for k in prices if "/" not in k})
        print(f"✅  OpenRouter: {unique} models with pricing data")

    return prices


# ---------------------------------------------------------------------------
# PinchBench — JSON embebido en self.__next_f chunks
# ---------------------------------------------------------------------------

def fetch_pinchbench_scores(verbose: bool = True) -> dict:
    """
    PinchBench usa Next.js App Router con streaming SSR.
    Los datos del leaderboard están en uno de los self.__next_f chunks como JSON:

      {"model":"anthropic/claude-opus-4.7","percentage":91.57966938775512,...}

    El chunk tiene el formato:
      self.__next_f.push([1,"...escaped JSON string..."])

    Estrategia:
    1. Fetchear pinchbench.com
    2. Extraer todos los chunks de self.__next_f
    3. Unescape y buscar el que tiene "entries":[{"rank":...,"model":...,"percentage":...}]
    4. Parsear cada entry: model → percentage
    """
    if verbose:
        print("📡  Fetching PinchBench leaderboard...")

    headers = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
                      "AppleWebKit/537.36 (KHTML, like Gecko) "
                      "Chrome/124.0.0.0 Safari/537.36",
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "Accept-Language": "en-US,en;q=0.9",
    }

    try:
        resp = requests.get(PINCHBENCH_SITE_URL, headers=headers, timeout=20)
        resp.raise_for_status()
        html = resp.text
    except requests.RequestException as e:
        print(f"⚠️  PinchBench fetch failed: {e}", file=sys.stderr)
        return {}

    scores = {}

    # -------------------------------------------------------------------------
    # Estrategia 1: JSON en self.__next_f chunks (método probado y confirmado)
    # Los chunks están como: self.__next_f.push([1,"escaped_string"])
    # El string tiene el JSON con \\" en lugar de "
    # -------------------------------------------------------------------------
    chunks = re.findall(r'self\.__next_f\.push\(\[1,"(.*?)"\]\)', html, re.DOTALL)

    for chunk in chunks:
        if '"entries"' not in chunk and '"percentage"' not in chunk:
            continue

        # Unescape el string JSON (\\\" → \", \\\\ → \\, etc.)
        try:
            unescaped = chunk.encode('utf-8').decode('unicode_escape')
        except (UnicodeDecodeError, ValueError):
            # Fallback: unescape manual de los patrones más comunes
            unescaped = chunk.replace('\\"', '"').replace('\\\\', '\\').replace('\\n', '\n')

        # Buscar todos los objetos con "model" y "percentage"
        # El formato es: {"rank":N,"model":"org/name","provider":"...","percentage":XX.XX,...}
        entry_pattern = re.compile(
            r'\{"rank"\s*:\s*\d+\s*,\s*"model"\s*:\s*"([^"]+)"\s*,[^}]*"percentage"\s*:\s*([\d.]+)'
        )

        found = 0
        for m in entry_pattern.finditer(unescaped):
            model_id   = m.group(1)
            percentage = float(m.group(2))

            entry = {"overall": percentage, "by_category": {}}
            scores[normalize_model_id(model_id)] = entry
            scores[model_id.lower()]              = entry
            found += 1

        if found > 0:
            break  # chunk correcto encontrado, no seguir

    # -------------------------------------------------------------------------
    # Estrategia 2: regex directo sobre el HTML sin unescape
    # El HTML tiene literalmente: \"model\":\"anthropic/claude-opus-4.7\",
    # \"percentage\":91.57966938775512
    # -------------------------------------------------------------------------
    if not scores:
        direct_pattern = re.compile(
            r'\\"model\\":\\"([a-zA-Z0-9_\-]+/[a-zA-Z0-9_\-./]+)\\"'
            r'[^}]{0,200}?'
            r'\\"percentage\\":([\d.]+)'
        )
        for m in direct_pattern.finditer(html):
            model_id   = m.group(1)
            percentage = float(m.group(2))
            entry = {"overall": percentage, "by_category": {}}
            scores[normalize_model_id(model_id)] = entry
            scores[model_id.lower()]              = entry

    if verbose:
        if scores:
            unique = len({k for k in scores if "/" not in k})
            print(f"✅  PinchBench: {unique} models with scores")
        else:
            print("⚠️  PinchBench: no scores found — site structure may have changed.")
            print("    Scores won't be updated this run.")

    return scores


# ---------------------------------------------------------------------------
# Score computation
# ---------------------------------------------------------------------------

def compute_task_scores_from_pinchbench(bench_entry: dict) -> dict:
    overall     = bench_entry.get("overall", 0.0)
    by_category = bench_entry.get("by_category", {})

    base   = success_rate_to_score(overall)
    result = {f: base for f in TASK_FIELDS}

    for cat_name, cat_score in by_category.items():
        for pinch_cat, aegis_field in PINCHBENCH_CATEGORY_MAP.items():
            if pinch_cat in cat_name.lower() or cat_name.lower() in pinch_cat:
                result[aegis_field] = max(result[aegis_field], success_rate_to_score(cat_score))
                break

    return result


# ---------------------------------------------------------------------------
# Main update logic
# ---------------------------------------------------------------------------

def update_models(models, prices, bench_scores,
                  update_prices, update_scores, add_new, verbose) -> tuple:
    updated = deepcopy(models)
    changes = []

    for model in updated:
        mid      = model.get("model_id", "")
        norm     = normalize_model_id(mid)
        is_local = model.get("is_local", False)

        # --- Precios ---
        if update_prices and not is_local:
            pe = prices.get(norm) or prices.get(mid.lower())
            if pe:
                old_in  = model.get("cost_input_per_mtok",  0)
                old_out = model.get("cost_output_per_mtok", 0)
                if abs(old_in - pe["input"]) > 0.001 or abs(old_out - pe["output"]) > 0.001:
                    model["cost_input_per_mtok"]  = pe["input"]
                    model["cost_output_per_mtok"] = pe["output"]
                    changes.append(
                        f"  💰 {mid}: ${old_in}/{old_out} → ${pe['input']}/{pe['output']} /Mtok"
                    )

        # --- Scores ---
        if update_scores:
            be = bench_scores.get(norm) or bench_scores.get(mid.lower())
            if not be and "/" in mid:
                be = bench_scores.get(mid.split("/", 1)[1].lower())
            if be:
                new_scores = compute_task_scores_from_pinchbench(be)
                old_scores = model.get("task_scores", {})
                diffs = [
                    f"{f}: {old_scores.get(f, 0)}→{new_scores[f]}"
                    for f in TASK_FIELDS if old_scores.get(f, 0) != new_scores[f]
                ]
                if diffs:
                    model["task_scores"] = new_scores
                    changes.append(f"  📊 {mid}: {', '.join(diffs)}")

    return updated, changes


# ---------------------------------------------------------------------------
# YAML I/O
# ---------------------------------------------------------------------------

def load_yaml(path: Path) -> list:
    with open(path, "r", encoding="utf-8") as f:
        return yaml.safe_load(f) or []


def dump_yaml(models: list, path: Path) -> None:
    with open(path, "w", encoding="utf-8") as f:
        yaml.dump(models, f, default_flow_style=False, allow_unicode=True,
                  sort_keys=False, indent=2)


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Aegis Model Catalog Synchronizer")
    parser.add_argument("--only-prices", action="store_true",
                        help="Solo actualizar precios desde OpenRouter")
    parser.add_argument("--only-scores", action="store_true",
                        help="Solo actualizar task_scores desde PinchBench")
    parser.add_argument("--dry-run", action="store_true",
                        help="Muestra los cambios sin escribir el archivo")
    parser.add_argument("--add-new", action="store_true",
                        help="Agrega modelos nuevos encontrados (por defecto NO)")
    parser.add_argument("--quiet", action="store_true",
                        help="Suprime output informativo")
    parser.add_argument("--models-yaml", type=Path, default=MODELS_YAML,
                        help=f"Path al models.yaml (default: {MODELS_YAML})")
    args = parser.parse_args()

    verbose       = not args.quiet
    update_prices = not args.only_scores
    update_scores = not args.only_prices

    if not args.models_yaml.exists():
        print(f"❌  models.yaml not found at: {args.models_yaml}", file=sys.stderr)
        sys.exit(1)

    if verbose:
        print(f"📂  Loading: {args.models_yaml}")

    models = load_yaml(args.models_yaml)
    if verbose:
        print(f"    {len(models)} models loaded")

    prices       = fetch_openrouter_prices(verbose) if update_prices else {}
    bench_scores = fetch_pinchbench_scores(verbose) if update_scores else {}

    updated, changes = update_models(
        models, prices, bench_scores,
        update_prices, update_scores, args.add_new, verbose
    )

    if not changes:
        print("\n✅  No changes detected — catalog is up to date.")
        return

    print(f"\n📋  {len(changes)} change(s) found:\n")
    for c in changes:
        print(c)

    if args.dry_run:
        print("\n🔍  Dry-run mode — no files written.")
    else:
        dump_yaml(updated, args.models_yaml)
        print(f"\n💾  Written: {args.models_yaml}")
        print("    Run 'cargo build --workspace' to verify the YAML parses correctly.")


if __name__ == "__main__":
    main()

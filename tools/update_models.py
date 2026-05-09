#!/usr/bin/env python3
"""
update_models.py — Aegis Model Catalog Synchronizer
====================================================
Actualiza kernel/crates/ank-core/src/router/models.yaml con:
  - Precios reales desde OpenRouter API (público, sin key)
  - Task scores derivados de PinchBench success rates

Uso:
  python tools/update_models.py                # sincronización completa
  python tools/update_models.py --only-prices  # solo precios
  python tools/update_models.py --only-scores  # solo scores
  python tools/update_models.py --dry-run      # muestra diff sin escribir
  python tools/update_models.py --add-new      # agrega modelos nuevos encontrados

Dependencias:
  pip install requests pyyaml beautifulsoup4
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

SCRIPT_DIR = Path(__file__).parent
REPO_ROOT = SCRIPT_DIR.parent
MODELS_YAML = REPO_ROOT / "kernel" / "crates" / "ank-core" / "src" / "router" / "models.yaml"

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

OPENROUTER_MODELS_URL = "https://openrouter.ai/api/v1/models"
PINCHBENCH_URL = "https://pinchbench.com/"

# Mapeo de categorías PinchBench → TaskType de Aegis
PINCHBENCH_CATEGORY_MAP = {
    "Code":             "coding",
    "Code Devops":      "coding",
    "Core Agent":       "chat",
    "Agent":            "chat",
    "Data Analysis":    "analysis",
    "Log Analysis":     "analysis",
    "Research":         "analysis",
    "Research Knowledge": "analysis",
    "File Ops":         "extraction",
    "Productivity":     "planning",
    "Meeting Analysis": "summarization",
    "Writing":          "summarization",
    "Creative":         "chat",
    "Security":         "analysis",
}

# Todos los campos de task_scores que maneja Aegis
TASK_FIELDS = ["chat", "coding", "planning", "analysis", "summarization", "extraction"]

# Providers que usan OpenRouter como proxy (sus precios están en OpenRouter)
OPENROUTER_PROXIED_PROVIDERS = {"anthropic", "deepseek", "mistral", "qwen", "openrouter"}


# ---------------------------------------------------------------------------
# Score conversion
# ---------------------------------------------------------------------------

def success_rate_to_score(rate: float) -> int:
    """Convierte success rate (0-100) a score Aegis (1-5)."""
    if rate <= 0:
        return 1
    return min(5, max(1, math.ceil(rate / 20.0)))


# ---------------------------------------------------------------------------
# Model ID normalization for matching
# ---------------------------------------------------------------------------

def normalize_model_id(model_id: str) -> str:
    """
    Normaliza un model_id para comparación cross-source.
    Strips el prefijo provider/ si existe.
    Ej: "anthropic/claude-sonnet-4-6" → "claude-sonnet-4-6"
         "ollama/mistral-7b" → "mistral-7b"
    """
    if "/" in model_id:
        return model_id.split("/", 1)[1].lower()
    return model_id.lower()


def build_lookup(models: list[dict]) -> dict[str, dict]:
    """
    Construye un dict de lookup {normalized_id: entry} desde models.yaml.
    """
    lookup = {}
    for m in models:
        mid = m.get("model_id", "")
        norm = normalize_model_id(mid)
        lookup[norm] = m
        # También indexar por el model_id completo para match exacto
        lookup[mid.lower()] = m
    return lookup


# ---------------------------------------------------------------------------
# OpenRouter — precios reales
# ---------------------------------------------------------------------------

def fetch_openrouter_prices(verbose: bool = True) -> dict[str, dict]:
    """
    Fetchea todos los modelos de OpenRouter y retorna un dict:
    {normalized_model_id: {"input": float, "output": float, "display": str}}

    Los precios de OpenRouter vienen en $/token, los convertimos a $/Mtok.
    """
    if verbose:
        print("📡  Fetching OpenRouter model list...")

    try:
        resp = requests.get(OPENROUTER_MODELS_URL, timeout=15)
        resp.raise_for_status()
    except requests.RequestException as e:
        print(f"⚠️  OpenRouter fetch failed: {e}", file=sys.stderr)
        return {}

    data = resp.json()
    prices = {}

    for model in data.get("data", []):
        mid = model.get("id", "")
        pricing = model.get("pricing", {})
        try:
            input_per_tok = float(pricing.get("prompt", 0) or 0)
            output_per_tok = float(pricing.get("completion", 0) or 0)
        except (ValueError, TypeError):
            continue

        # Convertir de $/token a $/Mtok (millón de tokens)
        input_mtok = round(input_per_tok * 1_000_000, 4)
        output_mtok = round(output_per_tok * 1_000_000, 4)

        if input_mtok == 0 and output_mtok == 0:
            continue  # modelo sin precio conocido, ignorar

        norm = normalize_model_id(mid)
        prices[norm] = {
            "input": input_mtok,
            "output": output_mtok,
            "display": model.get("name", mid),
            "context_window": model.get("context_length"),
        }

    if verbose:
        print(f"✅  OpenRouter: {len(prices)} models with pricing data")

    return prices


# ---------------------------------------------------------------------------
# PinchBench — scores por modelo
# ---------------------------------------------------------------------------

def fetch_pinchbench_scores(verbose: bool = True) -> dict[str, dict]:
    """
    Scrapea PinchBench y retorna un dict:
    {model_id_string: {"overall": float, "by_category": {category: float}}}

    PinchBench embebe los datos del benchmark como JSON en el bundle JS.
    Estrategia: buscar el objeto __NEXT_DATA__ o el JSON de resultados embebido.
    """
    if verbose:
        print("📡  Fetching PinchBench data...")

    headers = {
        "User-Agent": "Mozilla/5.0 (compatible; AegisBenchBot/1.0; +https://github.com/Gustavo324234/Aegis-Core)"
    }

    try:
        resp = requests.get(PINCHBENCH_URL, headers=headers, timeout=20)
        resp.raise_for_status()
    except requests.RequestException as e:
        print(f"⚠️  PinchBench fetch failed: {e}", file=sys.stderr)
        return {}

    html = resp.text
    scores = {}

    # Estrategia 1: buscar __NEXT_DATA__ (Next.js)
    match = re.search(r'<script id="__NEXT_DATA__" type="application/json">(.*?)</script>', html, re.DOTALL)
    if match:
        try:
            next_data = json.loads(match.group(1))
            # Navegar la estructura de Next.js para encontrar los resultados
            props = next_data.get("props", {}).get("pageProps", {})
            results = props.get("results") or props.get("models") or props.get("benchmarks") or []
            if results:
                scores = _parse_pinchbench_results(results)
                if verbose and scores:
                    print(f"✅  PinchBench (__NEXT_DATA__): {len(scores)} models")
                    return scores
        except (json.JSONDecodeError, AttributeError):
            pass

    # Estrategia 2: buscar JSON inline con patrón de resultados
    # PinchBench suele embeber los datos como window.__DATA__ o similar
    patterns = [
        r'window\.__DATA__\s*=\s*(\{.*?\});',
        r'window\.__BENCHMARK_DATA__\s*=\s*(\[.*?\]);',
        r'"models"\s*:\s*(\[[\s\S]*?\])\s*[,}]',
    ]

    for pattern in patterns:
        match = re.search(pattern, html, re.DOTALL)
        if match:
            try:
                data = json.loads(match.group(1))
                if isinstance(data, list):
                    scores = _parse_pinchbench_results(data)
                elif isinstance(data, dict):
                    models_list = data.get("models") or data.get("results") or []
                    scores = _parse_pinchbench_results(models_list)
                if scores:
                    if verbose:
                        print(f"✅  PinchBench (inline JSON): {len(scores)} models")
                    return scores
            except (json.JSONDecodeError, AttributeError):
                continue

    # Estrategia 3: buscar chunks de webpack con datos de modelos
    chunk_matches = re.finditer(
        r'\{["\']model["\'][:\s]+["\']([^"\']+)["\'][,\s]+["\'](?:score|success_rate|overall)["\'][:\s]+(\d+\.?\d*)',
        html
    )
    chunk_scores = {}
    for m in chunk_matches:
        model_name = m.group(1)
        score = float(m.group(2))
        chunk_scores[normalize_model_id(model_name)] = {
            "overall": score,
            "by_category": {}
        }

    if chunk_scores:
        if verbose:
            print(f"✅  PinchBench (regex chunks): {len(chunk_scores)} models")
        return chunk_scores

    if verbose:
        print("⚠️  PinchBench: no structured data found. Scores won't be updated.")
        print("    → El sitio puede haber cambiado su estructura. Revisar manualmente.")

    return {}


def _parse_pinchbench_results(results: list) -> dict[str, dict]:
    """
    Parsea una lista de resultados de PinchBench al formato interno.
    Acepta múltiples esquemas de datos que PinchBench ha usado históricamente.
    """
    scores = {}

    for item in results:
        if not isinstance(item, dict):
            continue

        # Intentar extraer el nombre del modelo
        model_name = (
            item.get("model") or
            item.get("model_id") or
            item.get("name") or
            item.get("id") or
            ""
        )
        if not model_name:
            continue

        # Intentar extraer el score overall
        overall = (
            item.get("overall") or
            item.get("success_rate") or
            item.get("score") or
            item.get("best_score") or
            0.0
        )
        try:
            overall = float(overall)
        except (ValueError, TypeError):
            overall = 0.0

        # Intentar extraer scores por categoría
        by_category = {}
        categories = item.get("categories") or item.get("by_category") or item.get("tasks") or {}
        if isinstance(categories, dict):
            for cat, val in categories.items():
                try:
                    by_category[cat] = float(val)
                except (ValueError, TypeError):
                    pass
        elif isinstance(categories, list):
            for cat_item in categories:
                if isinstance(cat_item, dict):
                    cat_name = cat_item.get("name") or cat_item.get("category") or ""
                    cat_score = cat_item.get("score") or cat_item.get("success_rate") or 0
                    if cat_name:
                        try:
                            by_category[cat_name] = float(cat_score)
                        except (ValueError, TypeError):
                            pass

        norm = normalize_model_id(model_name)
        scores[norm] = {
            "overall": overall,
            "by_category": by_category,
            "raw_name": model_name,
        }

    return scores


# ---------------------------------------------------------------------------
# Score computation
# ---------------------------------------------------------------------------

def compute_task_scores_from_pinchbench(bench_entry: dict) -> dict[str, int]:
    """
    Deriva task_scores de Aegis desde un entry de PinchBench.

    Si hay scores por categoría, los mapea al campo correspondiente de Aegis.
    Si solo hay score overall, lo usa como base para todos los campos.
    """
    by_category = bench_entry.get("by_category", {})
    overall = bench_entry.get("overall", 0.0)

    # Inicializar con el score overall para todos los campos
    base_score = success_rate_to_score(overall)
    result = {field: base_score for field in TASK_FIELDS}

    # Override con scores específicos por categoría si están disponibles
    for cat_name, cat_score in by_category.items():
        # Buscar el campo de Aegis correspondiente (match parcial case-insensitive)
        cat_lower = cat_name.lower()
        for pinch_cat, aegis_field in PINCHBENCH_CATEGORY_MAP.items():
            if pinch_cat.lower() in cat_lower or cat_lower in pinch_cat.lower():
                result[aegis_field] = max(
                    result[aegis_field],
                    success_rate_to_score(cat_score)
                )
                break

    return result


# ---------------------------------------------------------------------------
# Main update logic
# ---------------------------------------------------------------------------

def update_models(
    models: list[dict],
    prices: dict[str, dict],
    bench_scores: dict[str, dict],
    update_prices: bool = True,
    update_scores: bool = True,
    add_new: bool = False,
    verbose: bool = True,
) -> tuple[list[dict], list[str]]:
    """
    Aplica actualizaciones a la lista de modelos.
    Retorna (updated_models, list_of_changes).
    """
    updated = deepcopy(models)
    changes = []

    for model in updated:
        mid = model.get("model_id", "")
        norm = normalize_model_id(mid)
        provider = model.get("provider", "")
        is_local = model.get("is_local", False)

        # --- Actualizar precios (solo modelos cloud) ---
        if update_prices and not is_local:
            # Buscar en OpenRouter por normalized ID
            price_entry = prices.get(norm) or prices.get(mid.lower())

            # Para modelos con prefijo de org (ej "anthropic/claude-..."), buscar también
            # por el ID completo como lo conoce OpenRouter
            if not price_entry and "/" in mid:
                # OpenRouter usa el formato "org/model" directamente
                price_entry = prices.get(mid.lower())

            if price_entry:
                old_in = model.get("cost_input_per_mtok", 0)
                old_out = model.get("cost_output_per_mtok", 0)
                new_in = price_entry["input"]
                new_out = price_entry["output"]

                if abs(old_in - new_in) > 0.001 or abs(old_out - new_out) > 0.001:
                    model["cost_input_per_mtok"] = new_in
                    model["cost_output_per_mtok"] = new_out
                    changes.append(
                        f"  💰 {mid}: cost ${old_in}/{old_out} → ${new_in}/{new_out} /Mtok"
                    )

        # --- Actualizar scores ---
        if update_scores:
            bench_entry = bench_scores.get(norm)

            # Intentar también sin la parte del provider si es modelo con prefijo
            if not bench_entry and "/" in mid:
                bare = mid.split("/", 1)[1].lower()
                bench_entry = bench_scores.get(bare)

            if bench_entry:
                new_scores = compute_task_scores_from_pinchbench(bench_entry)
                old_scores = model.get("task_scores", {})

                score_changes = []
                for field in TASK_FIELDS:
                    old_val = old_scores.get(field, 0)
                    new_val = new_scores.get(field, old_val)
                    if old_val != new_val:
                        score_changes.append(f"{field}: {old_val}→{new_val}")

                if score_changes:
                    model["task_scores"] = new_scores
                    changes.append(
                        f"  📊 {mid}: scores updated ({', '.join(score_changes)})"
                    )

    return updated, changes


# ---------------------------------------------------------------------------
# YAML diff display
# ---------------------------------------------------------------------------

def show_diff(original: list[dict], updated: list[dict], changes: list[str]) -> None:
    """Muestra un resumen de los cambios encontrados."""
    if not changes:
        print("\n✅  No changes detected — catalog is up to date.")
        return

    print(f"\n📋  {len(changes)} change(s) found:\n")
    for c in changes:
        print(c)


# ---------------------------------------------------------------------------
# YAML serialization
# ---------------------------------------------------------------------------

def load_yaml(path: Path) -> list[dict]:
    with open(path, "r", encoding="utf-8") as f:
        return yaml.safe_load(f) or []


def dump_yaml(models: list[dict], path: Path) -> None:
    """
    Escribe models.yaml preservando el formato existente lo mejor posible.
    Usa representación literal para strings con caracteres especiales.
    """
    with open(path, "w", encoding="utf-8") as f:
        yaml.dump(
            models,
            f,
            default_flow_style=False,
            allow_unicode=True,
            sort_keys=False,
            indent=2,
        )


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(
        description="Aegis Model Catalog Synchronizer — actualiza models.yaml con datos reales"
    )
    parser.add_argument(
        "--only-prices",
        action="store_true",
        help="Solo actualizar precios desde OpenRouter",
    )
    parser.add_argument(
        "--only-scores",
        action="store_true",
        help="Solo actualizar task_scores desde PinchBench",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Muestra los cambios sin escribir el archivo",
    )
    parser.add_argument(
        "--add-new",
        action="store_true",
        help="Agrega modelos nuevos encontrados en las fuentes (por defecto NO se agregan)",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suprime output informativo",
    )
    parser.add_argument(
        "--models-yaml",
        type=Path,
        default=MODELS_YAML,
        help=f"Path al models.yaml (default: {MODELS_YAML})",
    )

    args = parser.parse_args()
    verbose = not args.quiet

    update_prices = not args.only_scores
    update_scores = not args.only_prices

    # Verificar que el archivo existe
    if not args.models_yaml.exists():
        print(f"❌  models.yaml not found at: {args.models_yaml}", file=sys.stderr)
        print("    Asegurate de ejecutar el script desde la raíz del repo.", file=sys.stderr)
        sys.exit(1)

    if verbose:
        print(f"📂  Loading: {args.models_yaml}")

    models = load_yaml(args.models_yaml)

    if verbose:
        print(f"    {len(models)} models loaded")

    # Fetchear fuentes externas
    prices = {}
    bench_scores = {}

    if update_prices:
        prices = fetch_openrouter_prices(verbose=verbose)

    if update_scores:
        bench_scores = fetch_pinchbench_scores(verbose=verbose)

    # Aplicar actualizaciones
    updated, changes = update_models(
        models=models,
        prices=prices,
        bench_scores=bench_scores,
        update_prices=update_prices,
        update_scores=update_scores,
        add_new=args.add_new,
        verbose=verbose,
    )

    # Mostrar diff
    show_diff(models, updated, changes)

    # Escribir si no es dry-run y hay cambios
    if not args.dry_run and changes:
        dump_yaml(updated, args.models_yaml)
        print(f"\n💾  Written: {args.models_yaml}")
        print(f"    Run 'cargo build --workspace' to verify the YAML parses correctly.")
    elif args.dry_run and changes:
        print("\n🔍  Dry-run mode — no files written.")
    elif not changes:
        pass  # ya informado arriba


if __name__ == "__main__":
    main()

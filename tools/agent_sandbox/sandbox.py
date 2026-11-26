#!/usr/bin/env python3
"""
Aegis Cognitive Agent Sandbox & Personal Assistant Simulator
=============================================================
Entorno aislado con:
1. Chat infinito con memoria a corto plazo (ventana deslizante) y largo plazo (hechos y preferencias).
2. Registro inteligente de finanzas/gastos/notas con inferencia semántica ("super" -> comida).
3. Ejecución de agentes en segundo plano (no bloqueante en modo interactivo).
4. Demonio Chronos de aprendizaje y consolidación en segundo plano.
"""

import os
import sys
import json
import time
import threading
import argparse
import subprocess
from pathlib import Path
from typing import Dict, Any, List, Optional

import requests

BASE_DIR = Path(__file__).resolve().parent
WORKSPACE_DIR = BASE_DIR / "workspace"
PROMPTS_DIR = BASE_DIR / "prompts"
ENV_PATH = BASE_DIR.parent.parent / ".env"
LOG_FILE = BASE_DIR / "sandbox.log"
MEMORY_FILE = WORKSPACE_DIR / "aegis_memory.json"


def log_event(message: str):
    """Appends an event to the persistent sandbox.log file."""
    timestamp = time.strftime("%Y-%m-%d %H:%M:%S")
    with open(LOG_FILE, "a", encoding="utf-8") as f:
        f.write(f"[{timestamp}] {message}\n")


def load_env(env_file: Path) -> Dict[str, List[str]]:
    env = {}
    if not env_file.exists():
        return env
    with open(env_file, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith("#") and "=" in line:
                k, v = line.split("=", 1)
                keys = [item.strip() for item in v.split(",") if item.strip()]
                env[k.strip()] = keys
    return env


class LLMClient:
    def __init__(self, provider: str = "gemini", model: Optional[str] = None):
        self.env = load_env(ENV_PATH)
        self.provider = provider
        self.key_idx = 0
        self.lock = threading.Lock()

        if provider == "gemini":
            self.api_keys = self.env.get("GEMINI_API_KEYS") or self.env.get("GEMINI_API_KEY", [])
            self.api_url = "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
            self.model = model or "gemini-3.6-flash"
        elif provider == "openrouter":
            self.api_keys = self.env.get("OPENROUTER_API_KEYS") or self.env.get("OPENROUTER_API_KEY", [])
            self.api_url = "https://openrouter.ai/api/v1/chat/completions"
            self.model = model or "openrouter/free"
        elif provider == "groq":
            self.api_keys = self.env.get("GROQ_API_KEYS") or self.env.get("GROQ_API_KEY", [])
            self.api_url = "https://api.groq.com/openai/v1/chat/completions"
            self.model = model or "openai/gpt-oss-120b"
        else:
            raise ValueError(f"Unknown provider: {provider}")

        if not self.api_keys:
            raise ValueError(f"API key for {provider} not found in {ENV_PATH}")

        # Configure automatic cross-provider fallback (Gemini <-> OpenRouter)
        if provider == "gemini":
            self.fallback_keys = self.env.get("OPENROUTER_API_KEYS") or self.env.get("OPENROUTER_API_KEY", [])
            self.fallback_url = "https://openrouter.ai/api/v1/chat/completions"
            self.fallback_model = "google/gemini-2.5-flash"
        else:
            self.fallback_keys = self.env.get("GEMINI_API_KEYS") or self.env.get("GEMINI_API_KEY", [])
            self.fallback_url = "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
            self.fallback_model = "gemini-2.5-flash"

    def chat_completion(
        self,
        messages: List[Dict[str, Any]],
        tools: Optional[List[Dict[str, Any]]] = None,
        temperature: float = 0.2,
        max_tokens: int = 2048,
    ) -> Dict[str, Any]:
        num_keys = len(self.api_keys)
        last_err = "Unknown error"

        for attempt in range(max(num_keys, 3)):
            with self.lock:
                current_key = self.api_keys[self.key_idx % num_keys]

            headers = {
                "Authorization": f"Bearer {current_key}",
                "Content-Type": "application/json",
            }
            payload = {
                "model": self.model,
                "messages": messages,
                "temperature": temperature,
                "max_tokens": max_tokens,
            }
            if tools:
                payload["tools"] = tools

            start_time = time.time()
            try:
                resp = requests.post(self.api_url, json=payload, headers=headers, timeout=30)
                elapsed = time.time() - start_time
                if resp.status_code in (429, 500, 502, 503, 504):
                    last_err = f"HTTP {resp.status_code}: {resp.text[:200]}"
                    with self.lock:
                        self.key_idx = (self.key_idx + 1) % num_keys
                    time.sleep(1.0)
                    continue

                if resp.status_code != 200:
                    return {
                        "error": f"HTTP {resp.status_code}: {resp.text}",
                        "elapsed": elapsed,
                    }
                data = resp.json()
                choice = data["choices"][0]["message"]
                usage = data.get("usage", {})
                return {
                    "role": choice.get("role", "assistant"),
                    "content": choice.get("content"),
                    "tool_calls": choice.get("tool_calls"),
                    "usage": usage,
                    "elapsed": elapsed,
                }
            except Exception as e:
                last_err = str(e)
                time.sleep(0.5)

        # Fallback to secondary provider if primary provider is overloaded (503/429)
        if self.fallback_keys:
            for fb_key in self.fallback_keys[:2]:
                headers = {
                    "Authorization": f"Bearer {fb_key}",
                    "Content-Type": "application/json",
                }
                payload = {
                    "model": self.fallback_model,
                    "messages": messages,
                    "temperature": temperature,
                    "max_tokens": max_tokens,
                }
                if tools:
                    payload["tools"] = tools
                start_time = time.time()
                try:
                    resp = requests.post(self.fallback_url, json=payload, headers=headers, timeout=30)
                    elapsed = time.time() - start_time
                    if resp.status_code == 200:
                        data = resp.json()
                        choice = data["choices"][0]["message"]
                        return {
                            "role": choice.get("role", "assistant"),
                            "content": choice.get("content"),
                            "tool_calls": choice.get("tool_calls"),
                            "usage": data.get("usage", {}),
                            "elapsed": elapsed,
                        }
                except Exception:
                    pass

        return {"error": f"All keys/fallbacks exhausted. Last error: {last_err}", "elapsed": 0}


# =====================================================================
# PERSONAL MEMORY STORE (Corto y Largo Plazo + Finanzas + Recordatorios)
# =====================================================================

CATEGORY_ALIASES = {
    "super": "comida",
    "supermercado": "comida",
    "almacen": "comida",
    "carniceria": "comida",
    "verduleria": "comida",
    "panaderia": "comida",
    "restaurante": "comida",
    "delivery": "comida",
    "alimentos": "comida",
    "nafta": "transporte",
    "combustible": "transporte",
    "uber": "transporte",
    "cabify": "transporte",
    "subte": "transporte",
    "colectivo": "transporte",
    "peaje": "transporte",
    "farmacia": "salud",
    "medico": "salud",
    "gimnasio": "salud",
}


def _fold_spanish_lowercase(text: str) -> str:
    trans = str.maketrans("áàäâãéèëêíìïîóòöôõúùüûñç", "aaaaaeeeeiiiiooooouuuunc")
    return text.strip().lower().translate(trans)


def _stem_spanish_token(tok: str) -> str:
    if len(tok) > 6 and tok.endswith("ciones"):
        return tok[:-6] + "cion"
    if len(tok) > 4 and tok.endswith("ces"):
        return tok[:-3] + "z"
    if len(tok) > 4 and tok.endswith("es") and tok[-3] in "rnldjsz":
        return tok[:-2]
    if len(tok) > 3 and tok.endswith("s"):
        return tok[:-1]
    return tok


_FACT_STOPWORDS = {
    "un", "uno", "una", "unos", "unas", "dos", "tres", "cuatro", "cinco", "seis", "siete",
    "ocho", "nueve", "diez", "primer", "primero", "segunda", "segundo", "tercer", "tercera",
    "tercero", "otro", "otra", "otros", "otras", "mas", "menos", "varios", "varias", "solo",
    "solamente", "actualmente", "ahora", "hoy", "el", "la", "los", "las", "de", "del", "al",
    "a", "en", "con", "por", "para", "sin", "sobre", "es", "soy", "sos", "son", "fue", "era",
    "ser", "esta", "estoy", "estan", "estar", "tiene", "tengo", "tenes", "tienes", "tener",
    "hay", "hace", "hacer", "da", "doy", "dar", "dicta", "dicto", "dictar", "trabaja",
    "trabajo", "trabajar", "usuario", "persona", "que", "como", "cuando", "donde", "y", "e",
    "o", "u", "pero", "tambien", "ademas", "su", "sus", "mi", "mis", "tu", "tus", "me", "te",
    "se", "le", "les", "lo", "nos",
}


def normalize_fact_topic_tokens(fact: str) -> List[str]:
    import re
    folded = _fold_spanish_lowercase(fact)
    raw_tokens = [t for t in re.split(r"[^a-z0-9]+", folded) if t]
    tokens = []
    for t in raw_tokens:
        if t.isdigit() or t in _FACT_STOPWORDS:
            continue
        stemmed = _stem_spanish_token(t)
        if len(stemmed) >= 3:
            tokens.append(stemmed)
    return sorted(set(tokens))


def facts_share_topic(cat_a: str, fact_a: str, cat_b: str, fact_b: str) -> bool:
    if fact_a.strip().lower() == fact_b.strip().lower():
        return True
    ta = normalize_fact_topic_tokens(fact_a)
    tb = normalize_fact_topic_tokens(fact_b)
    if not ta or not tb:
        return False
    if ta == tb:
        return True
    set_a, set_b = set(ta), set(tb)
    inter = len(set_a & set_b)
    if inter < 2:
        return False
    union = len(set_a | set_b)
    min_len = min(len(set_a), len(set_b))
    jaccard = inter / union
    containment = inter / min_len
    ca, cb = cat_a.strip().lower(), cat_b.strip().lower()
    if ca == cb or ca == "general" or cb == "general":
        return jaccard >= 0.60 or containment >= 0.80
    return jaccard >= 0.75 or (containment >= 0.99 and min_len >= 2)


class PersonalMemoryStore:
    def __init__(self, path: Path):
        self.path = path
        self.lock = threading.Lock()
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.data = self._load()

    def _default_data(self) -> Dict[str, Any]:
        return {
            "facts": [],
            "ledger": [],
            "reminders": [],
            "chat_history": [],
            "compacted_summary": "",
            "last_consolidated_turn_count": 0,
        }

    def _load(self) -> Dict[str, Any]:
        if not self.path.exists():
            return self._default_data()
        try:
            loaded = json.loads(self.path.read_text(encoding="utf-8"))
            default = self._default_data()
            for k, v in default.items():
                if k not in loaded:
                    loaded[k] = v
            return loaded
        except Exception:
            return self._default_data()

    def save(self):
        with self.lock:
            self.path.write_text(
                json.dumps(self.data, indent=2, ensure_ascii=False),
                encoding="utf-8",
            )

    def reset(self):
        with self.lock:
            self.data = self._default_data()
        self.save()

    def add_fact(
        self,
        category: str,
        fact: str,
        action: str = "upsert",
        fact_id: Optional[Any] = None,
    ) -> Dict[str, Any]:
        category = (category or "").strip().lower() or "general"
        fact = (fact or "").strip()
        action = (action or "upsert").strip().lower()
        parsed_id: Optional[int] = None
        if fact_id is not None and str(fact_id).strip():
            try:
                parsed_id = int(str(fact_id).strip().lstrip("#").strip())
            except ValueError:
                parsed_id = None

        with self.lock:
            if action == "delete":
                if parsed_id is None:
                    return {"status": "error", "detail": "Indicá el id de la nota a eliminar."}
                before = len(self.data["facts"])
                self.data["facts"] = [f for f in self.data["facts"] if f.get("id") != parsed_id]
                if len(self.data["facts"]) == before:
                    return {"status": "not_found", "id": parsed_id}
                res = {"status": "deleted", "id": parsed_id}
            else:
                if not fact:
                    return {"status": "error", "detail": "El hecho no puede estar vacío."}
                ts = time.strftime("%Y-%m-%d %H:%M:%S")
                updated_entry = None
                prev_fact = None

                if parsed_id is not None:
                    for existing in self.data["facts"]:
                        if existing.get("id") == parsed_id:
                            prev_fact = existing["fact"]
                            existing["category"] = category
                            existing["fact"] = fact
                            existing["timestamp"] = ts
                            updated_entry = existing
                            break

                if updated_entry is not None:
                    self.data["facts"] = [
                        f for f in self.data["facts"]
                        if f.get("id") == parsed_id
                        or not facts_share_topic(category, fact, f.get("category", "general"), f.get("fact", ""))
                    ]
                    res = {"status": "updated", "previous_fact": prev_fact, "entry": updated_entry}
                else:
                    exact = next(
                        (f for f in self.data["facts"] if f["fact"].strip().lower() == fact.lower()),
                        None,
                    )
                    if exact is not None:
                        return {"status": "already_known", "fact": exact}

                    matches = [
                        f for f in self.data["facts"]
                        if facts_share_topic(category, fact, f.get("category", "general"), f.get("fact", ""))
                    ]
                    if matches:
                        primary = matches[-1]
                        prev_fact = primary["fact"]
                        primary["category"] = category
                        primary["fact"] = fact
                        primary["timestamp"] = ts
                        dup_ids = {m["id"] for m in matches[:-1]}
                        if dup_ids:
                            self.data["facts"] = [f for f in self.data["facts"] if f["id"] not in dup_ids]
                        res = {"status": "updated", "previous_fact": prev_fact, "entry": primary}
                    else:
                        next_id = max((int(f.get("id", 0)) for f in self.data["facts"]), default=0) + 1
                        entry = {
                            "id": next_id,
                            "category": category,
                            "fact": fact,
                            "timestamp": ts,
                        }
                        self.data["facts"].append(entry)
                        res = {"status": "saved", "entry": entry}
        self.save()
        return res

    def log_entry(self, domain: str, category: str, amount: float, description: str) -> Dict[str, Any]:
        domain = (domain or "gastos").strip().lower()
        raw_cat = (category or "otros").strip().lower()
        norm_cat = CATEGORY_ALIASES.get(raw_cat, raw_cat)

        with self.lock:
            entry = {
                "id": len(self.data["ledger"]) + 1,
                "domain": domain,
                "category": norm_cat,
                "amount": float(amount),
                "description": description.strip() or raw_cat,
                "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
            }
            self.data["ledger"].append(entry)

            cat_total = sum(
                x["amount"]
                for x in self.data["ledger"]
                if x["domain"] == domain and x["category"] == norm_cat
            )
            domain_total = sum(
                x["amount"]
                for x in self.data["ledger"]
                if x["domain"] == domain
            )
        self.save()
        return {
            "status": "recorded",
            "entry": entry,
            "category_total": cat_total,
            "domain_total": domain_total,
        }

    def query_ledger(self, domain: Optional[str] = None, category: Optional[str] = None) -> Dict[str, Any]:
        with self.lock:
            items = list(self.data["ledger"])

        if domain:
            d_norm = domain.strip().lower()
            items = [x for x in items if x["domain"] == d_norm]

        if category and category.strip().lower() not in ("all", "todos", "todas", ""):
            c_raw = category.strip().lower()
            c_norm = CATEGORY_ALIASES.get(c_raw, c_raw)
            items = [x for x in items if x["category"] == c_norm]

        by_category: Dict[str, float] = {}
        for x in items:
            cat = x["category"]
            by_category[cat] = by_category.get(cat, 0.0) + x["amount"]

        total = sum(x["amount"] for x in items)
        return {
            "total": total,
            "count": len(items),
            "totals_by_category": by_category,
            "recent_entries": items[-10:],
        }

    def manage_reminder(self, action: str, text: str = "", time_hint: str = "") -> Dict[str, Any]:
        action = (action or "list").strip().lower()
        with self.lock:
            if action == "add":
                item = {
                    "id": len(self.data["reminders"]) + 1,
                    "text": text.strip(),
                    "time_hint": time_hint.strip(),
                    "completed": False,
                    "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
                }
                self.data["reminders"].append(item)
                res = {"status": "added", "reminder": item}
            elif action == "complete":
                matched = []
                for r in self.data["reminders"]:
                    if not r["completed"] and text.lower() in r["text"].lower():
                        r["completed"] = True
                        matched.append(r)
                res = {"status": "completed", "updated": matched}
            else:
                pending = [r for r in self.data["reminders"] if not r["completed"]]
                res = {"status": "ok", "pending_reminders": pending}
        self.save()
        return res

    def append_turn(self, role: str, content: str):
        if not content:
            return
        with self.lock:
            self.data["chat_history"].append({
                "role": role,
                "content": content,
                "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
            })
            # Sliding window: keep last 24 messages in active window
            if len(self.data["chat_history"]) > 24:
                older = self.data["chat_history"][:-16]
                self.data["chat_history"] = self.data["chat_history"][-16:]
                snippet = " | ".join(f"{m['role']}: {m['content'][:80]}" for m in older)
                if self.data["compacted_summary"]:
                    self.data["compacted_summary"] = (self.data["compacted_summary"] + " || " + snippet)[-2000:]
                else:
                    self.data["compacted_summary"] = snippet[-2000:]
        self.save()

    def get_recent_messages(self, limit: int = 12) -> List[Dict[str, str]]:
        with self.lock:
            turns = self.data["chat_history"][-limit:]
            return [{"role": t["role"], "content": t["content"]} for t in turns]

    def build_system_memory_injection(self) -> str:
        with self.lock:
            facts = list(self.data["facts"])
            ledger = list(self.data["ledger"])
            reminders = [r for r in self.data["reminders"] if not r["completed"]]
            summary = self.data.get("compacted_summary", "")

        sections = []

        if facts:
            fact_lines = [f"- [#{f['id']}] [{f['category']}] {f['fact']}" for f in facts[-25:]]
            sections.append("[MEMORIA A LARGO PLAZO DEL USUARIO]\n" + "\n".join(fact_lines))
        else:
            sections.append("[MEMORIA A LARGO PLAZO DEL USUARIO]\n(Sin hechos registrados aún)")

        if ledger:
            by_cat: Dict[str, float] = {}
            for item in ledger:
                if item["domain"] == "gastos":
                    by_cat[item["category"]] = by_cat.get(item["category"], 0.0) + item["amount"]
            tot_str = ", ".join(f"{k}: ${v:,.0f}" for k, v in by_cat.items())
            last_3 = [f"  * {x['description']} ({x['category']}): ${x['amount']:,.0f}" for x in ledger[-3:]]
            sections.append(
                f"[RESUMEN FINANCIERO ACTUAL]\nTotales de gastos por categoría: {tot_str or 'Ninguno'}\nÚltimos movimientos:\n"
                + "\n".join(last_3)
            )

        if reminders:
            rem_lines = [f"- {r['text']}" + (f" ({r['time_hint']})" if r['time_hint'] else "") for r in reminders]
            sections.append("[RECORDATORIOS PENDIENTES]\n" + "\n".join(rem_lines))

        if summary:
            sections.append(f"[RESUMEN DE CONVERSACIONES ANTERIORES]\n{summary}")

        return "\n\n".join(sections)


# =====================================================================
# DEFINICIÓN DE HERRAMIENTAS (Chat Aegis + Workers)
# =====================================================================

TOOL_LOG_EXPENSE = {
    "type": "function",
    "function": {
        "name": "log_expense_or_entry",
        "description": "Registra un gasto, compra, ingreso o movimiento numérico del usuario. Deducí la categoría por sentido común (ej: 'super' -> 'comida', 'nafta' -> 'transporte').",
        "parameters": {
            "type": "object",
            "properties": {
                "domain": {
                    "type": "string",
                    "enum": ["gastos", "ingresos", "otros"],
                    "description": "Dominio del registro (por defecto 'gastos')."
                },
                "category": {
                    "type": "string",
                    "description": "Categoría normalizada en minúsculas: 'comida', 'transporte', 'hogar', 'salud', 'ocio', 'tecnologia', 'impuestos', etc."
                },
                "amount": {
                    "type": "number",
                    "description": "Monto numérico del gasto o ingreso."
                },
                "description": {
                    "type": "string",
                    "description": "Descripción clara del ítem (ej: 'Supermercado', 'Carnicería', 'Carga de nafta')."
                }
            },
            "required": ["category", "amount", "description"]
        }
    }
}

TOOL_QUERY_LEDGER = {
    "type": "function",
    "function": {
        "name": "query_ledger",
        "description": "Consulta los gastos acumulados, totales por categoría o historial de movimientos financieros.",
        "parameters": {
            "type": "object",
            "properties": {
                "domain": {"type": "string", "description": "'gastos' o 'ingresos' (opcional)"},
                "category": {"type": "string", "description": "Categoría específica como 'comida', 'transporte' o 'all' para ver todas."}
            }
        }
    }
}

TOOL_REMEMBER_FACT = {
    "type": "function",
    "function": {
        "name": "remember_fact",
        "description": "Guarda, actualiza (consolidando una nota existente sin duplicar) o elimina un dato importante sobre el usuario en la memoria a largo plazo.",
        "parameters": {
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["upsert", "update", "delete"],
                    "description": "Acción: 'upsert' (actualiza si ya hay una nota del mismo tema o crea una nueva), 'update' (reemplaza por 'id'), o 'delete' (elimina por 'id')."
                },
                "id": {
                    "type": "string",
                    "description": "ID numérico de la nota existente (ej. '12' o '#12') tomado de [MEMORIA A LARGO PLAZO DEL USUARIO]."
                },
                "category": {
                    "type": "string",
                    "description": "Categoría del recuerdo: 'preferencias', 'rutina', 'salud', 'trabajo', 'familia', 'general'."
                },
                "fact": {
                    "type": "string",
                    "description": "El hecho concreto y consolidado a recordar a largo plazo."
                }
            },
            "required": ["category", "fact"]
        }
    }
}

TOOL_MANAGE_REMINDER = {
    "type": "function",
    "function": {
        "name": "manage_reminder",
        "description": "Agrega, lista o marca como completado un recordatorio o tarea pendiente del usuario.",
        "parameters": {
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ["add", "list", "complete"]},
                "text": {"type": "string", "description": "Texto del recordatorio o tarea."},
                "time_hint": {"type": "string", "description": "Día/hora o referencia temporal (ej: 'mañana 10hs', 'jueves 20hs')."}
            },
            "required": ["action"]
        }
    }
}

TOOL_SPAWN_AGENT = {
    "type": "function",
    "function": {
        "name": "spawn_agent",
        "description": "Delega una tarea técnica pesada (crear scripts, editar código, leer/escribir archivos en disco, ejecutar comandos o investigar en segundo plano) a un agente trabajador.",
        "parameters": {
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Identificador corto de la tarea"},
                "scope": {"type": "string", "description": "Descripción completa y detallada de lo que debe hacer el agente"},
                "role": {"type": "string", "enum": ["lead_worker", "sub_specialist", "project_supervisor", "specialist"]},
                "task_type": {"type": "string", "enum": ["code", "analysis", "planning"]}
            },
            "required": ["role", "scope"]
        }
    }
}

TOOL_READ_FILE = {
    "type": "function",
    "function": {
        "name": "read_file",
        "description": "Read the contents of a file in the workspace.",
        "parameters": {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Relative path inside workspace"}
            },
            "required": ["path"]
        }
    }
}

TOOL_WRITE_FILE = {
    "type": "function",
    "function": {
        "name": "write_file",
        "description": "Create or overwrite a file in the workspace.",
        "parameters": {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Relative path inside workspace"},
                "content": {"type": "string", "description": "The full file content"}
            },
            "required": ["path", "content"]
        }
    }
}

TOOL_LIST_FILES = {
    "type": "function",
    "function": {
        "name": "list_files",
        "description": "List files and directories in the workspace.",
        "parameters": {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Relative directory path (empty or '.' for root)"}
            }
        }
    }
}

TOOL_EXECUTE_COMMAND = {
    "type": "function",
    "function": {
        "name": "execute_command",
        "description": "Execute a shell command inside the workspace directory.",
        "parameters": {
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Shell command to run"}
            },
            "required": ["command"]
        }
    }
}

TOOL_REPORT = {
    "type": "function",
    "function": {
        "name": "report",
        "description": "Report final results of the task to complete execution.",
        "parameters": {
            "type": "object",
            "properties": {
                "status": {"type": "string", "enum": ["completed", "error", "blocked"]},
                "summary": {"type": "string", "description": "Summary of actions taken and final result"}
            },
            "required": ["status", "summary"]
        }
    }
}


class MockToolExecutor:
    """Executes workspace file & shell tools."""

    def __init__(self, workspace: Path):
        self.workspace = workspace
        self.workspace.mkdir(parents=True, exist_ok=True)

    def execute(self, name: str, args: Dict[str, Any]) -> str:
        try:
            if name == "read_file":
                target = (self.workspace / args["path"]).resolve()
                if not str(target).startswith(str(self.workspace)):
                    return json.dumps({"error": "Path traversal forbidden"})
                if not target.exists():
                    return json.dumps({"error": f"File not found: {args['path']}"})
                return json.dumps({"content": target.read_text(encoding="utf-8")})

            elif name == "write_file":
                target = (self.workspace / args["path"]).resolve()
                if not str(target).startswith(str(self.workspace)):
                    return json.dumps({"error": "Path traversal forbidden"})
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(args["content"], encoding="utf-8")
                return json.dumps({"status": "ok", "bytes_written": len(args["content"])})

            elif name == "list_files":
                rel = args.get("path", "") or "."
                target = (self.workspace / rel).resolve()
                if not target.exists() or not target.is_dir():
                    return json.dumps({"error": f"Directory not found: {rel}"})
                items = [p.name + ("/" if p.is_dir() else "") for p in target.iterdir()]
                return json.dumps({"files": items})

            elif name == "execute_command":
                cmd = args["command"]
                res = subprocess.run(
                    cmd,
                    shell=True,
                    cwd=str(self.workspace),
                    capture_output=True,
                    text=True,
                    timeout=15,
                )
                return json.dumps({
                    "returncode": res.returncode,
                    "stdout": res.stdout[:2000],
                    "stderr": res.stderr[:2000],
                })

            elif name == "report":
                return json.dumps({"acknowledged": True, "status": args.get("status")})

            return json.dumps({"error": f"Unknown tool: {name}"})

        except Exception as e:
            return json.dumps({"error": str(e)})


class AgentSandbox:
    def __init__(self, client: LLMClient, prompt_version: str = "v2_direct", verbose: bool = False):
        self.client = client
        self.version = prompt_version
        self.verbose = verbose
        self.prompts_path = PROMPTS_DIR / prompt_version
        self.executor = MockToolExecutor(WORKSPACE_DIR)
        self.memory = PersonalMemoryStore(MEMORY_FILE)
        self.background_jobs: List[Dict[str, Any]] = []
        self.last_user_activity = time.time()

        # Load prompts
        if prompt_version == "v2_direct":
            self.chat_prompt = (self.prompts_path / "chat_conductor.md").read_text(encoding="utf-8")
            self.worker_prompt = (self.prompts_path / "lead_worker.md").read_text(encoding="utf-8")
            self.sub_prompt = (self.prompts_path / "sub_specialist.md").read_text(encoding="utf-8")
        else:
            self.chat_prompt = (self.prompts_path / "chat_agent.md").read_text(encoding="utf-8")
            self.worker_prompt = (self.prompts_path / "project_supervisor.md").read_text(encoding="utf-8")
            self.sub_prompt = (self.prompts_path / "specialist.md").read_text(encoding="utf-8")

    def execute_personal_tool(self, fn_name: str, fn_args: Dict[str, Any]) -> str:
        """Executes instant personal assistant tools (memory, expenses, reminders)."""
        if fn_name == "log_expense_or_entry":
            res = self.memory.log_entry(
                domain=fn_args.get("domain", "gastos"),
                category=fn_args.get("category", "otros"),
                amount=fn_args.get("amount", 0),
                description=fn_args.get("description", ""),
            )
            return json.dumps(res, ensure_ascii=False)

        elif fn_name == "query_ledger":
            res = self.memory.query_ledger(
                domain=fn_args.get("domain"),
                category=fn_args.get("category"),
            )
            return json.dumps(res, ensure_ascii=False)

        elif fn_name == "remember_fact":
            res = self.memory.add_fact(
                category=fn_args.get("category", "general"),
                fact=fn_args.get("fact", ""),
                action=fn_args.get("action", "upsert"),
                fact_id=fn_args.get("id"),
            )
            return json.dumps(res, ensure_ascii=False)

        elif fn_name == "manage_reminder":
            res = self.memory.manage_reminder(
                action=fn_args.get("action", "list"),
                text=fn_args.get("text", ""),
                time_hint=fn_args.get("time_hint", ""),
            )
            return json.dumps(res, ensure_ascii=False)

        return json.dumps({"error": f"Unknown personal tool: {fn_name}"})

    def run_worker_loop(self, task_name: str, task_scope: str, max_turns: int = 10) -> Dict[str, Any]:
        """Runs the Worker execution loop with tools and immediate automatic dispatch."""
        system_content = f"[TASK CONTEXT]\nTask: {task_name}\nScope: {task_scope}\n\n{self.worker_prompt}"
        messages = [
            {"role": "system", "content": system_content},
            {"role": "user", "content": f"Begin execution for task: {task_scope}"}
        ]

        if self.verbose:
            print(f"\n[DEBUG: Lead Worker] Starting loop for task '{task_name}'")
            print(f"[DEBUG: Lead Worker] Scope: {task_scope}")
        log_event(f"Worker Loop Started: task={task_name}, scope={task_scope[:100]}")

        tools = [TOOL_READ_FILE, TOOL_WRITE_FILE, TOOL_LIST_FILES, TOOL_EXECUTE_COMMAND, TOOL_REPORT]
        execution_trace = []

        for turn in range(max_turns):
            if self.verbose:
                print(f"\n--- [Lead Worker Turn {turn + 1}/{max_turns}] ---")

            resp = self.client.chat_completion(messages, tools=tools)
            if "error" in resp:
                err_msg = resp["error"]
                if self.verbose:
                    print(f"[ERROR: Lead Worker] LLM call failed: {err_msg}")
                log_event(f"Worker Turn {turn+1} Error: {err_msg}")
                return {"status": "error", "error": err_msg, "trace": execution_trace}

            tool_calls = resp.get("tool_calls") or []
            content = resp.get("content")

            trace_entry = {
                "turn": turn + 1,
                "content": content,
                "tool_calls": tool_calls,
                "elapsed": resp.get("elapsed", 0),
            }
            execution_trace.append(trace_entry)

            if self.verbose and content:
                print(f"[Lead Worker Speech]: {content}")

            finished = False
            report_summary = None

            assistant_msg: Dict[str, Any] = {"role": "assistant"}
            if content:
                assistant_msg["content"] = content
            if tool_calls:
                assistant_msg["tool_calls"] = tool_calls
            messages.append(assistant_msg)

            if not tool_calls:
                if self.verbose:
                    print(f"[DEBUG: Lead Worker] Completed turn with text (0 tool calls).")
                log_event(f"Worker Completed (text): {content[:100] if content else 'empty'}")
                return {
                    "status": "completed",
                    "summary": content or "Execution finished without tool call",
                    "turns": turn + 1,
                    "trace": execution_trace,
                }

            for tc in tool_calls:
                fn = tc["function"]
                fn_name = fn["name"]
                try:
                    fn_args = json.loads(fn["arguments"])
                except Exception as e:
                    fn_args = {"raw": fn.get("arguments", "")}
                    if self.verbose:
                        print(f"[ERROR] Failed to parse tool arguments JSON: {e}")

                if self.verbose:
                    args_preview = json.dumps(fn_args, ensure_ascii=False)
                    if len(args_preview) > 120:
                        args_preview = args_preview[:117] + "..."
                    print(f"[Worker Tool Call] -> {fn_name}({args_preview})")

                tool_result = self.executor.execute(fn_name, fn_args)

                if self.verbose:
                    res_preview = tool_result
                    if len(res_preview) > 120:
                        res_preview = res_preview[:117] + "..."
                    print(f"[Worker Tool Result] <- {res_preview}")

                log_event(f"Tool {fn_name}: args={fn_args}, result={tool_result[:100]}")

                messages.append({
                    "role": "tool",
                    "tool_call_id": tc["id"],
                    "content": tool_result
                })

                if fn_name == "report":
                    finished = True
                    report_summary = fn_args.get("summary", "")

            if finished:
                if self.verbose:
                    print(f"[DEBUG: Lead Worker] Finished via report(): {report_summary}")
                log_event(f"Worker Finished via report(): {report_summary[:100]}")
                return {
                    "status": "completed",
                    "summary": report_summary,
                    "turns": turn + 1,
                    "trace": execution_trace,
                }

        log_event(f"Worker max turns ({max_turns}) reached.")
        return {
            "status": "max_turns_reached",
            "turns": max_turns,
            "trace": execution_trace,
        }

    def _spawn_background_worker(self, task_name: str, task_scope: str) -> Dict[str, Any]:
        """Spawns a worker in a non-blocking background thread so the user can keep chatting."""
        job_id = f"bg_job_{len(self.background_jobs) + 1}"
        job_record = {
            "job_id": job_id,
            "name": task_name,
            "scope": task_scope,
            "status": "running",
            "started_at": time.strftime("%H:%M:%S"),
            "summary": None,
        }
        self.background_jobs.append(job_record)

        def _bg_runner():
            res = self.run_worker_loop(task_name, task_scope)
            job_record["status"] = res.get("status", "completed")
            job_record["summary"] = res.get("summary") or res.get("error") or "Finalizado"
            job_record["finished_at"] = time.strftime("%H:%M:%S")
            # Inject completion into memory so Aegis knows it finished
            self.memory.append_turn(
                "system",
                f"[NOTIFICACIÓN DE TAREA EN SEGUNDO PLANO '{task_name}' ({job_id})]: Estado={job_record['status']}. Resumen: {job_record['summary']}"
            )
            print(f"\n\n🔔 [SEGUNDO PLANO COMPLETADO - {task_name} ({job_id})]:")
            print(f"   Resumen: {job_record['summary']}")
            print("[User] > ", end="", flush=True)

        t = threading.Thread(target=_bg_runner, daemon=True)
        t.start()
        return job_record

    def process_user_query(self, query: str, async_background: bool = False) -> Dict[str, Any]:
        """
        End-to-end Aegis Personal Assistant flow:
        - Injects long-term memory, financial ledger summary, reminders & recent chat window.
        - Runs ReAct loop for instant personal tools (expenses, memory, reminders).
        - Delegates heavy tasks via spawn_agent (async in interactive mode, sync in test mode).
        """
        start = time.time()
        self.last_user_activity = start
        log_event(f"Chat Turn: User message = {query[:100]}")

        memory_block = self.memory.build_system_memory_injection()
        full_system_prompt = f"{self.chat_prompt}\n\n{memory_block}"

        messages: List[Dict[str, Any]] = [{"role": "system", "content": full_system_prompt}]
        messages.extend(self.memory.get_recent_messages(limit=12))
        messages.append({"role": "user", "content": query})

        chat_tools = [
            TOOL_LOG_EXPENSE,
            TOOL_QUERY_LEDGER,
            TOOL_REMEMBER_FACT,
            TOOL_MANAGE_REMINDER,
            TOOL_SPAWN_AGENT,
        ]

        personal_tools_used = []
        spawn_call_args = None
        worker_result = None

        # Up to 3 ReAct iterations for instant personal tools
        for step in range(3):
            if self.verbose:
                print(f"\n[DEBUG: Aegis Chat] Turn {step + 1} (Model: {self.client.model})")

            resp = self.client.chat_completion(messages, tools=chat_tools)
            if "error" in resp:
                return {"status": "error", "message": resp["error"], "elapsed": time.time() - start}

            tool_calls = resp.get("tool_calls") or []
            content = resp.get("content")

            if not tool_calls:
                # Final conversational response ready
                self.memory.append_turn("user", query)
                self.memory.append_turn("assistant", content or "")
                if spawn_call_args:
                    return {
                        "type": "delegated_task",
                        "chat_reply": content,
                        "spawn_args": spawn_call_args,
                        "worker_result": worker_result,
                        "personal_tools_used": personal_tools_used,
                        "elapsed": time.time() - start,
                    }
                return {
                    "type": "direct_reply",
                    "chat_reply": content,
                    "personal_tools_used": personal_tools_used,
                    "elapsed": time.time() - start,
                }

            # Append assistant message with tool_calls
            assistant_msg: Dict[str, Any] = {"role": "assistant"}
            if content:
                assistant_msg["content"] = content
            assistant_msg["tool_calls"] = tool_calls
            messages.append(assistant_msg)

            for tc in tool_calls:
                fn_name = tc["function"]["name"]
                try:
                    fn_args = json.loads(tc["function"]["arguments"])
                except Exception:
                    fn_args = {}

                if self.verbose:
                    print(f"[Aegis Tool] -> {fn_name}({json.dumps(fn_args, ensure_ascii=False)})")

                if fn_name == "spawn_agent":
                    spawn_call_args = fn_args
                    task_name = fn_args.get("name", "task")
                    task_scope = fn_args.get("scope", query)

                    if async_background:
                        job_info = self._spawn_background_worker(task_name, task_scope)
                        worker_result = {"status": "running_in_background", "job": job_info}
                        tool_output = json.dumps({
                            "status": "spawned_in_background",
                            "job_id": job_info["job_id"],
                            "message": "Agent is now working in the background. Confirm briefly to the user so they can keep chatting."
                        })
                    else:
                        worker_result = self.run_worker_loop(task_name, task_scope)
                        tool_output = json.dumps({
                            "status": worker_result.get("status"),
                            "summary": worker_result.get("summary"),
                        }, ensure_ascii=False)
                else:
                    tool_output = self.execute_personal_tool(fn_name, fn_args)
                    personal_tools_used.append({"tool": fn_name, "args": fn_args, "result": json.loads(tool_output)})
                    log_event(f"Personal Tool {fn_name}: args={fn_args}, result={tool_output[:120]}")

                if self.verbose:
                    print(f"[Aegis Tool Result] <- {tool_output[:140]}")

                messages.append({
                    "role": "tool",
                    "tool_call_id": tc["id"],
                    "content": tool_output,
                })

            # If in sync mode and we already ran a heavy worker, we can return right away
            if spawn_call_args and not async_background:
                self.memory.append_turn("user", query)
                summary_text = (worker_result or {}).get("summary") or "Tarea completada."
                self.memory.append_turn("assistant", summary_text)
                return {
                    "type": "delegated_task",
                    "chat_reply": content or summary_text,
                    "spawn_args": spawn_call_args,
                    "worker_result": worker_result,
                    "personal_tools_used": personal_tools_used,
                    "elapsed": time.time() - start,
                }

        # Fallback if loop ends
        return {
            "type": "direct_reply",
            "chat_reply": "Listo, procesado.",
            "personal_tools_used": personal_tools_used,
            "elapsed": time.time() - start,
        }

    def run_chronos_consolidation(self) -> Dict[str, Any]:
        """
        Chronos Daemon: Analyzes recent dialogue when idle and extracts implicit facts
        or habits about the user into long-term memory.
        """
        recent = self.memory.get_recent_messages(limit=10)
        if not recent:
            return {"status": "skipped", "reason": "No recent conversation to consolidate."}

        convo_text = "\n".join(f"{m['role'].upper()}: {m['content']}" for m in recent)
        prompt = (
            "Sos el módulo Chronos de consolidación de memoria a largo plazo de Aegis OS.\n"
            "Analizá la siguiente conversación reciente y extraé únicamente hechos nuevos, gustos, "
            "costumbres, proyectos o datos personales del usuario que valga la pena recordar en el futuro.\n"
            "Respondé ESTRICTAMENTE con un JSON válido con el formato:\n"
            '{"extracted_facts": [{"category": "preferencias|rutina|trabajo|salud|general", "fact": "..."}]}\n'
            "Si no hay hechos nuevos relevantes, respondé: {\"extracted_facts\": []}\n\n"
            f"CONVERSACIÓN RECIENTE:\n{convo_text}"
        )

        resp = self.client.chat_completion([{"role": "user", "content": prompt}], temperature=0.1)
        if "error" in resp:
            return {"status": "error", "error": resp["error"]}

        raw = (resp.get("content") or "").strip()
        # Extract JSON block if wrapped in markdown
        if "```" in raw:
            parts = raw.split("```")
            for p in parts:
                p_clean = p.strip()
                if p_clean.startswith("json"):
                    p_clean = p_clean[4:].strip()
                if p_clean.startswith("{"):
                    raw = p_clean
                    break

        try:
            parsed = json.loads(raw)
            new_facts = parsed.get("extracted_facts", [])
            saved = []
            for item in new_facts:
                if item.get("fact"):
                    r = self.memory.add_fact(item.get("category", "general"), item["fact"])
                    if r["status"] == "saved":
                        saved.append(r["entry"])
            log_event(f"Chronos Consolidation: extracted {len(saved)} new facts.")
            return {"status": "ok", "new_facts_saved": saved}
        except Exception as e:
            return {"status": "parse_error", "raw": raw, "error": str(e)}


def run_interactive(sandbox: AgentSandbox):
    print("\n" + "=" * 68)
    print(" 🤖 Aegis Personal Assistant Sandbox — Chat Único Infinito")
    print(f" Modelo    : {sandbox.client.provider} ({sandbox.client.model})")
    print(f" Memoria   : {MEMORY_FILE}")
    print(f" Workspace : {WORKSPACE_DIR}")
    print(f" Verbose   : {'ON' if sandbox.verbose else 'OFF'} (cambiar con /verbose)")
    print("=" * 68)
    print(" Comandos rápidos:")
    print("   /memoria     - Ver todo lo que Aegis recuerda de vos")
    print("   /gastos      - Ver tabla de gastos y totales por categoría")
    print("   /chronos     - Ejecutar aprendizaje automático en segundo plano")
    print("   /jobs        - Ver estado de agentes trabajando en segundo plano")
    print("   /workspace   - Listar archivos creados por los agentes")
    print("   /view <file> - Ver contenido de un archivo")
    print("   /clear       - Limpiar memoria y archivos para empezar de cero")
    print("   /verbose     - Activar/desactivar trazas técnicas en vivo")
    print("   /exit        - Salir")
    print("=" * 68 + "\n")

    while True:
        try:
            user_input = input("\n[User] > ").strip()
            if not user_input:
                continue

            if user_input.lower() in ("/exit", "/quit", "exit", "quit"):
                break

            if user_input == "/verbose":
                sandbox.verbose = not sandbox.verbose
                print(f"[Sandbox] Modo debug detallado: {'ACTIVADO' if sandbox.verbose else 'DESACTIVADO'}")
                continue

            if user_input in ("/memoria", "/memory"):
                print("\n" + "-" * 55)
                print(sandbox.memory.build_system_memory_injection())
                print("-" * 55)
                continue

            if user_input == "/gastos":
                q = sandbox.memory.query_ledger(domain="gastos")
                print("\n--- 💰 [Balance de Gastos Registrados] ---")
                print(f"Total Acumulado: ${q['total']:,.2f} ({q['count']} movimientos)")
                print("\nDesglose por categoría:")
                for cat, tot in q["totals_by_category"].items():
                    print(f"  • {cat.capitalize():<15}: ${tot:,.2f}")
                print("\nÚltimos movimientos:")
                for item in q["recent_entries"]:
                    print(f"  [{item['timestamp']}] {item['description']} ({item['category']}) -> ${item['amount']:,.2f}")
                print("-------------------------------------------")
                continue

            if user_input == "/chronos":
                print("[Chronos] Analizando conversación reciente para extraer aprendizajes a largo plazo...")
                res = sandbox.run_chronos_consolidation()
                if res.get("status") == "ok":
                    saved = res.get("new_facts_saved", [])
                    if saved:
                        print(f"[Chronos] ¡Se aprendieron {len(saved)} hecho(s) nuevo(s)!:")
                        for s in saved:
                            print(f"  + [{s['category']}] {s['fact']}")
                    else:
                        print("[Chronos] No se detectaron hechos nuevos para agregar (ya estaban en memoria).")
                else:
                    print(f"[Chronos] Resultado: {res}")
                continue

            if user_input == "/jobs":
                if not sandbox.background_jobs:
                    print("[Jobs] No hay tareas en segundo plano registradas en esta sesión.")
                else:
                    print("\n--- ⚙️ [Tareas en Segundo Plano] ---")
                    for j in sandbox.background_jobs:
                        print(f"  • [{j['job_id']}] {j['name']} | Estado: {j['status']} | Inicio: {j['started_at']}")
                        if j.get("summary"):
                            print(f"    Resumen: {j['summary']}")
                continue

            if user_input == "/workspace":
                files = list(WORKSPACE_DIR.glob("**/*"))
                print(f"\n[Archivos en Workspace ({len(files)} total)]:")
                for f in files:
                    if f.is_file():
                        rel = f.relative_to(WORKSPACE_DIR)
                        print(f"  - {rel} ({f.stat().st_size} bytes)")
                continue

            if user_input.startswith("/view "):
                fname = user_input.split(" ", 1)[1].strip()
                target = (WORKSPACE_DIR / fname).resolve()
                if target.exists() and target.is_file():
                    print(f"\n--- [Contenido de {fname}] ---")
                    print(target.read_text(encoding="utf-8"))
                    print("--- [Fin del Archivo] ---")
                else:
                    print(f"[Error] Archivo no encontrado: {fname}")
                continue

            if user_input == "/clear":
                sandbox.memory.reset()
                for f in WORKSPACE_DIR.glob("*"):
                    if f.is_file() and f.name != "aegis_memory.json":
                        f.unlink()
                print("[Sandbox] Memoria de Aegis y archivos del workspace reiniciados a cero.")
                continue

            if user_input == "/log":
                if LOG_FILE.exists():
                    lines = LOG_FILE.read_text(encoding="utf-8").splitlines()[-25:]
                    print("\n--- [Últimos Eventos en sandbox.log] ---")
                    for l in lines:
                        print(l)
                    print("--- [Fin de Logs] ---")
                else:
                    print("[Sandbox] El archivo de log está vacío.")
                continue

            if not sandbox.verbose:
                print("[Aegis pensando...]")

            # En modo interactivo las tareas pesadas corren en segundo plano sin bloquear el chat
            res = sandbox.process_user_query(user_input, async_background=True)

            if res.get("status") == "error":
                print(f"\n[ERROR] Falló la solicitud: {res.get('message')}")
                continue

            if res.get("personal_tools_used") and not sandbox.verbose:
                for pt in res["personal_tools_used"]:
                    tname = pt["tool"]
                    if tname == "log_expense_or_entry":
                        entry = pt["result"].get("entry", {})
                        cat_tot = pt["result"].get("category_total", 0)
                        print(f"  💾 [Gasto registrado: ${entry.get('amount', 0):,.0f} en '{entry.get('category')}' | Acumulado categoría: ${cat_tot:,.0f}]")
                    elif tname == "remember_fact":
                        entry = pt["result"].get("entry") or pt["result"].get("fact", {})
                        print(f"  🧠 [Memoria a largo plazo guardada: {entry.get('fact', '')}]")
                    elif tname == "manage_reminder":
                        print(f"  📌 [Recordatorio actualizado]")

            if res["type"] == "delegated_task":
                args = res["spawn_args"]
                w = res.get("worker_result") or {}
                job = w.get("job", {})
                print(f"  🚀 [Agente lanzado en 2do plano ({job.get('job_id', 'worker')}): {args.get('name')}]")

            if res.get("chat_reply"):
                print(f"\n[Aegis] {res['chat_reply']}")

            print(f"(Tiempo: {res['elapsed']:.2f}s)")

        except KeyboardInterrupt:
            print("\nSaliendo.")
            break


def run_scenarios(sandbox: AgentSandbox, scenarios_path: Path):
    scenarios = json.loads(scenarios_path.read_text(encoding="utf-8"))
    sandbox.memory.reset()
    print("\n" + "=" * 70)
    print(f" Running Aegis Personal Assistant Benchmark Scenarios ({sandbox.version})")
    print(f" Model: {sandbox.client.model} ({sandbox.client.provider})")
    print("=" * 70)

    passed = 0
    total = len(scenarios)

    for sc in scenarios:
        sc_id = sc["id"]
        name = sc["name"]
        msg = sc["user_message"]
        exp_action = sc["expected_action"]

        print(f"\n[{sc_id}] {name}")
        print(f"  Input: \"{msg}\"")

        if sc_id == "code_bug_fix":
            (WORKSPACE_DIR / "config.json").write_text(json.dumps({"port": 8080, "env": "dev"}), encoding="utf-8")

        res = sandbox.process_user_query(msg, async_background=False)

        if res.get("status") == "error":
            print(f"  Result: FAIL (API Error: {res.get('message')})")
            continue

        actual_action = res["type"]
        tools_used = [t["tool"] for t in res.get("personal_tools_used", [])]
        success = False

        if exp_action == "direct_reply" and actual_action == "direct_reply":
            success = True
            reply_preview = (res.get("chat_reply") or "").replace("\n", " ")
            print(f"  Result: PASS (Answered directly: \"{reply_preview[:65]}...\")")
        elif exp_action == "personal_tool" and tools_used:
            exp_tool = sc.get("expected_tool")
            if not exp_tool or exp_tool in tools_used:
                success = True
                reply_preview = (res.get("chat_reply") or "").replace("\n", " ")
                print(f"  Result: PASS (Used {tools_used} -> \"{reply_preview[:65]}...\")")
            else:
                print(f"  Result: FAIL (Expected tool {exp_tool}, got {tools_used})")
        elif exp_action == "spawn_agent" and actual_action == "delegated_task":
            w = res["worker_result"]
            if w.get("status") == "completed":
                success = True
                print(f"  Result: PASS (Spawned worker, executed & reported in {w['turns']} turns)")
                print(f"  Summary: {w.get('summary', '')[:80]}...")
            else:
                print(f"  Result: FAIL (Worker failed with status: {w.get('status')})")
        else:
            print(f"  Result: FAIL (Expected {exp_action}, got {actual_action}, tools={tools_used})")

        if success:
            passed += 1

    print("\n" + "=" * 70)
    print(f" Scorecard: {passed}/{total} Passed ({(passed/total)*100:.0f}%)")
    print("=" * 70 + "\n")


def main():
    parser = argparse.ArgumentParser(description="Aegis Personal Assistant Simulator & Sandbox")
    parser.add_argument("--provider", default="openrouter", choices=["gemini", "openrouter", "groq"])
    parser.add_argument("--model", default=None, help="LLM model name")
    parser.add_argument("--version", default="v2_direct", choices=["v1_legacy", "v2_direct"])
    parser.add_argument("--test", action="store_true", help="Run automated scenarios")
    parser.add_argument("--interactive", action="store_true", help="Start interactive CLI session")
    parser.add_argument("-v", "--verbose", action="store_true", help="Enable detailed debug output")
    args = parser.parse_args()

    client = LLMClient(provider=args.provider, model=args.model)
    sandbox = AgentSandbox(client, prompt_version=args.version, verbose=args.verbose)

    if args.test:
        run_scenarios(sandbox, BASE_DIR / "scenarios.json")
    else:
        run_interactive(sandbox)


if __name__ == "__main__":
    main()

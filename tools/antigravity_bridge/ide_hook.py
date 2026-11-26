#!/usr/bin/env python3
"""
CORE-385: Antigravity IDE Lifecycle Hook (`Stop` event bridge).

Connects an active Antigravity IDE conversation to the local Aegis-Core assistant
(`http://127.0.0.1:8000`) so that:
1. Whenever Antigravity finishes a response, the assistant receives the summary
   and reads it aloud to the user (on desktop or mobile).
2. If the user sent an instruction to Antigravity via voice or chat (`outbox`),
   this hook returns `{"decision": "continue", "reason": "<instruction>"}` so
   Antigravity automatically processes the user's reply.

Usage in `.gemini/hooks/hooks.json`:
{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "python tools/antigravity_bridge/ide_hook.py",
            "timeout": 10
          }
        ]
      }
    ]
  }
}
"""

from __future__ import annotations

import json
import os
import sys
import urllib.request
from pathlib import Path
from typing import Optional


AEGIS_BASE_URL = os.environ.get("AEGIS_URL", "http://127.0.0.1:8000").rstrip("/")
AEGIS_HOOK_TOKEN = os.environ.get("AEGIS_HOOK_TOKEN", "").strip()


def _auth_headers(extra: Optional[dict[str, str]] = None) -> dict[str, str]:
    headers = dict(extra) if extra else {}
    if AEGIS_HOOK_TOKEN:
        headers["X-Aegis-Hook-Token"] = AEGIS_HOOK_TOKEN
    return headers


def extract_latest_assistant_response(transcript_path: str) -> Optional[str]:
    path = Path(transcript_path)
    if not path.is_file():
        return None
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        for raw in reversed(lines):
            raw = raw.strip()
            if not raw:
                continue
            try:
                step = json.loads(raw)
            except Exception:
                continue
            if step.get("type") == "PLANNER_RESPONSE":
                content = (step.get("content") or "").strip()
                if content:
                    return content
    except Exception:
        return None
    return None


def post_json(url: str, payload: dict) -> None:
    try:
        data = json.dumps(payload).encode("utf-8")
        req = urllib.request.Request(
            url,
            data=data,
            headers=_auth_headers({"Content-Type": "application/json"}),
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=3.0) as _:
            pass
    except Exception:
        pass


def pop_outbox(url: str) -> Optional[str]:
    try:
        req = urllib.request.Request(url, headers=_auth_headers(), method="GET")
        with urllib.request.urlopen(req, timeout=3.0) as resp:
            body = json.loads(resp.read().decode("utf-8"))
            if body.get("has_message") and body.get("instruction"):
                return str(body["instruction"]).strip()
    except Exception:
        pass
    return None


def main() -> int:
    try:
        raw_stdin = sys.stdin.read().strip()
        hook_input = json.loads(raw_stdin) if raw_stdin else {}
    except Exception:
        hook_input = {}

    conversation_id = hook_input.get("conversationId") or ""
    transcript_path = hook_input.get("transcriptPath") or ""

    if transcript_path:
        summary = extract_latest_assistant_response(transcript_path)
        if summary:
            post_json(
                f"{AEGIS_BASE_URL}/api/antigravity/ide/notify",
                {
                    "conversation_id": conversation_id,
                    "project_name": "Antigravity IDE",
                    "summary": summary[:1500],
                },
            )

    instruction = pop_outbox(f"{AEGIS_BASE_URL}/api/antigravity/ide/outbox/pop")
    if instruction:
        sys.stdout.write(
            json.dumps(
                {
                    "decision": "continue",
                    "reason": f"[Mensaje del usuario vía Asistente Personal]: {instruction}",
                },
                ensure_ascii=False,
            )
            + "\n"
        )
        sys.stdout.flush()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

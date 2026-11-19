#!/usr/bin/env python3
"""
Tests unitarios y de protocolo NDJSON para `tools/antigravity_bridge/agy_worker.py` (CORE-383).
"""

import json
import os
import subprocess
import sys
import unittest
from pathlib import Path

WORKER_PATH = Path(__file__).resolve().parent / "agy_worker.py"


class TestAntigravityWorker(unittest.TestCase):
    def setUp(self):
        env = os.environ.copy()
        env["AEGIS_AGY_MOCK"] = "1"
        env["PYTHONIOENCODING"] = "utf-8"
        self.proc = subprocess.Popen(
            [sys.executable, str(WORKER_PATH)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            env=env,
        )

    def tearDown(self):
        if self.proc.poll() is None:
            try:
                self.send_cmd({"id": "bye", "cmd": "shutdown"})
                self.proc.wait(timeout=2)
            except Exception:
                self.proc.kill()
        if self.proc.stdin:
            self.proc.stdin.close()
        if self.proc.stdout:
            self.proc.stdout.close()
        if self.proc.stderr:
            self.proc.stderr.close()

    def send_cmd(self, payload: dict) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write(json.dumps(payload) + "\n")
        self.proc.stdin.flush()

    def read_event(self) -> dict:
        assert self.proc.stdout is not None
        line = self.proc.stdout.readline()
        self.assertTrue(line, "Worker closed stdout unexpectedly")
        return json.loads(line.strip())

    def test_status_and_multi_turn_session_persistence(self):
        # 1. Check status
        self.send_cmd({"id": "st-1", "cmd": "status"})
        st = self.read_event()
        self.assertEqual(st["id"], "st-1")
        self.assertEqual(st["event"], "status")
        self.assertTrue(st["sdk_installed"])

        # 2. First turn in session "proj-alpha"
        self.send_cmd({
            "id": "turn-1",
            "cmd": "chat",
            "session_id": "proj-alpha",
            "prompt": "Revisá el archivo README y los tests",
        })

        events_turn_1 = []
        while True:
            ev = self.read_event()
            events_turn_1.append(ev)
            if ev.get("event") in ("completed", "error"):
                break

        event_types = [e["event"] for e in events_turn_1]
        self.assertIn("thought", event_types)
        self.assertIn("tool_call", event_types)
        self.assertIn("token", event_types)
        self.assertIn("completed", event_types)

        completed_1 = events_turn_1[-1]
        self.assertIn("turno #1", completed_1["full_text"])

        # 3. Second turn in SAME session "proj-alpha" -> must preserve session state (turn #2)
        self.send_cmd({
            "id": "turn-2",
            "cmd": "chat",
            "session_id": "proj-alpha",
            "prompt": "Ahora agregá un test adicional",
        })

        events_turn_2 = []
        while True:
            ev = self.read_event()
            events_turn_2.append(ev)
            if ev.get("event") in ("completed", "error"):
                break

        completed_2 = events_turn_2[-1]
        self.assertEqual(completed_2["event"], "completed")
        self.assertIn("turno #2", completed_2["full_text"])

        # 4. Close session
        self.send_cmd({"id": "close-1", "cmd": "close_session", "session_id": "proj-alpha"})
        closed_ev = self.read_event()
        self.assertEqual(closed_ev["event"], "session_closed")
        self.assertTrue(closed_ev["closed"])


if __name__ == "__main__":
    unittest.main()

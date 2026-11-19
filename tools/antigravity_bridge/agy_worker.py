#!/usr/bin/env python3
"""
CORE-383 — Antigravity SDK Worker Daemon (`agy_worker.py`)
Puente persistente entre Aegis-Core (`AgentOrchestrator`) y el SDK oficial `google-antigravity`.
Lee comandos JSON-Lines desde `stdin` y emite eventos de streaming (`thought`, `tool_call`,
`token`, `completed`, `error`, `status`) por `stdout`.
"""

import asyncio
import importlib.metadata
import importlib.util
import json
import os
import sys
import traceback
from typing import Any, Dict, Optional


def emit(payload: Dict[str, Any]) -> None:
    """Emite una línea JSON atómica hacia stdout y fuerza flush inmediato."""
    sys.stdout.write(json.dumps(payload, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def check_sdk_status() -> Dict[str, Any]:
    """Verifica si el paquete `google-antigravity` está instalado y su versión."""
    if os.environ.get("AEGIS_AGY_MOCK") == "1":
        return {
            "sdk_installed": True,
            "version": "0.1.23-mock",
            "mode": "mock",
        }
    try:
        spec = importlib.util.find_spec("google.antigravity")
        if spec is None:
            return {
                "sdk_installed": False,
                "version": None,
                "mode": "missing",
            }
        try:
            ver = importlib.metadata.version("google-antigravity")
        except Exception:
            ver = "installed"
        return {
            "sdk_installed": True,
            "version": ver,
            "mode": "native",
        }
    except Exception as exc:
        return {
            "sdk_installed": False,
            "version": None,
            "mode": "error",
            "detail": str(exc),
        }


class MockResponse:
    """Respuesta simulada compatible con la interfaz async de `google.antigravity`."""

    def __init__(self, prompt: str, history_len: int):
        self._prompt = prompt
        self._history_len = history_len
        self.thoughts = self._iter_thoughts()
        self.tool_calls = self._iter_tools()

    async def _iter_thoughts(self):
        yield f"Analizando solicitud en sesión de Antigravity (turno #{self._history_len})..."

    async def _iter_tools(self):
        if "archivo" in self._prompt.lower() or "test" in self._prompt.lower():
            class _Call:
                name = "read_file"
                args = {"Path": "README.md"}
            yield _Call()

    def __aiter__(self):
        return self._iter_tokens()

    async def _iter_tokens(self):
        reply = (
            f"[Antigravity SDK] Procesé tu solicitud (turno #{self._history_len}): "
            f"\"{self._prompt}\". Tarea completada con éxito."
        )
        for word in reply.split(" "):
            yield word + " "
            await asyncio.sleep(0.005)


class MockAgentSession:
    """Sesión simulada para pruebas determinísticas cuando `AEGIS_AGY_MOCK=1`."""

    def __init__(self, system_instructions: str = ""):
        self.system_instructions = system_instructions
        self.turns = 0

    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc, tb):
        return False

    async def chat(self, prompt: str) -> MockResponse:
        self.turns += 1
        return MockResponse(prompt, self.turns)


class SessionManager:
    """Gestiona las sesiones activas de `google.antigravity.Agent` por `session_id`."""

    def __init__(self) -> None:
        self.sessions: Dict[str, Any] = {}
        self.active_tasks: Dict[str, asyncio.Task] = {}

    async def get_or_create_session(
        self,
        session_id: str,
        system_instructions: str = "",
        allow_write: bool = True,
        workspace: Optional[str] = None,
    ) -> Any:
        if session_id in self.sessions:
            return self.sessions[session_id]

        if workspace and os.path.isdir(workspace):
            try:
                os.chdir(workspace)
            except OSError:
                pass

        if os.environ.get("AEGIS_AGY_MOCK") == "1":
            agent = MockAgentSession(system_instructions=system_instructions)
            await agent.__aenter__()
            self.sessions[session_id] = agent
            return agent

        from google.antigravity import Agent, CapabilitiesConfig, LocalAgentConfig

        kwargs: Dict[str, Any] = {}
        if system_instructions:
            kwargs["system_instructions"] = system_instructions
        if allow_write:
            kwargs["capabilities"] = CapabilitiesConfig()

        config = LocalAgentConfig(**kwargs)
        agent = Agent(config)
        await agent.__aenter__()
        self.sessions[session_id] = agent
        return agent

    async def close_session(self, session_id: str) -> bool:
        task = self.active_tasks.pop(session_id, None)
        if task and not task.done():
            task.cancel()
        agent = self.sessions.pop(session_id, None)
        if agent is not None:
            try:
                await agent.__aexit__(None, None, None)
            except Exception:
                pass
            return True
        return False

    async def close_all(self) -> None:
        for sid in list(self.sessions.keys()):
            await self.close_session(sid)


async def handle_chat_command(
    manager: SessionManager,
    req_id: str,
    cmd_data: Dict[str, Any],
) -> None:
    session_id = str(cmd_data.get("session_id") or "default")
    prompt = str(cmd_data.get("prompt") or "").strip()
    system_instructions = str(cmd_data.get("system_instructions") or "")
    allow_write = bool(cmd_data.get("allow_write", True))
    workspace = cmd_data.get("workspace")

    if not prompt:
        emit({
            "id": req_id,
            "session_id": session_id,
            "event": "error",
            "code": "EMPTY_PROMPT",
            "message": "El prompt enviado a Antigravity está vacío.",
        })
        return

    status = check_sdk_status()
    if not status.get("sdk_installed"):
        emit({
            "id": req_id,
            "session_id": session_id,
            "event": "error",
            "code": "SDK_NOT_AVAILABLE",
            "message": (
                "El paquete google-antigravity no está instalado en el entorno Python. "
                "Instalá con: pip install google-antigravity"
            ),
        })
        return

    try:
        agent = await manager.get_or_create_session(
            session_id=session_id,
            system_instructions=system_instructions,
            allow_write=allow_write,
            workspace=workspace,
        )
        response = await agent.chat(prompt)

        async def pump_thoughts() -> None:
            thoughts_stream = getattr(response, "thoughts", None)
            if thoughts_stream is None:
                return
            try:
                async for thought in thoughts_stream:
                    text = str(thought).strip()
                    if text:
                        emit({
                            "id": req_id,
                            "session_id": session_id,
                            "event": "thought",
                            "text": text,
                        })
            except Exception:
                pass

        async def pump_tools() -> None:
            tools_stream = getattr(response, "tool_calls", None)
            if tools_stream is None:
                return
            try:
                async for call in tools_stream:
                    name = getattr(call, "name", "tool")
                    args = getattr(call, "args", {})
                    if not isinstance(args, dict):
                        try:
                            args = dict(args)
                        except Exception:
                            args = {"raw": str(args)}
                    emit({
                        "id": req_id,
                        "session_id": session_id,
                        "event": "tool_call",
                        "name": str(name),
                        "args": args,
                    })
            except Exception:
                pass

        full_chunks = []

        async def pump_tokens() -> None:
            async for token in response:
                piece = str(token)
                if piece:
                    full_chunks.append(piece)
                    emit({
                        "id": req_id,
                        "session_id": session_id,
                        "event": "token",
                        "text": piece,
                    })

        await asyncio.gather(
            pump_thoughts(),
            pump_tools(),
            pump_tokens(),
        )

        full_text = "".join(full_chunks).strip()
        emit({
            "id": req_id,
            "session_id": session_id,
            "event": "completed",
            "full_text": full_text,
        })

    except asyncio.CancelledError:
        emit({
            "id": req_id,
            "session_id": session_id,
            "event": "error",
            "code": "CANCELLED",
            "message": "Ejecución de Antigravity cancelada por el usuario.",
        })
    except Exception as exc:
        emit({
            "id": req_id,
            "session_id": session_id,
            "event": "error",
            "code": "EXECUTION_ERROR",
            "message": f"Error en Antigravity SDK: {exc}",
            "trace": traceback.format_exc(limit=3),
        })


async def main_loop() -> None:
    manager = SessionManager()
    loop = asyncio.get_running_loop()

    def read_stdin_line() -> str:
        return sys.stdin.readline()

    try:
        while True:
            line = await loop.run_in_executor(None, read_stdin_line)
            if not line:
                break
            line = line.strip()
            if not line:
                continue

            try:
                data = json.loads(line)
            except json.JSONDecodeError as exc:
                emit({
                    "id": "invalid",
                    "event": "error",
                    "code": "INVALID_JSON",
                    "message": str(exc),
                })
                continue

            req_id = str(data.get("id") or "req")
            cmd = str(data.get("cmd") or "").lower()

            if cmd in ("status", "ping"):
                st = check_sdk_status()
                emit({
                    "id": req_id,
                    "event": "status",
                    "active_sessions": list(manager.sessions.keys()),
                    **st,
                })
            elif cmd == "chat":
                session_id = str(data.get("session_id") or "default")
                prev_task = manager.active_tasks.get(session_id)
                if prev_task and not prev_task.done():
                    prev_task.cancel()
                task = asyncio.create_task(handle_chat_command(manager, req_id, data))
                manager.active_tasks[session_id] = task
            elif cmd == "cancel":
                session_id = str(data.get("session_id") or "default")
                task = manager.active_tasks.pop(session_id, None)
                if task and not task.done():
                    task.cancel()
                emit({
                    "id": req_id,
                    "session_id": session_id,
                    "event": "cancelled",
                })
            elif cmd == "close_session":
                session_id = str(data.get("session_id") or "default")
                closed = await manager.close_session(session_id)
                emit({
                    "id": req_id,
                    "session_id": session_id,
                    "event": "session_closed",
                    "closed": closed,
                })
            elif cmd == "shutdown":
                await manager.close_all()
                emit({"id": req_id, "event": "shutdown"})
                break
            else:
                emit({
                    "id": req_id,
                    "event": "error",
                    "code": "UNKNOWN_CMD",
                    "message": f"Comando desconocido: {cmd}",
                })
    finally:
        await manager.close_all()


if __name__ == "__main__":
    if sys.platform == "win32":
        try:
            sys.stdin.reconfigure(encoding="utf-8")
            sys.stdout.reconfigure(encoding="utf-8")
        except Exception:
            pass
    asyncio.run(main_loop())

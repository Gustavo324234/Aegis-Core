#!/usr/bin/env python3
"""
Launcher for the isolated Aegis test web server (`ank-server`) alongside the
interactive CLI sandbox (`run_sandbox.bat`).

Never reads or overwrites the production `C:\\ProgramData\\Aegis\\aegis.env`
file and defaults to HTTP port 8090 / gRPC port 50059 so it can run safely
beside the production AegisOS service on port 8000.
"""

from __future__ import annotations

import argparse
import ctypes
import os
import secrets
import shutil
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Dict, Optional

REPO_ROOT = Path(__file__).resolve().parents[2]
SANDBOX_DIR = REPO_ROOT / "tools" / "agent_sandbox"
LOCAL_SANDBOX_ENV = SANDBOX_DIR / ".env"
DEFAULT_HTTP_PORT = 8090
DEFAULT_GRPC_PORT = 50059


def parse_env_vars(env_path: Path) -> Dict[str, str]:
    """Parses simple KEY=VALUE pairs from an env file without executing it."""
    parsed: Dict[str, str] = {}
    if not env_path.exists():
        return parsed
    for raw_line in env_path.read_text(encoding="utf-8", errors="ignore").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        cleaned = value.strip()
        if len(cleaned) >= 2 and (
            (cleaned[0] == '"' and cleaned[-1] == '"')
            or (cleaned[0] == "'" and cleaned[-1] == "'")
        ):
            cleaned = cleaned[1:-1]
        parsed[key.strip()] = cleaned
    return parsed


def resolve_sandbox_env(
    repo_root: Path = REPO_ROOT,
    explicit_env: Optional[str] = None,
) -> Path:
    """
    Resolves which env file the test web server should load:
    1. Explicit `AEGIS_SANDBOX_ENV` if set and present.
    2. Repo-local `tools/agent_sandbox/.env` if present.
    3. Auto-created `tools/agent_sandbox/.env` isolated on port 8090.
    """
    if explicit_env:
        candidate = Path(explicit_env)
        if candidate.exists():
            return candidate

    local_env = repo_root / "tools" / "agent_sandbox" / ".env"
    if local_env.exists():
        return local_env

    return bootstrap_sandbox_env(repo_root, local_env)


def bootstrap_sandbox_env(repo_root: Path, target_env: Path) -> Path:
    """Creates an isolated sandbox `.env` seeded with API keys from root `.env`."""
    target_env.parent.mkdir(parents=True, exist_ok=True)
    root_vars = parse_env_vars(repo_root / ".env")
    data_dir = (repo_root / "tools" / "agent_sandbox" / "server_data").as_posix()
    agents_dir = (repo_root / "kernel" / "config" / "agents").as_posix()
    ui_dist_dir = (repo_root / "shell" / "ui" / "dist").as_posix()

    lines = [
        f"AEGIS_ROOT_KEY={secrets.token_hex(32)}",
        f'AEGIS_DATA_DIR="{data_dir}"',
        f'AEGIS_AGENTS_CONFIG_DIR="{agents_dir}"',
        f'UI_DIST_PATH="{ui_dist_dir}"',
        "AEGIS_MODEL_PROFILE=cloud",
        "DEFAULT_MODEL_PREF=CloudOnly",
        "RUST_LOG=info",
        f"AEGIS_HTTP_PORT={DEFAULT_HTTP_PORT}",
        f"ANK_HTTP_PORT={DEFAULT_HTTP_PORT}",
        f"AEGIS_GRPC_PORT={DEFAULT_GRPC_PORT}",
    ]

    for key_name in (
        "AEGIS_PLUGIN_ROOT_KEY",
        "OPENROUTER_API_KEYS",
        "OPENROUTER_API_KEY",
        "GROQ_API_KEYS",
        "GROQ_API_KEY",
        "GEMINI_API_KEYS",
        "GEMINI_API_KEY",
        "ELEVENLABS_API_KEY",
        "OLLAMA_API_KEY",
    ):
        val = root_vars.get(key_name)
        if val:
            lines.append(f"{key_name}={val}")

    target_env.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return target_env


def select_ank_server_binary(repo_root: Path = REPO_ROOT) -> Optional[Path]:
    """Returns the newest compiled `ank-server` executable, if present."""
    ext = ".exe" if os.name == "nt" else ""
    candidates = [
        repo_root / "target" / "debug" / f"ank-server{ext}",
        repo_root / "target" / "release" / f"ank-server{ext}",
    ]
    existing = [path for path in candidates if path.is_file()]
    if not existing:
        return None
    return max(existing, key=lambda p: p.stat().st_mtime)


def resolve_http_port(env_path: Path) -> int:
    """Reads `AEGIS_HTTP_PORT` or `ANK_HTTP_PORT`, defaulting to 8090."""
    env_vars = parse_env_vars(env_path)
    raw = env_vars.get("AEGIS_HTTP_PORT") or env_vars.get("ANK_HTTP_PORT")
    if raw:
        try:
            port = int(raw)
            if 1 <= port <= 65535:
                return port
        except ValueError:
            pass
    return DEFAULT_HTTP_PORT


def is_port_listening(port: int, host: str = "127.0.0.1") -> bool:
    """Checks via TCP connect whether the test server port is already accepting connections."""
    try:
        with socket.create_connection((host, port), timeout=0.4):
            return True
    except OSError:
        return False


def wait_for_server_ready(port: int, timeout_secs: float = 15.0) -> bool:
    """Polls `/health` (falling back to TCP connect) until the test web server is ready."""
    deadline = time.monotonic() + timeout_secs
    health_url = f"http://127.0.0.1:{port}/health"
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(health_url, timeout=1.0) as resp:
                if 200 <= resp.status < 500:
                    return True
        except urllib.error.HTTPError:
            return True
        except OSError:
            pass
        if is_port_listening(port):
            return True
        time.sleep(0.35)
    return False


def ensure_ui_dist(repo_root: Path = REPO_ROOT) -> None:
    """Builds `shell/ui` if `dist/index.html` is missing."""
    index_html = repo_root / "shell" / "ui" / "dist" / "index.html"
    if index_html.is_file():
        return
    npm_cmd = shutil.which("npm.cmd") or shutil.which("npm")
    if not npm_cmd:
        raise RuntimeError(
            "Falta shell/ui/dist/index.html y no se encontro npm para compilar la UI."
        )
    print("[INFO] Compilando UI web (shell/ui) porque dist no existe...")
    subprocess.run(
        [npm_cmd, "--prefix", "shell/ui", "run", "build"],
        cwd=str(repo_root),
        check=True,
    )


def ensure_server_binary(repo_root: Path = REPO_ROOT) -> Path:
    """Finds the newest `ank-server` binary or builds it via `cargo build -p ank-server`."""
    binary = select_ank_server_binary(repo_root)
    if binary is not None:
        return binary
    cargo_cmd = shutil.which("cargo")
    if not cargo_cmd:
        raise RuntimeError(
            "No se encontro target/debug/ank-server ni target/release/ank-server ni cargo."
        )
    print("[INFO] Compilando server web de prueba (cargo build -p ank-server)...")
    subprocess.run(
        [cargo_cmd, "build", "-p", "ank-server"],
        cwd=str(repo_root),
        check=True,
    )
    binary = select_ank_server_binary(repo_root)
    if binary is None:
        raise RuntimeError("La compilacion termino pero no se encontro el binario ank-server.")
    return binary


def exec_server_foreground(binary_path: Path, env_path: Path, port: int) -> int:
    """Runs `ank-server` inside the dedicated server console window."""
    if os.name == "nt":
        try:
            ctypes.windll.kernel32.SetConsoleTitleW(
                f"Aegis Test Web Server (http://localhost:{port})"
            )
        except Exception:
            pass

    child_env = os.environ.copy()
    child_env["AEGIS_ENV_FILE"] = str(env_path)
    print(f"[INFO] Levantando Aegis Test Web Server en http://localhost:{port} ...")
    print(f"[INFO] Binario: {binary_path}")
    print(f"[INFO] Env:     {env_path}\n")

    result = subprocess.run([str(binary_path)], cwd=str(REPO_ROOT), env=child_env)
    if result.returncode != 0:
        print(f"\n[ERROR] El server web termino con codigo {result.returncode}.")
        if sys.stdin and sys.stdin.isatty():
            input("Presiona Enter para cerrar esta ventana...")
    return result.returncode


def launch_test_web_server(repo_root: Path = REPO_ROOT) -> int:
    """Ensures UI + binary + isolated env are ready and starts `ank-server` on port 8090."""
    env_path = resolve_sandbox_env(
        repo_root=repo_root,
        explicit_env=os.environ.get("AEGIS_SANDBOX_ENV"),
    )
    port = resolve_http_port(env_path)

    if is_port_listening(port):
        print(f"[OK] El server web de prueba ya esta activo en http://localhost:{port}")
        return 0

    ensure_ui_dist(repo_root)
    binary_path = ensure_server_binary(repo_root)

    creationflags = (
        subprocess.CREATE_NEW_CONSOLE | subprocess.CREATE_NEW_PROCESS_GROUP
        if os.name == "nt"
        else 0
    )
    cmd = [
        sys.executable,
        str(Path(__file__).resolve()),
        "--exec-server",
        "--binary",
        str(binary_path),
        "--env-file",
        str(env_path),
        "--port",
        str(port),
    ]
    subprocess.Popen(
        cmd,
        cwd=str(repo_root),
        creationflags=creationflags,
        close_fds=True,
    )

    if wait_for_server_ready(port, timeout_secs=20.0):
        print(f"[OK] Server web de prueba listo en http://localhost:{port}")
    else:
        print(
            f"[WARN] El server web se inicio en otra ventana; verifica http://localhost:{port} en unos segundos."
        )
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Aegis sandbox web server launcher")
    parser.add_argument("--exec-server", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("--binary", help=argparse.SUPPRESS)
    parser.add_argument("--env-file", help=argparse.SUPPRESS)
    parser.add_argument("--port", type=int, default=DEFAULT_HTTP_PORT, help=argparse.SUPPRESS)
    args, _unknown = parser.parse_known_args()

    if args.exec_server:
        if not args.binary or not args.env_file:
            return 2
        return exec_server_foreground(Path(args.binary), Path(args.env_file), args.port)

    return launch_test_web_server(REPO_ROOT)


if __name__ == "__main__":
    raise SystemExit(main())

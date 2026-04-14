#!/usr/bin/env python3
"""
Aegis OS — Local Deploy Script
================================
Compila el kernel y la UI localmente y despliega directo al servidor
via SCP/SSH. No requiere GitHub ni CI/CD.

Uso:
    python deploy.py                        # build completo + deploy
    python deploy.py --kernel-only          # solo binario (más rápido)
    python deploy.py --ui-only              # solo UI dist
    python deploy.py --no-build             # sube lo ya compilado
    python deploy.py --host 192.168.1.10    # servidor distinto al default

Requisitos:
    - SSH key configurada para el servidor
    - Rust toolchain instalado localmente
    - Node.js instalado localmente
    - rsync o scp disponible
"""

import argparse
import os
import subprocess
import sys
import time
from pathlib import Path

# ── Configuración default ─────────────────────────────────────────────────────
DEFAULT_HOST     = "192.168.1.6"
DEFAULT_USER     = "tavo"
DEFAULT_SSH_KEY  = str(Path.home() / ".ssh" / "id_ed25519")
REMOTE_BIN       = "/usr/local/bin/ank-server"
REMOTE_UI        = "/usr/share/aegis/ui"
REMOTE_SERVICE   = "aegis"

# ── Colores ANSI ──────────────────────────────────────────────────────────────
class C:
    CYAN   = "\033[36m"
    GREEN  = "\033[32m"
    YELLOW = "\033[33m"
    RED    = "\033[31m"
    BOLD   = "\033[1m"
    RESET  = "\033[0m"

def log(msg: str)     : print(f"{C.CYAN}  →{C.RESET} {msg}")
def ok(msg: str)      : print(f"{C.GREEN}  ✓{C.RESET} {msg}")
def warn(msg: str)    : print(f"{C.YELLOW}  ⚠{C.RESET} {msg}")
def error(msg: str)   : print(f"{C.RED}  ✗{C.RESET} {msg}", file=sys.stderr)
def header(msg: str)  : print(f"\n{C.BOLD}{C.CYAN}{msg}{C.RESET}")
def die(msg: str)     : error(msg); sys.exit(1)

# ── Helpers ───────────────────────────────────────────────────────────────────
def run(cmd: list[str], cwd: Path | None = None, env: dict | None = None) -> None:
    """Ejecuta un comando y falla si retorna error."""
    full_env = {**os.environ, **(env or {})}
    result = subprocess.run(cmd, cwd=cwd, env=full_env)
    if result.returncode != 0:
        die(f"Falló: {' '.join(cmd)}")

def ssh(host: str, user: str, key: str, cmd: str) -> subprocess.CompletedProcess:
    """Ejecuta un comando remoto via SSH."""
    return subprocess.run(
        ["ssh", "-i", key, "-o", "StrictHostKeyChecking=no",
         f"{user}@{host}", cmd],
        capture_output=True, text=True
    )

def scp(host: str, user: str, key: str, local: str, remote: str) -> None:
    """Copia un archivo local al servidor via SCP."""
    result = subprocess.run(
        ["scp", "-i", key, "-o", "StrictHostKeyChecking=no",
         "-r", local, f"{user}@{host}:{remote}"],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        die(f"SCP falló: {result.stderr}")

def rsync(host: str, user: str, key: str, local: str, remote: str) -> None:
    """Sincroniza un directorio via rsync (más eficiente que scp para la UI)."""
    result = subprocess.run(
        ["rsync", "-az", "--delete",
         "-e", f"ssh -i {key} -o StrictHostKeyChecking=no",
         local + "/", f"{user}@{host}:{remote}/"],
        capture_output=True, text=True
    )
    if result.returncode != 0:
        die(f"rsync falló: {result.stderr}")

def find_project_root() -> Path:
    """Busca el root del proyecto (donde está Cargo.toml)."""
    current = Path(__file__).parent
    for path in [current, *current.parents]:
        if (path / "Cargo.toml").exists() and (path / "shell").exists():
            return path
    die("No se encontró el root del proyecto (Cargo.toml + shell/)")

# ── Pasos de build ────────────────────────────────────────────────────────────
def build_ui(root: Path) -> Path:
    """Compila la UI React. Retorna el path al dist/."""
    header("Compilando UI (React)...")
    ui_dir = root / "shell" / "ui"
    if not ui_dir.exists():
        die(f"No se encontró shell/ui en {ui_dir}")

    log("npm ci...")
    run(["npm", "ci"], cwd=ui_dir)

    log("npm run build...")
    run(["npm", "run", "build"], cwd=ui_dir)

    dist = ui_dir / "dist"
    if not dist.exists():
        die(f"Build de UI falló: no se creó {dist}")

    ok(f"UI compilada → {dist}")
    return dist

def build_kernel(root: Path) -> Path:
    """Compila el kernel Rust. Retorna el path al binario."""
    header("Compilando Kernel (Rust)...")
    log("cargo build --release -p ank-server ...")

    run(
        ["cargo", "build", "--release", "-p", "ank-server"],
        cwd=root
    )

    binary = root / "target" / "release" / "ank-server"
    if not binary.exists():
        die(f"Build del kernel falló: no se encontró {binary}")

    size_mb = binary.stat().st_size / (1024 * 1024)
    ok(f"Binario compilado → {binary} ({size_mb:.1f} MB)")
    return binary

# ── Pasos de deploy ───────────────────────────────────────────────────────────
def check_connection(host: str, user: str, key: str) -> None:
    """Verifica conectividad SSH con el servidor."""
    log(f"Verificando conexión SSH a {user}@{host}...")
    result = ssh(host, user, key, "echo OK")
    if result.returncode != 0 or "OK" not in result.stdout:
        die(f"No se pudo conectar a {user}@{host}. Verificá la SSH key y el host.")
    ok("Conexión SSH OK")

def stop_service(host: str, user: str, key: str) -> None:
    """Detiene el servicio Aegis en el servidor."""
    log(f"Deteniendo servicio {REMOTE_SERVICE}...")
    result = ssh(host, user, key, f"sudo systemctl stop {REMOTE_SERVICE} 2>/dev/null; echo stopped")
    if "stopped" in result.stdout:
        ok("Servicio detenido")
    else:
        warn("No se pudo detener el servicio (puede que no esté corriendo)")

def deploy_binary(host: str, user: str, key: str, binary: Path) -> None:
    """Sube el binario al servidor."""
    log(f"Subiendo binario ({binary.stat().st_size / (1024*1024):.1f} MB)...")
    t = time.time()

    # Subir a /tmp primero para evitar conflictos con el binario en uso
    scp(host, user, key, str(binary), "/tmp/ank-server-new")

    # Mover a la ubicación final con sudo
    result = ssh(host, user, key,
        f"sudo mv /tmp/ank-server-new {REMOTE_BIN} && "
        f"sudo chmod +x {REMOTE_BIN} && "
        f"sudo chown aegis:aegis {REMOTE_BIN} 2>/dev/null || sudo chown root:root {REMOTE_BIN}"
    )
    if result.returncode != 0:
        die(f"Error al instalar binario: {result.stderr}")

    elapsed = time.time() - t
    ok(f"Binario desplegado en {elapsed:.1f}s")

def deploy_ui(host: str, user: str, key: str, dist: Path) -> None:
    """Sube la UI dist al servidor."""
    log(f"Subiendo UI ({dist})...")
    t = time.time()

    # Verificar si rsync está disponible
    has_rsync = subprocess.run(["which", "rsync"], capture_output=True).returncode == 0

    # Crear directorio destino si no existe
    ssh(host, user, key, f"sudo mkdir -p {REMOTE_UI} && sudo chmod 755 {REMOTE_UI}")

    if has_rsync:
        # rsync es más eficiente — solo sube los archivos cambiados
        result = subprocess.run(
            ["rsync", "-az", "--delete",
             "-e", f"ssh -i {key} -o StrictHostKeyChecking=no",
             str(dist) + "/", f"{user}@{host}:/tmp/aegis-ui-dist/"],
            capture_output=True, text=True
        )
        if result.returncode != 0:
            die(f"rsync falló: {result.stderr}")
        ssh(host, user, key,
            f"sudo rsync -a --delete /tmp/aegis-ui-dist/ {REMOTE_UI}/ && "
            f"sudo chown -R aegis:aegis {REMOTE_UI} 2>/dev/null || true && "
            f"rm -rf /tmp/aegis-ui-dist"
        )
    else:
        # Fallback a scp
        scp(host, user, key, str(dist), f"/tmp/aegis-ui-dist")
        ssh(host, user, key,
            f"sudo rm -rf {REMOTE_UI}/* && "
            f"sudo cp -r /tmp/aegis-ui-dist/* {REMOTE_UI}/ && "
            f"sudo chown -R aegis:aegis {REMOTE_UI} 2>/dev/null || true && "
            f"rm -rf /tmp/aegis-ui-dist"
        )

    elapsed = time.time() - t
    ok(f"UI desplegada en {elapsed:.1f}s")

def start_service(host: str, user: str, key: str) -> None:
    """Inicia el servicio Aegis y verifica que arranque."""
    log("Iniciando servicio...")
    result = ssh(host, user, key, f"sudo systemctl start {REMOTE_SERVICE}")
    if result.returncode != 0:
        die(f"Error al iniciar servicio: {result.stderr}")

    log("Esperando health check (15s)...")
    time.sleep(5)

    for attempt in range(10):
        result = ssh(host, user, key,
            "curl -s http://localhost:8000/health 2>/dev/null | grep -q Online && echo UP || echo DOWN"
        )
        if "UP" in result.stdout:
            ok("Servicio UP — health check OK")
            return
        time.sleep(1)

    # No es un error fatal — el servicio puede tardar más
    warn("Health check tardó más de lo esperado. Verificá con: sudo systemctl status aegis")

def show_token(host: str, user: str, key: str) -> None:
    """Muestra el setup token si el sistema está en STATE_INITIALIZING."""
    result = ssh(host, user, key,
        "curl -s http://localhost:8000/api/system/state 2>/dev/null"
    )
    if "STATE_INITIALIZING" in result.stdout:
        token_result = ssh(host, user, key,
            "sudo journalctl -u aegis -n 50 --no-pager 2>/dev/null | grep -oP '(?<=setup_token=)\\S+' | tail -1"
        )
        token = token_result.stdout.strip()
        ip_result = ssh(host, user, key, "hostname -I | awk '{print $1}'")
        ip = ip_result.stdout.strip()
        if token:
            print(f"\n  {C.CYAN}Setup URL:{C.RESET}")
            print(f"  {C.GREEN}http://{ip}:8000?setup_token={token}{C.RESET}\n")

# ── Main ──────────────────────────────────────────────────────────────────────
def main():
    parser = argparse.ArgumentParser(
        description="Aegis OS — Deploy local → servidor sin pasar por GitHub"
    )
    parser.add_argument("--host",         default=DEFAULT_HOST,    help=f"Servidor (default: {DEFAULT_HOST})")
    parser.add_argument("--user",         default=DEFAULT_USER,    help=f"Usuario SSH (default: {DEFAULT_USER})")
    parser.add_argument("--key",          default=DEFAULT_SSH_KEY, help="Path a la SSH key")
    parser.add_argument("--kernel-only",  action="store_true",     help="Solo compilar y desplegar el binario")
    parser.add_argument("--ui-only",      action="store_true",     help="Solo compilar y desplegar la UI")
    parser.add_argument("--no-build",     action="store_true",     help="Desplegar sin recompilar (usa binario existente)")
    args = parser.parse_args()

    root = find_project_root()

    print(f"\n{C.BOLD}{C.CYAN}{'═' * 50}")
    print(f"  AEGIS OS — LOCAL DEPLOY")
    print(f"{'═' * 50}{C.RESET}")
    print(f"  Proyecto : {root}")
    print(f"  Servidor : {args.user}@{args.host}")
    print(f"  SSH Key  : {args.key}")
    print()

    # 1. Verificar conexión
    check_connection(args.host, args.user, args.key)

    # 2. Build
    binary = None
    dist   = None

    if not args.no_build:
        if not args.ui_only:
            binary = build_kernel(root)
        if not args.kernel_only:
            dist = build_ui(root)
    else:
        warn("--no-build: usando artefactos existentes")
        if not args.ui_only:
            binary = root / "target" / "release" / "ank-server"
            if not binary.exists():
                die(f"Binario no encontrado en {binary}. Compilá primero.")
        if not args.kernel_only:
            dist = root / "shell" / "ui" / "dist"
            if not dist.exists():
                die(f"UI dist no encontrada en {dist}. Compilá primero.")

    # 3. Deploy
    header("Desplegando en el servidor...")
    stop_service(args.host, args.user, args.key)

    if binary:
        deploy_binary(args.host, args.user, args.key, binary)
    if dist:
        deploy_ui(args.host, args.user, args.key, dist)

    start_service(args.host, args.user, args.key)
    show_token(args.host, args.user, args.key)

    print(f"\n{C.BOLD}{C.GREEN}  Deploy completado exitosamente.{C.RESET}\n")

if __name__ == "__main__":
    main()

# Aegis CLI — Command Reference

The `aegis` command is the primary management interface for a running Aegis OS installation.

> **Platform availability:** The `aegis` CLI is a Bash script available on **Linux and macOS**.
> On **Windows**, Aegis runs as a Windows Service managed via PowerShell — see the [Windows section](#windows-powershell-equivalents) below.

---

## Installation

### Linux / macOS

The CLI is installed automatically at `/usr/local/bin/aegis` when you run `install.sh`.

```bash
curl -fsSL https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.sh | sudo bash
```

### Windows

On Windows, Aegis is installed as a Windows Service via `install.ps1`. There is no separate CLI binary — use PowerShell to manage the service directly.

```powershell
# Run as Administrator
irm https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.ps1 | iex
```

---

## Status & Diagnostics

### `aegis status`
Check service health and API connectivity.

```bash
sudo aegis status
```

Output includes: service state, PID, uptime, HTTP health endpoint response, and remote access URL (Cloudflare Tunnel if active).

---

### `aegis version`
Display the currently installed version of `ank-server`.

```bash
aegis version
```

---

### `aegis logs [N]`
Follow the live log stream. Defaults to the last 100 lines.

```bash
sudo aegis logs          # last 100 lines, then follow
sudo aegis logs 200      # last 200 lines, then follow
```

Equivalent to `journalctl -u aegis -n N -f` (native) or `docker compose logs -f` (Docker).

---

### `aegis diag`
Generate a deep SRE diagnostic report. Collects system info, service state,
recent errors, port status, and environment configuration.

```bash
sudo aegis diag
```

Useful for filing bug reports — the output gives a full snapshot of the installation state.

---

## Service Control

### `aegis start`
Start the Aegis service.

```bash
sudo aegis start
```

---

### `aegis stop`
Stop the Aegis service.

```bash
sudo aegis stop
```

---

### `aegis restart`
Restart the Aegis service. Use after configuration changes.

```bash
sudo aegis restart
```

---

### `aegis token`
Print the local setup URL with a fresh authentication token. Use this to access
the Aegis web interface from within your local network.

```bash
sudo aegis token
```

Output:
```
Setup URL: http://192.168.1.x:8000?setup_token=<token>
Token expires in 30 minutes.
```

---

### `aegis tunnel`
Manually start a Cloudflare Tunnel for remote HTTPS access. Normally this starts
automatically at boot — use this command only if the tunnel is down.

```bash
sudo aegis tunnel
```

---

## Updates

### `aegis update`
Update to the latest **nightly** build. Downloads the new binary, replaces the
existing one, updates UI assets and agent instruction files, and restarts the service.

```bash
sudo aegis update
```

---

### `aegis update --beta`
Alias for `--nightly`. Same behavior as `aegis update`.

```bash
sudo aegis update --beta
```

---

### `aegis update --stable`
Update to the latest **stable** release. Use this to pin back to stable after
running a nightly.

```bash
sudo aegis update --stable
```

---

### `aegis uninstall`
Remove Aegis from the system.

```bash
sudo aegis uninstall
```

Removes the binary, CLI, systemd service, and configuration files.
The data directory (`/var/lib/aegis/`) is preserved by default.

---

## Windows PowerShell Equivalents

On Windows, Aegis runs as the `AegisOS` Windows Service. Use these PowerShell
commands as equivalents to the Linux CLI:

| Linux CLI | Windows PowerShell |
|---|---|
| `sudo aegis start` | `Start-Service AegisOS` |
| `sudo aegis stop` | `Stop-Service AegisOS` |
| `sudo aegis restart` | `Restart-Service AegisOS` |
| `sudo aegis status` | `Get-Service AegisOS` |
| `sudo aegis logs` | `Get-EventLog -LogName Application -Source AegisOS -Newest 100` |
| `sudo aegis update` | Re-run `install.ps1` as Administrator |
| `aegis version` | `& "$env:ProgramFiles\Aegis\ank-server.exe" --version` |

**Update on Windows:**
```powershell
# Run as Administrator — downloads and applies the latest nightly
irm https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.ps1 | iex

# Update to a specific version
powershell -ExecutionPolicy Bypass -File install.ps1 -ReleaseTag "v1.2.3"
```

---

## Environment

Aegis reads its configuration from the environment file generated at install time.

| Variable | Description | Example |
|---|---|---|
| `AEGIS_ROOT_KEY` | Master authentication key (auto-generated at install) | `a3f8...` |
| `AEGIS_DATA_DIR` | Data directory | `/var/lib/aegis` |
| `AEGIS_AGENTS_CONFIG_DIR` | Agent instruction files directory | `/etc/aegis/agents` |
| `AEGIS_MODEL_PROFILE` | Inference profile | `cloud` / `local` / `hybrid` |
| `UI_DIST_PATH` | Path to UI assets | `/usr/share/aegis/ui` |
| `HW_PROFILE` | Hardware tier | `1` / `2` / `3` |
| `DEFAULT_MODEL_PREF` | Default model preference | `CloudOnly` / `LocalOnly` / `HybridSmart` |

**Linux/macOS path:** `/etc/aegis/aegis.env` — must be `640 root:aegis`. Never world-readable.

**Windows path:** `%ProgramData%\Aegis\aegis.env`

---

## Paths

### Linux / macOS

| Path | Description |
|---|---|
| `/usr/local/bin/ank-server` | Main binary |
| `/usr/local/bin/aegis` | CLI tool |
| `/etc/aegis/aegis.env` | Environment configuration |
| `/etc/aegis/mode` | Installation mode (`native` or `docker`) |
| `/etc/aegis/agents/` | Agent instruction files |
| `/var/lib/aegis/` | Data directory |
| `/var/lib/aegis/logs/` | Log files |
| `/usr/share/aegis/ui/` | Web UI assets (native mode) |
| `/root/aegis_install.log` | Installer log |

### Windows

| Path | Description |
|---|---|
| `%ProgramFiles%\Aegis\ank-server.exe` | Main binary |
| `%ProgramFiles%\Aegis\ui\` | Web UI assets |
| `%ProgramData%\Aegis\aegis.env` | Environment configuration |
| `%ProgramData%\Aegis\logs\` | Log files |
| `%ProgramData%\Aegis\agents\` | Agent instruction files |

---

## Troubleshooting

### Service won't start

**Linux/macOS:**
```bash
sudo aegis logs 50       # check recent log output
sudo aegis diag          # full diagnostic snapshot
journalctl -u aegis -n 100 --no-pager   # raw systemd logs
```

**Windows:**
```powershell
Get-EventLog -LogName Application -Source AegisOS -Newest 50
Get-Service AegisOS
```

### Port conflict

Aegis uses ports `8000` (HTTP/WebSocket) and `50051` (gRPC internal). Check for conflicts:

**Linux/macOS:**
```bash
ss -tulpn | grep -E '8000|50051'
```

**Windows:**
```powershell
netstat -ano | findstr "8000 50051"
```

### Token expired

**Linux/macOS:**
```bash
sudo aegis token         # regenerates a fresh token
```

**Windows:** Restart the service and check Event Viewer for the new token in the startup logs.

### Manual service management (bypass aegis CLI)

**Linux/macOS:**
```bash
sudo systemctl status aegis
sudo systemctl restart aegis
sudo journalctl -u aegis -f
```

**Windows:**
```powershell
Get-Service AegisOS
Restart-Service AegisOS
```

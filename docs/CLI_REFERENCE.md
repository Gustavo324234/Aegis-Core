# Aegis CLI — Command Reference

The `aegis` command is the primary management interface for a running Aegis OS installation.

> **Platform availability:** On **Linux and macOS** the `aegis` CLI is a Bash script.
> On **Windows** it is a PowerShell script (`aegis.ps1` + `aegis.cmd` wrapper) installed to
> `%ProgramFiles%\Aegis` and added to the system `PATH`, so `aegis <command>` works from any
> terminal. Aegis itself runs as the `AegisOS` Windows Service — native PowerShell
> equivalents are listed in the [Windows section](#windows-powershell-equivalents) below.

---

## Installation

### Linux / macOS

The CLI is installed automatically at `/usr/local/bin/aegis` when you run `install.sh`.

```bash
curl -fsSL https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.sh | sudo bash
```

### Windows

On Windows, Aegis is installed as a Windows Service via `install.ps1`, which also installs
the `aegis` CLI (`aegis.ps1` + `aegis.cmd`) into `%ProgramFiles%\Aegis` and adds it to the
system `PATH`. Open a **new** terminal after installing so the updated `PATH` is picked up.

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

### `aegis trace [OPTS]`
Follow **filtered model-routing events only** — no raw JSON walls. Useful for
watching which model handles each request in real time.

```bash
sudo aegis trace                     # follow routing events (last 500 lines)
sudo aegis trace -n 100              # start from the last 100 lines
sudo aegis trace --pid proc_xxx      # only events for a specific process
sudo aegis trace --since "5m ago"    # passthrough to journalctl --since
sudo aegis trace --no-follow         # print and exit, don't tail
```

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
Update to the latest **stable** release (the default channel). Downloads the new
binary, replaces the existing one, updates UI assets and agent instruction files,
and restarts the service.

```bash
sudo aegis update
```

---

### `aegis update --nightly`
Update to the latest **nightly** build from `main`. `--beta` is accepted as an
alias for `--nightly`.

```bash
sudo aegis update --nightly
```

---

### `aegis update --stable`
Explicitly select the stable channel. Use this to pin back to stable after
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

## Administrative Commands (via `ank-cli`)

When `ank-cli` is installed at `/usr/local/bin/ank-cli`, the `aegis` wrapper
forwards these additional commands to it:

### `aegis keygen`
Generate an Ed25519 keypair for signing Wasm plugins (Citadel Protocol).

```bash
aegis keygen                                  # writes plugin_signer.key / plugin_signer.pub
aegis keygen --secret my.key --public my.pub  # custom output paths
```

> The server also auto-generates a local keypair in `<data-dir>/keys/` on first
> boot if no `AEGIS_PLUGIN_ROOT_KEY` is configured, so manual keygen is only
> needed for custom signing workflows.

### `aegis sign <plugin.wasm>`
Sign a Wasm plugin with a private key. Produces the `.wasm.sig` file required
by the Citadel Protocol to load the plugin.

```bash
aegis sign my_plugin.wasm                     # uses ./plugin_signer.key
aegis sign my_plugin.wasm --secret my.key     # custom private key
```

### `aegis ps`
List active processes (PCBs) in the kernel.

### `aegis run <prompt>`
Send a prompt to the AI and stream the output to the terminal.

### `aegis admin create-tenant <name>`
Create a new tenant/enclave (requires Master Admin credentials).

---

## Windows PowerShell Equivalents

On Windows, Aegis runs as the `AegisOS` Windows Service and the same `aegis` commands
(`start`, `stop`, `restart`, `status`, `logs [N]`, `trace [N] [PID]`, `version`, `token`,
`diag`, `update [--nightly|--stable]`, `uninstall`) are available from any elevated
terminal. If you prefer, these native PowerShell equivalents also work:

| Aegis CLI | Windows PowerShell equivalent |
|---|---|
| `aegis start` | `Start-Service AegisOS` |
| `aegis stop` | `Stop-Service AegisOS` |
| `aegis restart` | `Restart-Service AegisOS` |
| `aegis status` | `Get-Service AegisOS` |
| `aegis logs` | `Get-EventLog -LogName Application -Source AegisOS -Newest 100` |
| `aegis update` | Re-run `install.ps1` as Administrator |
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
| `AEGIS_PLUGIN_ROOT_KEY` | Ed25519 public key (hex) that anchors plugin-signature trust. Auto-generated in `<data-dir>/keys/` on first boot if absent | `9f2c...` |
| `AEGIS_ALLOW_INSECURE_PLUGINS` | Set to `1` to load unsigned Wasm plugins (dev/CI only — disables Citadel signature checks) | `0` |
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

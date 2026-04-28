# Aegis CLI — Command Reference

The `aegis` command is the primary management interface for a running Aegis OS installation.
It is installed automatically by `installer/install.sh` at `/usr/local/bin/aegis`.

---

## Status & Diagnostics

### `aegis status`
Check service health and API connectivity.

```bash
sudo aegis status
```

Output includes: service state, PID, uptime, HTTP health endpoint response, and current inference profile.

---

### `aegis version`
Display the currently installed version of `ank-server`.

```bash
aegis version
```

---

### `aegis logs [N]`
Follow the live log stream. Defaults to the last 50 lines.

```bash
sudo aegis logs          # last 50 lines, then follow
sudo aegis logs 100      # last 100 lines, then follow
```

Equivalent to `journalctl -u aegis -n N -f`.

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

## Updates

### `aegis update`
Interactive update to the latest **stable** release. Downloads the new binary,
replaces the existing one, and restarts the service.

```bash
sudo aegis update
```

---

### `aegis update --beta`
Update to the latest **nightly** build from the `main` branch.
Use to get the newest features before a stable release.

```bash
sudo aegis update --beta
```

---

### `aegis update --stable`
Explicitly target the stable release channel. Useful if you previously installed
a nightly and want to pin back to stable.

```bash
sudo aegis update --stable
```

---

## Environment

Aegis reads its configuration from `/etc/aegis/aegis.env`.

| Variable | Description | Example |
|---|---|---|
| `AEGIS_ROOT_KEY` | Master authentication key (auto-generated at install) | `a3f8...` |
| `AEGIS_DATA_DIR` | Data directory | `/var/lib/aegis` |
| `AEGIS_MODEL_PROFILE` | Inference profile | `cloud` / `local` / `hybrid` |
| `UI_DIST_PATH` | Path to UI assets (native mode only) | `/usr/share/aegis/ui` |
| `HW_PROFILE` | Hardware tier | `1` / `2` / `3` |
| `DEFAULT_MODEL_PREF` | Default model preference | `CloudOnly` / `LocalOnly` / `HybridSmart` |

**File permissions:** `/etc/aegis/aegis.env` must be `640 root:aegis`. Never world-readable.

---

## Paths

| Path | Description |
|---|---|
| `/usr/local/bin/ank-server` | Main binary |
| `/usr/local/bin/aegis` | CLI tool |
| `/etc/aegis/aegis.env` | Environment configuration |
| `/etc/aegis/mode` | Installation mode (`native` or `docker`) |
| `/var/lib/aegis/` | Data directory |
| `/var/lib/aegis/logs/` | Log files |
| `/usr/share/aegis/ui/` | Web UI assets (native mode) |
| `/root/aegis_install.log` | Installer log |

---

## Troubleshooting

### Service won't start

```bash
sudo aegis logs 50       # check recent log output
sudo aegis diag          # full diagnostic snapshot
journalctl -u aegis -n 100 --no-pager   # raw systemd logs
```

### Port conflict

Aegis uses ports `8000` (HTTP) and `50051` (gRPC). Check for conflicts:

```bash
ss -tulpn | grep -E '8000|50051'
```

### Token expired

```bash
sudo aegis token         # regenerates a fresh token
```

### Manual service management (bypass aegis CLI)

```bash
sudo systemctl status aegis
sudo systemctl restart aegis
sudo journalctl -u aegis -f
```

---

## Uninstall

```bash
sudo bash /path/to/Aegis-Core/installer/uninstall.sh
```

This removes the binary, CLI, systemd service, and configuration files.
The data directory (`/var/lib/aegis/`) is preserved by default.

# installer/

Aegis deployment tooling — Bash / Docker / systemd

Handles installation, updates, and operation of Aegis on Linux servers and
native Windows/macOS systems.

## Scripts

| Script | Purpose |
|---|---|
| `install.sh` | One-line installer (native + Docker modes) |
| `setup-service.sh` | Native mode: configures systemd and environment |
| `docker-compose.yml` | Docker deployment |
| `aegis` | CLI tool for management |
| `uninstall.sh` | Clean uninstall |
| `aegis.service` | Systemd service template |

## Aegis CLI Usage

The `aegis` command is the primary control interface for the system.

### Core Commands
- `aegis status`: Check service health and API connectivity.
- `aegis version`: Display the currently installed core version.
- `aegis logs [N]`: Follow the last N lines of system logs.
- `aegis diag`: Generate a deep SRE diagnostic report.

### Lifecycle Management
- `aegis start` / `stop` / `restart`: Manage the underlying service.
- `aegis token`: Retrieve the initial setup URL and token.

### Updates
- `aegis update`: Interactive update to the latest **stable** release.
- `aegis update --beta`: Update to the absolute latest **nightly** build (from `main`).
- `aegis update --stable`: Explicitly target the stable channel.


## Source

Migrated from: `Aegis-Installer` (archived)

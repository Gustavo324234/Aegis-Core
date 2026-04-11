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

## Source

Migrated from: `Aegis-Installer` (archived)

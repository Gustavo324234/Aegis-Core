# installer/

Aegis deployment tooling — Bash / Docker / systemd

Handles installation, updates, and operation of Aegis on Linux servers and
native Windows/macOS systems.

## Scripts

| Script | Purpose |
|---|---|
| `install.sh` | One-line installer (native + Docker modes) |
| `aegis-native-install.sh` | Native mode: downloads binaries, configures systemd |
| `docker-compose.yml` | Docker deployment |
| `aegis_hotreload.sh` | Hot-reload for development on remote servers |
| `aegis_sync.ps1` | Windows → server rsync + SSH |
| `uninstall.sh` | Clean uninstall |

## Source

Migrated from: `Aegis-Installer` (archived)

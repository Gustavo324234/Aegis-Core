# CORE-043 — installer: aegis CLI — start/stop/status/logs/update

**Épica:** 32 — Unified Binary
**Fase:** 5 — Installer
**Repo:** Aegis-Core — `installer/`
**Asignado a:** DevOps Engineer
**Prioridad:** 🟡 Media
**Estado:** DONE
**Depende de:** CORE-042

---

## Contexto

CLI unificada para operar Aegis en producción. Detecta automáticamente
si el sistema está en modo Docker o nativo y aplica el comando correcto.

**Referencia:** `Aegis-Installer/aegis_cli.sh`

---

## Trabajo requerido

### `installer/aegis` (script bash instalado en `/usr/local/bin/aegis`)

```bash
#!/usr/bin/env bash
set -euo pipefail

AEGIS_MODE="${AEGIS_MODE:-native}"   # 'native' o 'docker'
INSTALL_DIR="${INSTALL_DIR:-/opt/aegis}"

cmd_start()  { ... }
cmd_stop()   { ... }
cmd_status() { ... }
cmd_logs()   { ... }
cmd_update() { ... }
cmd_token()  { ... }  # regenerar setup_token

case "${1:-help}" in
  start)   cmd_start ;;
  stop)    cmd_stop ;;
  restart) cmd_stop; cmd_start ;;
  status)  cmd_status ;;
  logs)    cmd_logs "${2:-}" ;;
  update)  cmd_update ;;
  token)   cmd_token ;;
  *)       echo "Usage: aegis {start|stop|restart|status|logs|update|token}" ;;
esac
```

**Modo nativo:**
- `start` → `systemctl start aegis`
- `stop` → `systemctl stop aegis`
- `status` → `systemctl status aegis` + `curl /health`
- `logs` → `journalctl -u aegis -f`
- `update` → descargar nuevo binario desde GitHub Releases + `systemctl restart aegis`
- `token` → extraer setup_token de los logs del servicio

**Modo Docker:**
- `start` → `docker compose up -d`
- `stop` → `docker compose down`
- `status` → `docker compose ps`
- `logs` → `docker compose logs -f`
- `update` → `docker compose pull && docker compose up -d`

---

## Criterios de aceptación

- [ ] `aegis start` funciona en modo nativo y Docker
- [ ] `aegis status` muestra el estado correctamente en ambos modos
- [ ] `aegis logs` muestra logs en tiempo real
- [ ] `aegis update` descarga el nuevo binario/imagen y reinicia el servicio
- [ ] `aegis token` muestra el setup_token actual (o genera uno nuevo)
- [ ] `shellcheck installer/aegis` → 0 warnings

## Referencia

`Aegis-Installer/aegis_cli.sh` — adaptar para un solo proceso
`Aegis-Installer/aegis_token.sh` — incorporar como subcomando `token`

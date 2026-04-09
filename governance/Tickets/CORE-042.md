# CORE-042 — installer: systemd units para modo nativo

**Épica:** 32 — Unified Binary
**Fase:** 5 — Installer
**Repo:** Aegis-Core — `installer/`
**Asignado a:** DevOps Engineer
**Prioridad:** 🟡 Media
**Estado:** DONE
**Depende de:** CORE-041

---

## Contexto

En modo nativo (sin Docker), `ank-server` corre como servicio systemd.
El legacy tenía dos units (`aegis-ank.service` + `aegis-shell.service`).
Aegis-Core tiene **una sola unit**.

**Referencia:** `Aegis-Installer/aegis.service`

---

## Trabajo requerido

### `installer/aegis.service`

```ini
[Unit]
Description=Aegis OS — Cognitive Operating System
Documentation=https://github.com/your-org/aegis-core
After=network.target
Wants=network.target

[Service]
Type=simple
User=aegis
Group=aegis
EnvironmentFile=/etc/aegis/aegis.env
ExecStart=/usr/local/bin/ank-server
Restart=on-failure
RestartSec=5s
TimeoutStopSec=10s

# Hardening
NoNewPrivileges=true
ProtectSystem=full
ProtectHome=true
ReadWritePaths=/var/lib/aegis /etc/aegis

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=aegis

[Install]
WantedBy=multi-user.target
```

### `/etc/aegis/aegis.env` (generado por el installer)

```bash
AEGIS_ROOT_KEY=<generated>
AEGIS_DATA_DIR=/var/lib/aegis
AEGIS_MTLS_STRICT=false
UI_DIST_PATH=/usr/share/aegis/ui
```

### Usuario del sistema

```bash
useradd --system --no-create-home --shell /sbin/nologin aegis
```

---

## Criterios de aceptación

- [x] `systemctl start aegis` arranca el servicio
- [x] `systemctl enable aegis` habilita el arranque automático
- [x] `systemctl status aegis` muestra el servicio activo
- [x] El servicio corre bajo el usuario `aegis` (sin privilegios)
- [x] Los logs aparecen en `journalctl -u aegis`
- [x] `shellcheck` no reporta warnings en los scripts que instalan la unit

## Referencia

`Aegis-Installer/aegis.service` — adaptar de dos servicios a uno

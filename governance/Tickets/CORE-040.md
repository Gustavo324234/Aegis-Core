# CORE-040 — installer: install.sh unificado

**Épica:** 32 — Unified Binary
**Fase:** 5 — Installer
**Repo:** Aegis-Core — `installer/`
**Asignado a:** DevOps Engineer
**Prioridad:** 🔴 Alta
**Estado:** DONE
**Depende de:** CORE-020, CORE-036

---

## Contexto

El installer de Aegis-Core instala un único binario (`ank-server`) en lugar de
dos contenedores/procesos. La experiencia de usuario es idéntica al legacy,
el cambio es transparente para quien instala.

**Referencia:** `Aegis-Installer/install_aegis.sh` + `Aegis-Installer/aegis-native-install.sh`

---

## Trabajo requerido

### `installer/install.sh`

Script principal con dos modos (menú TUI igual al legacy):

```
┌─────────────────────────────────────────────────┐
│           AEGIS OS — INSTALLATION MODE          │
├─────────────────────────────────────────────────┤
│  [1] Native (recommended)                       │
│      Single binary, no Docker required          │
│  [2] Docker                                     │
│      Containerized deployment                   │
└─────────────────────────────────────────────────┘
```

**Modo nativo:**
1. Detectar OS y arquitectura (Linux x86_64, ARM64)
2. Descargar `ank-server` desde GitHub Releases (tag: latest)
3. Descargar `shell/ui/dist/` desde GitHub Releases
4. Generar `AEGIS_ROOT_KEY` con `openssl rand -hex 32`
5. Escribir `/etc/aegis/aegis.env` con `chmod 600`
6. Instalar `aegis-supervisor` como servicio systemd
7. Iniciar el servicio
8. Mostrar URL de acceso + esperar que aparezca el setup_token en logs

**Modo Docker:**
1. Verificar Docker + Compose V2 instalados
2. Descargar `installer/docker-compose.yml`
3. Generar `.env` con `AEGIS_ROOT_KEY`
4. `docker compose up -d`

### Diferencia clave vs. legacy

El `docker-compose.yml` de Aegis-Core tiene **un solo servicio** (`ank-server`)
en lugar de dos (`aegis-ank` + `aegis-shell`).

---

## Criterios de aceptación

- [ ] `bash install.sh` muestra el menú y acepta selección `1` o `2`
- [ ] Modo nativo descarga el binario correcto para la arquitectura detectada
- [ ] `AEGIS_ROOT_KEY` se genera y persiste en `/etc/aegis/aegis.env`
- [ ] El servicio systemd arranca automáticamente
- [ ] `curl http://localhost:8000/health` retorna 200 tras la instalación
- [ ] `shellcheck install.sh` → 0 warnings
- [ ] `set -euo pipefail` presente en todas las funciones

## Referencia

`Aegis-Installer/install_aegis.sh` — flujo de instalación a adaptar
`Aegis-Installer/aegis-native-install.sh` — instalación nativa a simplificar

# installer/

Aegis deployment tooling — multiplataforma.

Gestiona la instalación, actualizaciones y operación de Aegis en Linux, macOS y Windows.

## Scripts

| Script | Plataforma | Propósito |
|---|---|---|
| `install.sh` | Linux / macOS | Installer one-line (modos nativo + Docker) |
| `install.ps1` | Windows | Installer PowerShell (instala como Servicio de Windows) |
| `setup-service.sh` | Linux | Configura systemd y entorno (modo nativo) |
| `aegis` | Linux / macOS | CLI de gestión (`start`, `stop`, `update`, etc.) |
| `docker-compose.yml` | Linux / macOS | Deployment Docker |
| `uninstall.sh` | Linux / macOS | Desinstalación limpia |
| `aegis.service` | Linux | Template del servicio systemd |

## Plataformas soportadas

CI genera binarios pre-compilados para las siguientes plataformas en cada commit y release:

| Plataforma | Arquitectura | Binario publicado |
|---|---|---|
| Linux | x86_64 | `ank-server-linux-x86_64.tar.gz` |
| Linux | ARM64 | `ank-server-linux-arm64.tar.gz` |
| macOS | Apple Silicon (ARM64) | `ank-server-macos-arm64.zip` |
| macOS | Intel (x86_64) | `ank-server-macos-x86_64.zip` |
| Windows | x86_64 | `ank-server-windows-x86_64.zip` |

## Instalación rápida

### Linux / macOS
```bash
curl -fsSL https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.sh | sudo bash
```

### Windows (PowerShell como Administrador)
```powershell
irm https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.ps1 | iex
```

## Aegis CLI (Linux / macOS)

El comando `aegis` se instala en `/usr/local/bin/aegis` al ejecutar `install.sh`.

### Comandos principales

```bash
aegis status          # Salud del servicio y conectividad API
aegis version         # Versión instalada
aegis logs [N]        # Seguir logs en vivo (default 100 líneas)
aegis diag            # Reporte diagnóstico SRE completo
```

### Control del servicio

```bash
aegis start / stop / restart
aegis token           # Obtener URL de setup con token fresco
aegis tunnel          # Iniciar túnel Cloudflare manualmente
```

### Actualizaciones

```bash
aegis update          # Actualizar al último build nightly
aegis update --stable # Actualizar al último release estable
```

## Gestión en Windows (PowerShell)

En Windows no hay CLI dedicado. Usá PowerShell para gestionar el servicio `AegisOS`:

```powershell
Start-Service AegisOS
Stop-Service AegisOS
Restart-Service AegisOS
Get-Service AegisOS
Get-EventLog -LogName Application -Source AegisOS -Newest 50

# Actualizar: re-ejecutar el installer
irm https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.ps1 | iex
```

Ver [docs/CLI_REFERENCE.md](../docs/CLI_REFERENCE.md) para referencia completa y tabla de equivalencias.

## Source

Migrado de: `Aegis-Installer` (archivado)

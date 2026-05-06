# ==============================================================================
# AEGIS OS — Windows Installer (PowerShell)
# ==============================================================================
# Requiere: PowerShell 5.1+ o PowerShell Core 7+
# Ejecutar como Administrador:
#   irm https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.ps1 | iex
# O localmente:
#   powershell -ExecutionPolicy Bypass -File install.ps1
# ==============================================================================

#Requires -RunAsAdministrator

param(
    [string]$DataDir     = "$env:ProgramData\Aegis",
    [string]$InstallDir  = "$env:ProgramFiles\Aegis",
    [string]$ReleaseTag  = "nightly",
    [switch]$NoService,
    [switch]$Silent
)

$ErrorActionPreference = "Stop"

$GITHUB_ORG   = "Gustavo324234"
$GITHUB_REPO  = "Aegis-Core"
$GITHUB_RAW   = "https://raw.githubusercontent.com/$GITHUB_ORG/$GITHUB_REPO/main"
$RELEASE_URL  = "https://github.com/$GITHUB_ORG/$GITHUB_REPO/releases/download/$ReleaseTag"
$BIN_NAME     = "ank-server.exe"
$CLI_NAME     = "aegis.ps1"
$SERVICE_NAME = "AegisOS"

$SID_ADMINS = New-Object System.Security.Principal.SecurityIdentifier("S-1-5-32-544")
$SID_SYSTEM = New-Object System.Security.Principal.SecurityIdentifier("S-1-5-18")

# --- Colores ---
function Write-Step { param($msg) Write-Host "  -> $msg" -ForegroundColor Cyan }
function Write-OK   { param($msg) Write-Host "  [OK] $msg" -ForegroundColor Green }
function Write-Warn { param($msg) Write-Host "  [!] $msg" -ForegroundColor Yellow }
function Write-Fail { param($msg) Write-Host "  [ERROR] $msg" -ForegroundColor Red; exit 1 }

function Show-Banner {
    Write-Host ""
    Write-Host "    ___  _____  _____ _____  _____ " -ForegroundColor Cyan
    Write-Host "   / _ \|  ___||  __ \_ _/  / ____|" -ForegroundColor Cyan
    Write-Host "  / /_\ \ |__  | |  \/ |   | (___  " -ForegroundColor Cyan
    Write-Host "  |  _  |  __| | | __| |    \___ \ " -ForegroundColor Cyan
    Write-Host "  | | | | |___ | |_\ \ |_   ____) |" -ForegroundColor Cyan
    Write-Host "  \_| |_|_____/ \____/___/ |_____/ " -ForegroundColor Cyan
    Write-Host ""
    Write-Host "      Aegis OS — Windows Installer" -ForegroundColor White
    Write-Host "  ----------------------------------------" -ForegroundColor DarkGray
    Write-Host ""
}

function Set-AegisAcl {
    param([string]$Path, [bool]$IsDirectory = $true)

    $inherit = if ($IsDirectory) {
        [System.Security.AccessControl.InheritanceFlags]"ContainerInherit,ObjectInherit"
    } else {
        [System.Security.AccessControl.InheritanceFlags]::None
    }
    $propagation = [System.Security.AccessControl.PropagationFlags]::None
    $rights      = [System.Security.AccessControl.FileSystemRights]::FullControl
    $type        = [System.Security.AccessControl.AccessControlType]::Allow

    $acl = Get-Acl $Path
    $acl.SetAccessRuleProtection($true, $false)
    $acl.AddAccessRule((New-Object System.Security.AccessControl.FileSystemAccessRule($SID_ADMINS, $rights, $inherit, $propagation, $type)))
    $acl.AddAccessRule((New-Object System.Security.AccessControl.FileSystemAccessRule($SID_SYSTEM, $rights, $inherit, $propagation, $type)))
    Set-Acl $Path $acl
}

function Test-Prerequisites {
    Write-Step "Verificando prerequisitos..."

    if ($PSVersionTable.PSVersion.Major -lt 5) {
        Write-Fail "Se requiere PowerShell 5.1 o superior."
    }

    $os = [System.Environment]::OSVersion.Version
    if ($os.Major -lt 10) {
        Write-Fail "Se requiere Windows 10 / Server 2016 o superior."
    }

    $vcRedist = Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" -ErrorAction SilentlyContinue
    if (-not $vcRedist) {
        Write-Warn "Visual C++ Redistributable 2015-2022 no detectado."
        Write-Warn "Si el servicio no arranca, descargalo de: https://aka.ms/vs/17/release/vc_redist.x64.exe"
    }

    Write-OK "Prerequisitos verificados."
}

function New-AegisDirs {
    Write-Step "Creando directorios..."

    $dirs = @(
        $InstallDir,
        $DataDir,
        "$DataDir\logs",
        "$DataDir\plugins",
        "$DataDir\users",
        "$DataDir\agents"
    )

    foreach ($dir in $dirs) {
        if (-not (Test-Path $dir)) {
            New-Item -ItemType Directory -Path $dir -Force | Out-Null
        }
    }

    Set-AegisAcl -Path $DataDir -IsDirectory $true

    Write-OK "Directorios creados."
}

function Get-AegisBinaries {
    Write-Step "Descargando ank-server (Windows x86_64)..."

    $zipPath = "$env:TEMP\ank-server-windows-x86_64.zip"
    $zipUrl  = "$RELEASE_URL/ank-server-windows-x86_64.zip"

    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $zipUrl -OutFile $zipPath -UseBasicParsing
    } catch {
        Write-Fail "No se pudo descargar el binario desde:`n  $zipUrl`nError: $_"
    }

    Expand-Archive -Path $zipPath -DestinationPath $env:TEMP -Force
    Move-Item "$env:TEMP\ank-server-windows-x86_64.exe" "$InstallDir\$BIN_NAME" -Force
    Remove-Item $zipPath -Force

    Write-OK "ank-server.exe -> $InstallDir\$BIN_NAME"
}

function Get-AegisUI {
    Write-Step "Descargando assets de UI..."

    $tarPath = "$env:TEMP\ui-dist.tar.gz"
    $uiDir   = "$InstallDir\ui"

    try {
        Invoke-WebRequest -Uri "$RELEASE_URL/ui-dist.tar.gz" -OutFile $tarPath -UseBasicParsing
    } catch {
        Write-Warn "No se pudo descargar la UI. Continuando sin ella."
        return
    }

    if (-not (Test-Path $uiDir)) {
        New-Item -ItemType Directory -Path $uiDir -Force | Out-Null
    }

    tar -xzf $tarPath -C $uiDir
    Remove-Item $tarPath -Force

    Write-OK "UI assets -> $uiDir"
}

function Get-AgentsConfig {
    Write-Step "Descargando configuracion de agentes..."

    $tarPath = "$env:TEMP\agents-config.tar.gz"

    try {
        Invoke-WebRequest -Uri "$RELEASE_URL/agents-config.tar.gz" -OutFile $tarPath -UseBasicParsing
        tar -xzf $tarPath -C "$DataDir\agents"
        Remove-Item $tarPath -Force
        Write-OK "Agent config -> $DataDir\agents"
    } catch {
        Write-Warn "agents-config.tar.gz no disponible — usando fallbacks compilados."
    }
}

function New-AegisEnvFile {
    Write-Step "Generando configuracion de entorno..."

    $envPath = "$DataDir\aegis.env"

    if (Test-Path $envPath) {
        Write-Warn "Archivo de entorno existente preservado: $envPath"
        return
    }

    $bytes   = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    $rootKey = [BitConverter]::ToString($bytes).Replace("-", "").ToLower()

    $envContent = @"
AEGIS_ROOT_KEY=$rootKey
AEGIS_DATA_DIR=$DataDir
AEGIS_AGENTS_CONFIG_DIR=$DataDir\agents
UI_DIST_PATH=$InstallDir\ui
AEGIS_MODEL_PROFILE=cloud
DEFAULT_MODEL_PREF=CloudOnly
RUST_LOG=info
"@

    Set-Content -Path $envPath -Value $envContent -Encoding UTF8
    Set-AegisAcl -Path $envPath -IsDirectory $false

    Write-OK "Entorno generado: $envPath"
}

# Lee el aegis.env y escribe las vars en el registro del servicio (REG_MULTI_SZ).
# Se llama en instalacion fresca Y en actualizacion — es idempotente.
# Garantiza que el binario actual (pre-CORE-265) siempre reciba las vars correctas
# independientemente del historial de instalaciones previas.
function Write-EnvToServiceRegistry {
    param([string]$EnvPath)

    if (-not (Test-Path $EnvPath)) {
        Write-Warn "aegis.env no encontrado — registro del servicio no actualizado."
        return
    }

    $envVars = @{}
    Get-Content $EnvPath | ForEach-Object {
        if ($_ -match "^([^#=\s][^=]*)=(.*)$") {
            $envVars[$matches[1].Trim()] = $matches[2].Trim()
        }
    }

    if ($envVars.Count -eq 0) {
        Write-Warn "aegis.env no contiene variables validas."
        return
    }

    $regPath = "HKLM:\SYSTEM\CurrentControlSet\Services\$SERVICE_NAME"
    if (-not (Test-Path $regPath)) {
        Write-Warn "Registro del servicio no encontrado — omitiendo escritura de env vars."
        return
    }

    $envArray = [string[]]($envVars.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" })
    Set-ItemProperty -Path $regPath -Name "Environment" -Value $envArray -Type MultiString

    Write-OK "Variables de entorno sincronizadas en registro del servicio ($($envVars.Count) vars)."
}

function Install-AegisCLI {
    Write-Step "Instalando Aegis CLI (aegis.ps1)..."

    $cliDest = "$InstallDir\$CLI_NAME"

    # Descargar aegis.ps1 desde el repo
    try {
        Invoke-WebRequest -Uri "$GITHUB_RAW/installer/$CLI_NAME" -OutFile $cliDest -UseBasicParsing
    } catch {
        Write-Warn "No se pudo descargar aegis.ps1: $_"
        return
    }

    # Crear wrapper aegis.cmd en InstallDir para que funcione como comando sin extension
    # Esto permite escribir 'aegis status' en lugar de 'aegis.ps1 status'
    $wrapperPath = "$InstallDir\aegis.cmd"
    $wrapperContent = "@echo off`r`npowershell.exe -ExecutionPolicy Bypass -File `"$cliDest`" %*"
    [System.IO.File]::WriteAllText($wrapperPath, $wrapperContent, [System.Text.Encoding]::ASCII)

    Write-OK "CLI instalado -> $cliDest"
    Write-OK "Wrapper  -> $wrapperPath"
    Write-OK "Uso: aegis status / aegis logs / aegis update"
}

function Add-ToPath {
    Write-Step "Agregando $InstallDir al PATH del sistema..."

    $currentPath = [Environment]::GetEnvironmentVariable("Path", "Machine")
    if ($currentPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$currentPath;$InstallDir", "Machine")
        Write-OK "$InstallDir agregado al PATH."
        Write-Warn "Reinicia la terminal para usar el comando 'aegis'."
    } else {
        Write-Step "$InstallDir ya estaba en el PATH."
    }
}

function Install-AegisService {
    Write-Step "Configurando servicio de Windows..."

    $existing = Get-Service -Name $SERVICE_NAME -ErrorAction SilentlyContinue

    if ($existing) {
        # ── MODO ACTUALIZACION ──────────────────────────────────────────────
        Write-Step "Actualizacion detectada — reiniciando servicio con nuevo binario..."

        if ($existing.Status -ne 'Stopped') {
            Stop-Service $SERVICE_NAME -Force -ErrorAction SilentlyContinue
            $waited = 0
            while ((Get-Service $SERVICE_NAME -ErrorAction SilentlyContinue).Status -ne 'Stopped' -and $waited -lt 15) {
                Start-Sleep -Seconds 1
                $waited++
            }
        }

        # Siempre reescribir env vars en el registro desde aegis.env.
        # Cubre instalaciones parciales, reinstalaciones y corrupcion de registro.
        Write-EnvToServiceRegistry -EnvPath "$DataDir\aegis.env"

        try {
            Start-Service $SERVICE_NAME -ErrorAction Stop
            Write-OK "Servicio '$SERVICE_NAME' reiniciado con nuevo binario."
        } catch {
            Write-Warn "El servicio no pudo reiniciarse: $_"
            Write-Host "    Ejecuta: aegis start" -ForegroundColor Cyan
        }

    } else {
        # ── MODO INSTALACION FRESCA ─────────────────────────────────────────
        Write-Step "Instalacion fresca — registrando servicio..."

        $binPath = "`"$InstallDir\$BIN_NAME`""

        New-Service `
            -Name        $SERVICE_NAME `
            -DisplayName "Aegis OS — Cognitive Operating System" `
            -Description "Aegis OS kernel (ank-server)." `
            -BinaryPathName $binPath `
            -StartupType Automatic `
            -ErrorAction Stop | Out-Null

        Write-EnvToServiceRegistry -EnvPath "$DataDir\aegis.env"

        sc.exe failure $SERVICE_NAME reset= 60 actions= restart/5000/restart/10000/restart/30000 | Out-Null

        try {
            Start-Service $SERVICE_NAME -ErrorAction Stop
            Write-OK "Servicio '$SERVICE_NAME' iniciado."
        } catch {
            Write-Warn "El servicio no pudo iniciarse: $_"
            Write-Host "    Ejecuta: aegis diag" -ForegroundColor Cyan
        }
    }
}

function Wait-AndShow {
    Write-Step "Esperando que Aegis inicialice (max 30s)..."

    $url   = "http://localhost:8000/health"
    $ready = $false

    for ($i = 0; $i -lt 15; $i++) {
        Start-Sleep -Seconds 2
        try {
            $res = Invoke-WebRequest -Uri $url -UseBasicParsing -TimeoutSec 2 -ErrorAction Stop
            if ($res.Content -match "Online") { $ready = $true; break }
        } catch { }
    }

    Write-Host ""
    Write-Host "  ################################################################" -ForegroundColor Green
    Write-Host "  #          AEGIS OS — INSTALACION COMPLETA                    #" -ForegroundColor Green
    Write-Host "  ################################################################" -ForegroundColor Green
    Write-Host ""

    if ($ready) {
        $ip = (Get-NetIPAddress -AddressFamily IPv4 |
               Where-Object { $_.IPAddress -notlike "127.*" -and $_.IPAddress -notlike "169.*" } |
               Select-Object -First 1).IPAddress
        Write-Host "  Aegis esta corriendo en:" -ForegroundColor White
        Write-Host "    http://localhost:8000" -ForegroundColor Cyan
        if ($ip) { Write-Host "    http://${ip}:8000  (red local)" -ForegroundColor Cyan }
    } else {
        Write-Host "  [!] Aegis no respondio en 30s." -ForegroundColor Yellow
        Write-Host "      Ejecuta 'aegis diag' para diagnosticar." -ForegroundColor DarkGray
    }

    Write-Host ""
    Write-Host "  CLI disponible (nueva terminal):" -ForegroundColor White
    Write-Host "    aegis status    aegis logs    aegis diag    aegis update" -ForegroundColor DarkGray
    Write-Host ""
    Write-Host "  Datos:    $DataDir" -ForegroundColor DarkGray
    Write-Host "  Binario:  $InstallDir\$BIN_NAME" -ForegroundColor DarkGray
    Write-Host "  Config:   $DataDir\aegis.env" -ForegroundColor DarkGray
    Write-Host ""
    Write-Host "  ################################################################" -ForegroundColor Green
    Write-Host ""
}

# --- Main ---
Show-Banner
Test-Prerequisites
New-AegisDirs
Get-AegisBinaries
Get-AegisUI
Get-AgentsConfig
New-AegisEnvFile
Install-AegisCLI

if (-not $NoService) {
    Install-AegisService
} else {
    Write-Warn "Modo --NoService: servicio no registrado."
    Write-Host "  Iniciar manualmente: aegis start" -ForegroundColor Cyan
}

Add-ToPath
Wait-AndShow

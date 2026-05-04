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

$GITHUB_ORG  = "Gustavo324234"
$GITHUB_REPO = "Aegis-Core"
$RELEASE_URL = "https://github.com/$GITHUB_ORG/$GITHUB_REPO/releases/download/$ReleaseTag"
$BIN_NAME    = "ank-server.exe"
$SERVICE_NAME = "AegisOS"

# --- Colores ---
function Write-Step  { param($msg) Write-Host "  -> $msg" -ForegroundColor Cyan }
function Write-OK    { param($msg) Write-Host "  [OK] $msg" -ForegroundColor Green }
function Write-Warn  { param($msg) Write-Host "  [!] $msg" -ForegroundColor Yellow }
function Write-Fail  { param($msg) Write-Host "  [ERROR] $msg" -ForegroundColor Red; exit 1 }

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

function Test-Prerequisites {
    Write-Step "Verificando prerequisitos..."

    # PowerShell version
    if ($PSVersionTable.PSVersion.Major -lt 5) {
        Write-Fail "Se requiere PowerShell 5.1 o superior."
    }

    # Windows version (mínimo Windows 10 / Server 2016)
    $os = [System.Environment]::OSVersion.Version
    if ($os.Major -lt 10) {
        Write-Fail "Se requiere Windows 10 / Server 2016 o superior."
    }

    # Visual C++ Redistributable (requerido por SQLCipher)
    $vcRedist = Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" -ErrorAction SilentlyContinue
    if (-not $vcRedist) {
        Write-Warn "Visual C++ Redistributable 2015-2022 no encontrado."
        Write-Warn "Descargá desde: https://aka.ms/vs/17/release/vc_redist.x64.exe"
        Write-Warn "Instalalo y volvé a ejecutar este script."
        # No fallar — el binario puede tener SQLCipher estático
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

    # Permisos: solo SYSTEM y Administradores en DataDir
    $acl = Get-Acl $DataDir
    $acl.SetAccessRuleProtection($true, $false)
    $adminRule   = New-Object System.Security.AccessControl.FileSystemAccessRule("Administrators","FullControl","ContainerInherit,ObjectInherit","None","Allow")
    $systemRule  = New-Object System.Security.AccessControl.FileSystemAccessRule("SYSTEM","FullControl","ContainerInherit,ObjectInherit","None","Allow")
    $acl.AddAccessRule($adminRule)
    $acl.AddAccessRule($systemRule)
    Set-Acl $DataDir $acl

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
        Write-Fail "No se pudo descargar el binario desde $zipUrl`nError: $_"
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

    # tar está disponible en Windows 10 1803+
    tar -xzf $tarPath -C $uiDir
    Remove-Item $tarPath -Force

    Write-OK "UI assets -> $uiDir"
}

function Get-AgentsConfig {
    Write-Step "Descargando archivos de configuración de agentes..."

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
    Write-Step "Generando configuración de entorno..."

    $envPath = "$DataDir\aegis.env"

    if (Test-Path $envPath) {
        Write-Warn "Archivo de entorno existente preservado: $envPath"
        return
    }

    # Generar AEGIS_ROOT_KEY aleatorio (32 bytes hex)
    $bytes    = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    $rootKey  = [BitConverter]::ToString($bytes).Replace("-","").ToLower()

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

    # Permisos restrictivos en el env file
    $acl = Get-Acl $envPath
    $acl.SetAccessRuleProtection($true, $false)
    $acl.AddAccessRule((New-Object System.Security.AccessControl.FileSystemAccessRule("Administrators","FullControl","None","None","Allow")))
    $acl.AddAccessRule((New-Object System.Security.AccessControl.FileSystemAccessRule("SYSTEM","FullControl","None","None","Allow")))
    Set-Acl $envPath $acl

    Write-OK "Entorno generado: $envPath"
}

function Install-AegisService {
    Write-Step "Registrando servicio de Windows..."

    # Leer variables del env file para pasarlas como env del servicio
    $envVars = @{}
    Get-Content "$DataDir\aegis.env" | ForEach-Object {
        if ($_ -match "^([^#=]+)=(.*)$") {
            $envVars[$matches[1].Trim()] = $matches[2].Trim()
        }
    }

    # Eliminar servicio previo si existe
    $existing = Get-Service -Name $SERVICE_NAME -ErrorAction SilentlyContinue
    if ($existing) {
        Write-Warn "Servicio existente encontrado — eliminando..."
        Stop-Service $SERVICE_NAME -Force -ErrorAction SilentlyContinue
        sc.exe delete $SERVICE_NAME | Out-Null
        Start-Sleep -Seconds 2
    }

    # Crear el servicio
    $binPath = "`"$InstallDir\$BIN_NAME`""

    New-Service `
        -Name $SERVICE_NAME `
        -DisplayName "Aegis OS — Cognitive Operating System" `
        -Description "Aegis OS kernel (ank-server). Manages cognitive processes, routing, and tenant data." `
        -BinaryPathName $binPath `
        -StartupType Automatic `
        -ErrorAction Stop | Out-Null

    # Configurar variables de entorno del servicio via registro
    $regPath = "HKLM:\SYSTEM\CurrentControlSet\Services\$SERVICE_NAME"
    $envArray = $envVars.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" }
    Set-ItemProperty -Path $regPath -Name "Environment" -Value $envArray

    # Configurar recuperación automática (restart en fallo)
    sc.exe failure $SERVICE_NAME reset= 60 actions= restart/5000/restart/10000/restart/30000 | Out-Null

    # Iniciar el servicio
    Start-Service $SERVICE_NAME
    Write-OK "Servicio '$SERVICE_NAME' iniciado."
}

function Add-ToPath {
    Write-Step "Agregando $InstallDir al PATH del sistema..."

    $currentPath = [Environment]::GetEnvironmentVariable("Path", "Machine")
    if ($currentPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$currentPath;$InstallDir", "Machine")
        Write-OK "$InstallDir agregado al PATH."
    } else {
        Write-Step "$InstallDir ya estaba en el PATH."
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
            if ($res.Content -match "Online") {
                $ready = $true
                break
            }
        } catch { }
    }

    Write-Host ""
    Write-Host "  ################################################################" -ForegroundColor Green
    Write-Host "  #          AEGIS OS — INSTALACION COMPLETA                    #" -ForegroundColor Green
    Write-Host "  ################################################################" -ForegroundColor Green
    Write-Host ""

    if ($ready) {
        $ip = (Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.IPAddress -notlike "127.*" -and $_.IPAddress -notlike "169.*" } | Select-Object -First 1).IPAddress
        Write-Host "  Aegis esta corriendo en:" -ForegroundColor White
        Write-Host "    http://localhost:8000" -ForegroundColor Cyan
        if ($ip) {
            Write-Host "    http://${ip}:8000  (acceso desde la red local)" -ForegroundColor Cyan
        }
        Write-Host ""
        Write-Host "  Para obtener el token de setup:" -ForegroundColor White
        Write-Host "    Abrí el Visor de Eventos -> Aplicaciones y Servicios -> AegisOS" -ForegroundColor DarkGray
        Write-Host "    O en PowerShell: Get-EventLog -LogName Application -Source AegisOS -Newest 20" -ForegroundColor DarkGray
    } else {
        Write-Warn "Aegis no respondio en 30s. Revisá los eventos:"
        Write-Host "    Get-EventLog -LogName Application -Source AegisOS -Newest 20" -ForegroundColor DarkGray
    }

    Write-Host ""
    Write-Host "  Para gestionar el servicio:" -ForegroundColor White
    Write-Host "    Start-Service AegisOS" -ForegroundColor DarkGray
    Write-Host "    Stop-Service AegisOS" -ForegroundColor DarkGray
    Write-Host "    Restart-Service AegisOS" -ForegroundColor DarkGray
    Write-Host "    Get-Service AegisOS" -ForegroundColor DarkGray
    Write-Host ""
    Write-Host "  Datos:          $DataDir" -ForegroundColor DarkGray
    Write-Host "  Binario:        $InstallDir\$BIN_NAME" -ForegroundColor DarkGray
    Write-Host "  Configuracion:  $DataDir\aegis.env" -ForegroundColor DarkGray
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

if (-not $NoService) {
    Install-AegisService
} else {
    Write-Warn "Modo --NoService: el servicio no fue registrado."
    Write-Host "  Para iniciar manualmente:"
    Write-Host "    & '$InstallDir\$BIN_NAME'" -ForegroundColor Cyan
}

Add-ToPath
Wait-AndShow

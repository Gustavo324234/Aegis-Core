# ==============================================================================
# AEGIS OS — Windows Installer (PowerShell)
# ==============================================================================
# Requiere: PowerShell 5.1+ o PowerShell Core 7+
# Ejecutar como Administrador:
#   irm https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.ps1 | iex
# O localmente:
#   powershell -ExecutionPolicy Bypass -File install.ps1
# ==============================================================================
#
# Nota de arquitectura:
#   ank-server lee C:/ProgramData/Aegis/aegis.env directamente al arrancar
#   via dotenvy (CORE-265). El installer solo necesita garantizar que ese
#   archivo exista con paths en forward slashes y valores con espacios entre
#   comillas dobles (requisito de dotenvy).
#
# ==============================================================================

#Requires -RunAsAdministrator

param(
    [string]$DataDir     = "$env:ProgramData\Aegis",
    [string]$InstallDir  = "$env:ProgramFiles\Aegis",
    [string]$ReleaseTag  = "nightly",
    [switch]$NoService,
    [switch]$Silent,
    [switch]$Repair,
    [int]$Port           = 0
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

# Normaliza un path para aegis.env:
# 1. Backslashes -> forward slashes (dotenvy los interpreta como escapes)
# 2. Envuelve en comillas dobles si contiene espacios (dotenvy requiere quoting)
function Format-EnvPath {
    param([string]$Path)
    $p = $Path.Replace("\", "/")
    if ($p -match " ") { return "`"$p`"" }
    return $p
}

function Test-PortInUse {
    param([int]$p)
    try {
        $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $p)
        $listener.Start()
        $listener.Stop()
        return $false
    } catch {
        return $true
    }
}

function Select-AegisPort {
    if ($Port -ne 0) {
        $script:AEGIS_HTTP_PORT = $Port
        Write-Step "Puerto HTTP Aegis configurado manualmente: $script:AEGIS_HTTP_PORT"
        return
    }

    # If aegis.env already exists, read the port from it to preserve existing config!
    $envPath = "$DataDir\aegis.env"
    if (Test-Path $envPath) {
        $envLines = Get-Content $envPath
        foreach ($line in $envLines) {
            if ($line -match "^(?:AEGIS_HTTP_PORT|ANK_HTTP_PORT)=(.*)$") {
                $script:AEGIS_HTTP_PORT = [int]($matches[1].Trim('"').Trim())
                Write-Step "Puerto HTTP Aegis detectado de la configuracion existente: $script:AEGIS_HTTP_PORT"
                return
            }
        }
    }

    if (-not (Test-PortInUse -p 8000)) {
        $script:AEGIS_HTTP_PORT = 8000
        Write-Step "Puerto HTTP Aegis: 8000 (libre)"
        return
    }

    Write-Warn "El puerto 8000 ya esta en uso por otro servicio."
    if (-not (Test-PortInUse -p 8080)) {
        $script:AEGIS_HTTP_PORT = 8080
        Write-Warn "Usando puerto alternativo: 8080"
        return
    }

    if (-not (Test-PortInUse -p 8090)) {
        $script:AEGIS_HTTP_PORT = 8090
        Write-Warn "Usando puerto alternativo: 8090"
        return
    }

    Write-Fail "Puertos 8000, 8080 y 8090 estan ocupados. Por favor, especifica un puerto libre con el parametro -Port."
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

function Verify-FileHash {
    param (
        [string]$filePath,
        [string]$expectedHash
    )
    if (-not $expectedHash) {
        Write-Warn "Suma de comprobacion SHA256 no encontrada para $(Split-Path $filePath -Leaf) — procediendo con advertencia."
        return
    }
    
    $cleanExpected = $expectedHash.Split()[0].Trim().ToLower()
    $actualHash = (Get-FileHash -Path $filePath -Algorithm SHA256).Hash.ToLower()
    
    if ($cleanExpected -ne $actualHash) {
        Write-Fail "ERROR CRITICO DE SEGURIDAD: La verificacion de integridad SHA256 falló para $(Split-Path $filePath -Leaf).`nEsperado: $cleanExpected`nObtenido: $actualHash"
    }
    Write-OK "Suma de comprobacion SHA256 verificada con exito para $(Split-Path $filePath -Leaf)."
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

    $expectedHash = ""
    try {
        $expectedHash = (Invoke-RestMethod -Uri "$zipUrl.sha256" -UseBasicParsing).Trim()
    } catch {}

    Verify-FileHash -filePath $zipPath -expectedHash $expectedHash

    Expand-Archive -Path $zipPath -DestinationPath $env:TEMP -Force
    Move-Item "$env:TEMP\ank-server-windows-x86_64.exe" "$InstallDir\$BIN_NAME" -Force
    Remove-Item $zipPath -Force

    Write-OK "ank-server.exe -> $InstallDir\$BIN_NAME"
}

function Get-AegisUI {
    Write-Step "Descargando assets de UI..."

    $tarPath = "$env:TEMP\ui-dist.tar.gz"
    $uiDir   = "$InstallDir\ui"
    $uiUrl   = "$RELEASE_URL/ui-dist.tar.gz"

    try {
        Invoke-WebRequest -Uri $uiUrl -OutFile $tarPath -UseBasicParsing
    } catch {
        Write-Warn "No se pudo descargar la UI. Continuando sin ella."
        return
    }

    $expectedHash = ""
    try {
        $expectedHash = (Invoke-RestMethod -Uri "$uiUrl.sha256" -UseBasicParsing).Trim()
    } catch {}

    if ($expectedHash) {
        Verify-FileHash -filePath $tarPath -expectedHash $expectedHash
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
    $configUrl = "$RELEASE_URL/agents-config.tar.gz"

    try {
        Invoke-WebRequest -Uri $configUrl -OutFile $tarPath -UseBasicParsing
        
        $expectedHash = ""
        try {
            $expectedHash = (Invoke-RestMethod -Uri "$configUrl.sha256" -UseBasicParsing).Trim()
        } catch {}

        if ($expectedHash) {
            Verify-FileHash -filePath $tarPath -expectedHash $expectedHash
        }

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

        # Normalizar vars de path en archivos de versiones anteriores:
        # - Backslashes -> forward slashes
        # - Valores con espacios -> entre comillas dobles
        $pathVars = @("AEGIS_DATA_DIR", "AEGIS_AGENTS_CONFIG_DIR", "UI_DIST_PATH")
        $lines    = Get-Content $envPath
        $changed  = $false

        $newLines = $lines | ForEach-Object {
            $line = $_
            foreach ($varName in $pathVars) {
                if ($line -match "^$varName=(.*)$") {
                    $val     = $matches[1].Trim('"')
                    $newLine = "$varName=$(Format-EnvPath $val)"
                    if ($newLine -ne $line) { $changed = $true }
                    $line = $newLine
                    break
                }
            }
            $line
        }

        # Release builds added by PR #277 refuse to start without an explicit
        # plugin signer key. Backfill the insecure-mode flag if neither it
        # nor AEGIS_PLUGIN_ROOT_KEY is present, so the upgrade doesn't break
        # running deployments. Mirrors the install.sh upgrade-path fix.
        # The line is appended to $newLines (not the file directly) so it
        # survives the Set-Content rewrite below.
        $hasPluginKey = $lines | Where-Object {
            $_ -match "^(AEGIS_PLUGIN_ROOT_KEY|AEGIS_ALLOW_INSECURE_PLUGINS)="
        }
        if (-not $hasPluginKey) {
            $newLines += "AEGIS_ALLOW_INSECURE_PLUGINS=1"
            Write-OK "Backfilled AEGIS_ALLOW_INSECURE_PLUGINS=1 (required by release builds since PR #277)."
            $changed = $true
        }

        # Backfill port if missing
        $hasPort = $lines | Where-Object { $_ -match "^(AEGIS_HTTP_PORT|ANK_HTTP_PORT)=" }
        if (-not $hasPort) {
            $newLines += "AEGIS_HTTP_PORT=$AEGIS_HTTP_PORT"
            $newLines += "ANK_HTTP_PORT=$AEGIS_HTTP_PORT"
            Write-OK "Backfilled AEGIS_HTTP_PORT/ANK_HTTP_PORT as $AEGIS_HTTP_PORT."
            $changed = $true
        }

        if ($changed) {
            Set-Content -Path $envPath -Value $newLines -Encoding UTF8
            Write-OK "aegis.env actualizado (path normalization + plugin-key backfill)."
        }
        return
    }

    $bytes   = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    $rootKey = [BitConverter]::ToString($bytes).Replace("-", "").ToLower()

    $envContent = @"
AEGIS_ROOT_KEY=$rootKey
AEGIS_DATA_DIR=$(Format-EnvPath $DataDir)
AEGIS_AGENTS_CONFIG_DIR=$(Format-EnvPath "$DataDir\agents")
UI_DIST_PATH=$(Format-EnvPath "$InstallDir\ui")
AEGIS_MODEL_PROFILE=cloud
DEFAULT_MODEL_PREF=CloudOnly
RUST_LOG=info
AEGIS_HTTP_PORT=$AEGIS_HTTP_PORT
ANK_HTTP_PORT=$AEGIS_HTTP_PORT
# Release builds refuse to start without an explicit AEGIS_PLUGIN_ROOT_KEY
# (hex-encoded ed25519 public key, >=32 bytes). Until a key-generation
# command ships, opt into the unsigned-plugin mode so the installer's
# first boot doesn't fail. To harden later: generate a real keypair, set
# AEGIS_PLUGIN_ROOT_KEY=<hex>, and remove the line below.
AEGIS_ALLOW_INSECURE_PLUGINS=1
"@

    Set-Content -Path $envPath -Value $envContent -Encoding UTF8
    Set-AegisAcl -Path $envPath -IsDirectory $false

    Write-OK "Entorno generado: $envPath"
}

# Detecta si la DB esta corrupta o desincronizada con la key actual.
# Si es asi, la renombra con timestamp para que el binario cree una nueva limpia.
function Repair-DatabaseIfCorrupted {
    Write-Step "Verificando integridad de la base de datos..."

    $dbPath = "$DataDir\aegis.db"
    if (-not (Test-Path $dbPath)) {
        Write-Step "No hay base de datos existente — instalacion limpia."
        return
    }

    $startOutput = & "$InstallDir\$BIN_NAME" 2>&1 | Select-Object -First 10
    $dbCorrupted = $startOutput | Where-Object {
        $_ -match "SQLCipher|hmac check failed|not a database|authentication failed"
    }

    if ($dbCorrupted) {
        $timestamp  = Get-Date -Format "yyyyMMdd_HHmmss"
        $backupPath = "$DataDir\aegis.db.bak_$timestamp"
        Rename-Item $dbPath $backupPath
        Write-Warn "Base de datos incompatible con la key actual."
        Write-Warn "Renombrada a: aegis.db.bak_$timestamp (datos preservados)"
        Write-Warn "El sistema iniciara con una base de datos nueva limpia."
    } else {
        Write-OK "Base de datos OK."
    }
}

function Install-AegisCLI {
    Write-Step "Instalando Aegis CLI (aegis.ps1)..."

    $cliDest = "$InstallDir\$CLI_NAME"

    try {
        Invoke-WebRequest -Uri "$GITHUB_RAW/installer/$CLI_NAME" -OutFile $cliDest -UseBasicParsing
    } catch {
        Write-Warn "No se pudo descargar aegis.ps1: $_"
        return
    }

    # Wrapper .cmd para que 'aegis' funcione sin extension en cualquier terminal
    $wrapperPath    = "$InstallDir\aegis.cmd"
    $wrapperContent = "@echo off`r`npowershell.exe -ExecutionPolicy Bypass -File `"$cliDest`" %*"
    [System.IO.File]::WriteAllText($wrapperPath, $wrapperContent, [System.Text.Encoding]::ASCII)

    Write-OK "CLI instalado -> $cliDest"
    Write-OK "Wrapper  -> $wrapperPath"
    Write-OK "Uso: aegis status / aegis logs / aegis diag / aegis update"
}

function Set-ServiceEnvironmentFromEnvFile {
    Write-Step "Escribiendo variables de entorno en el servicio Windows..."
    $envPath = "$DataDir\aegis.env"
    if (Test-Path $envPath) {
        $envLines = [System.Collections.Generic.List[string]]::new()
        Get-Content $envPath | ForEach-Object {
            $line = $_.Trim()
            if ($line -ne "" -and -not $line.StartsWith("#")) {
                $envLines.Add($line)
            }
        }
        try {
            $regPath = "SYSTEM\CurrentControlSet\Services\$SERVICE_NAME"
            $key = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey($regPath, $true)
            if ($key) {
                $key.SetValue("Environment", $envLines.ToArray(), [Microsoft.Win32.RegistryValueKind]::MultiString)
                $key.Close()
                Write-OK "Variables de entorno persistidas en el registro para el servicio."
            } else {
                Write-Warn "No se pudo abrir la clave del Registro del servicio para escribir el bloque de entorno."
            }
        } catch {
            Write-Warn "Error al escribir el bloque de entorno en el Registro: $_"
        }
    } else {
        Write-Warn "No se encontro aegis.env para cargar variables en el servicio."
    }
}

function Install-AegisService {
    Write-Step "Configurando servicio de Windows..."

    $existing = Get-Service -Name $SERVICE_NAME -ErrorAction SilentlyContinue

    if ($existing) {
        # ── MODO ACTUALIZACION / REPARACION ─────────────────────────────────
        # ank-server lee aegis.env directamente (CORE-265).
        # Aseguramos que la ruta y configuración estén correctas y persistimos environment.
        Write-Step "Servicio existente detectado — actualizando y configurando..."

        if ($existing.Status -ne 'Stopped') {
            Stop-Service $SERVICE_NAME -Force -ErrorAction SilentlyContinue
            $waited = 0
            while ((Get-Service $SERVICE_NAME -ErrorAction SilentlyContinue).Status -ne 'Stopped' -and $waited -lt 15) {
                Start-Sleep -Seconds 1
                $waited++
            }
        }

        # Asegurar tipo de inicio Automatic y ruta binaria correcta
        $binPath = "`"$InstallDir\$BIN_NAME`" --service"
        sc.exe config $SERVICE_NAME binPath= $binPath start= auto | Out-Null
        sc.exe failure $SERVICE_NAME reset= 60 actions= restart/5000/restart/10000/restart/30000 | Out-Null

        Set-ServiceEnvironmentFromEnvFile

        try {
            Start-Service $SERVICE_NAME -ErrorAction Stop
            Write-OK "Servicio '$SERVICE_NAME' iniciado."
        } catch {
            Write-Warn "El servicio no pudo iniciarse: $_"
            Write-Host "    Ejecuta: aegis start" -ForegroundColor Cyan
        }

    } else {
        # ── MODO INSTALACION FRESCA ─────────────────────────────────────────
        Write-Step "Instalacion fresca — registrando servicio..."

        $binPath = "`"$InstallDir\$BIN_NAME`" --service"

        New-Service `
            -Name        $SERVICE_NAME `
            -DisplayName "Aegis OS — Cognitive Operating System" `
            -Description "Aegis OS kernel (ank-server). Config: $DataDir\aegis.env" `
            -BinaryPathName $binPath `
            -StartupType Automatic `
            -ErrorAction Stop | Out-Null

        sc.exe failure $SERVICE_NAME reset= 60 actions= restart/5000/restart/10000/restart/30000 | Out-Null

        Set-ServiceEnvironmentFromEnvFile

        try {
            Start-Service $SERVICE_NAME -ErrorAction Stop
            Write-OK "Servicio '$SERVICE_NAME' iniciado."
        } catch {
            Write-Warn "El servicio no pudo iniciarse: $_"
            Write-Host "    Ejecuta: aegis diag" -ForegroundColor Cyan
        }
    }
}

function Add-ToPath {
    Write-Step "Agregando $InstallDir al PATH del sistema..."

    $currentPath = [Environment]::GetEnvironmentVariable("Path", "Machine")
    if ($currentPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("Path", "$currentPath;$InstallDir", "Machine")
        Write-OK "$InstallDir agregado al PATH."
        Write-Warn "Abre una terminal nueva para usar el comando 'aegis'."
    } else {
        Write-Step "$InstallDir ya estaba en el PATH."
    }
}

function Wait-AndShow {
    Write-Step "Esperando que Aegis inicialice (max 30s)..."

    $url   = "http://localhost:$AEGIS_HTTP_PORT/health"
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
        Write-Host "    http://localhost:$AEGIS_HTTP_PORT" -ForegroundColor Cyan
        if ($ip) { Write-Host "    http://${ip}:$AEGIS_HTTP_PORT  (red local)" -ForegroundColor Cyan }
    } else {
        Write-Host "  [!] Aegis no respondio en 30s." -ForegroundColor Yellow
        Write-Host "      Abre una terminal nueva y ejecuta: aegis diag" -ForegroundColor DarkGray
    }

    Write-Host ""
    Write-Host "  CLI (abre terminal nueva):" -ForegroundColor White
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
Select-AegisPort
if ($Repair) {
    Show-Banner
    Write-Step "Modo reparacion — reconfigurando y re-registrando servicio..."
    
    # Asegurar que el CLI esté en su lugar
    Install-AegisCLI
    
    if (-not $NoService) {
        Install-AegisService
    } else {
        Write-Warn "Modo --NoService: servicio no registrado."
    }
    
    Add-ToPath
    
    # Verificar que el servicio quedó corriendo e informar al usuario
    $svc = Get-Service $SERVICE_NAME -ErrorAction SilentlyContinue
    if ($svc -and $svc.Status -eq "Running") {
        Write-OK "Servicio AegisOS reparado y reiniciado con exito."
        Wait-AndShow
    } else {
        Write-Warn "El servicio no pudo iniciarse automaticamente."
        Write-Warn "Para iniciarlo manualmente ejecuta (como Administrador):"
        Write-Host '    powershell -ExecutionPolicy Bypass -File install.ps1 -Repair' -ForegroundColor Cyan
        Write-Host '    O via web:' -ForegroundColor DarkGray
        Write-Host '    powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.ps1 | iex" -Repair' -ForegroundColor Cyan
    }
    exit 0
}

Show-Banner
Test-Prerequisites
New-AegisDirs
Get-AegisBinaries
Get-AegisUI
Get-AgentsConfig
New-AegisEnvFile
Repair-DatabaseIfCorrupted
Install-AegisCLI

if (-not $NoService) {
    Install-AegisService
} else {
    Write-Warn "Modo --NoService: servicio no registrado."
    Write-Host "  Iniciar manualmente: aegis start" -ForegroundColor Cyan
}

Add-ToPath

# Al final de la instalación, verificar si el inicio automático funcionó
$svc = Get-Service $SERVICE_NAME -ErrorAction SilentlyContinue
if (-not $NoService -and $svc -and $svc.Status -ne "Running") {
    Write-Warn "El servicio AegisOS no pudo iniciarse automaticamente en segundo plano."
    Write-Warn "Para repararlo e iniciarlo, ejecuta (como Administrador):"
    Write-Host '    powershell -ExecutionPolicy Bypass -File install.ps1 -Repair' -ForegroundColor Cyan
}

Wait-AndShow

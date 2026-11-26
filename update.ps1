#Requires -Version 5.1
param(
    [string]$InstallDir  = "$env:ProgramFiles\Aegis",
    [string]$DataDir     = "$env:ProgramData\Aegis",
    [string]$ServiceName = "AegisOS"
)

$ErrorActionPreference = "Stop"

# Requiere elevación de administrador
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsPrincipal]::WindowsBuiltInRole::Administrator)) {
    Write-Host "Solicitando elevación de Administrador..." -ForegroundColor Yellow
    $argList = @(
        "-NoExit",
        "-ExecutionPolicy", "Bypass",
        "-File", "`"$PSCommandPath`"",
        "-InstallDir", "`"$InstallDir`"",
        "-DataDir", "`"$DataDir`"",
        "-ServiceName", "`"$ServiceName`""
    )
    Start-Process powershell -Verb RunAs -ArgumentList $argList
    exit
}

function Format-EnvPath {
    param([string]$Path)
    $p = $Path.Replace("\", "/")
    if ($p -match " ") { return "`"$p`"" }
    return $p
}

function Copy-FilteredTree {
    param(
        [string]$SourceDir,
        [string]$DestDir,
        [string[]]$ExcludeDirs = @(),
        [string[]]$ExcludePatterns = @()
    )
    if (-not (Test-Path $SourceDir)) { return }
    if (-not (Test-Path $DestDir)) {
        New-Item -ItemType Directory -Path $DestDir -Force | Out-Null
    }
    $srcResolved = (Resolve-Path $SourceDir).Path.TrimEnd("\", "/")
    Get-ChildItem -Path $SourceDir -Recurse -Force | ForEach-Object {
        $rel = $_.FullName.Substring($srcResolved.Length).TrimStart("\", "/")
        $parts = $rel -split "[\\/]"
        foreach ($exDir in $ExcludeDirs) {
            if ($parts -contains $exDir) { return }
        }
        $targetPath = Join-Path $DestDir $rel
        if ($_.PSIsContainer) {
            if (-not (Test-Path $targetPath)) {
                New-Item -ItemType Directory -Path $targetPath -Force | Out-Null
            }
        } else {
            foreach ($pat in $ExcludePatterns) {
                if ($_.Name -like $pat) { return }
            }
            $parentDir = Split-Path $targetPath -Parent
            if (-not (Test-Path $parentDir)) {
                New-Item -ItemType Directory -Path $parentDir -Force | Out-Null
            }
            Copy-Item -Path $_.FullName -Destination $targetPath -Force
        }
    }
}

$RepoRoot  = $PSScriptRoot
$BinSrc    = Join-Path $RepoRoot "target\release\ank-server.exe"
$UiSrc     = Join-Path $RepoRoot "shell\ui\dist"
$ModSrc    = Join-Path $RepoRoot "kernel\modules"
$AgentSrc  = Join-Path $RepoRoot "kernel\config\agents"
$WaSrc     = Join-Path $RepoRoot "tools\whatsapp_bridge"
$AgySrc    = Join-Path $RepoRoot "tools\antigravity_bridge"

if (-not (Test-Path $BinSrc)) {
    throw "No se encontró el binario compilado en $BinSrc. Ejecuta: cargo build --release -p ank-server"
}
if (-not (Test-Path $UiSrc)) {
    throw "No se encontró shell\ui\dist en $UiSrc. Ejecuta: npm --prefix shell/ui run build"
}

Write-Host "=========================================================" -ForegroundColor Cyan
Write-Host "  Actualizando Aegis OS (Servidor, UI, Módulos y Bridges)" -ForegroundColor Cyan
Write-Host "=========================================================" -ForegroundColor Cyan

Write-Host "[1/6] Deteniendo servicio $ServiceName..." -ForegroundColor Yellow
$svc = Get-Service $ServiceName -ErrorAction SilentlyContinue
if ($svc) {
    if ($svc.Status -ne "Stopped") {
        Stop-Service $ServiceName -Force -ErrorAction SilentlyContinue
        for ($i = 0; $i -lt 20; $i++) {
            Start-Sleep -Milliseconds 500
            $svc.Refresh()
            if ($svc.Status -eq "Stopped") { break }
        }
    }
}
$leftover = Get-Process -Name "ank-server" -ErrorAction SilentlyContinue
if ($leftover) {
    Write-Host "  Cerrando proceso residual ank-server..." -ForegroundColor Yellow
    Stop-Process -Name "ank-server" -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
}

Write-Host "[2/6] Copiando binario ank-server.exe..." -ForegroundColor Yellow
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}
Copy-Item $BinSrc (Join-Path $InstallDir "ank-server.exe") -Force

Write-Host "[3/6] Copiando UI frontend (shell\ui\dist)..." -ForegroundColor Yellow
$UiDest = Join-Path $InstallDir "ui"
if (-not (Test-Path $UiDest)) {
    New-Item -ItemType Directory -Path $UiDest -Force | Out-Null
}
Copy-Item (Join-Path $UiSrc "*") $UiDest -Recurse -Force

Write-Host "[4/6] Copiando módulos de dominio y configuración de agentes..." -ForegroundColor Yellow
$ModDest   = Join-Path $DataDir "modules"
$AgentDest = Join-Path $DataDir "agents"
Copy-FilteredTree -SourceDir $ModSrc -DestDir $ModDest
Copy-FilteredTree -SourceDir $AgentSrc -DestDir $AgentDest

Write-Host "[5/6] Sincronizando bridges (WhatsApp y Antigravity) y aegis.env..." -ForegroundColor Yellow
$WaDest  = Join-Path $DataDir "whatsapp_bridge"
$AgyDest = Join-Path $DataDir "antigravity_bridge"
Copy-FilteredTree -SourceDir $WaSrc  -DestDir $WaDest  -ExcludeDirs @("node_modules", ".sessions")
Copy-FilteredTree -SourceDir $AgySrc -DestDir $AgyDest -ExcludeDirs @("__pycache__") -ExcludePatterns @("*.pyc")

$EnvFile = Join-Path $DataDir "aegis.env"
if (Test-Path $EnvFile) {
    $envLines = @(Get-Content $EnvFile)
    $changed = $false
    $parityVars = [ordered]@{
        "AEGIS_MODULES_DIR"            = (Format-EnvPath $ModDest)
        "AEGIS_AGENTS_CONFIG_DIR"      = (Format-EnvPath $AgentDest)
        "AEGIS_WHATSAPP_BRIDGE_DIR"    = (Format-EnvPath $WaDest)
        "AEGIS_ANTIGRAVITY_BRIDGE_DIR" = (Format-EnvPath $AgyDest)
    }
    foreach ($kv in $parityVars.GetEnumerator()) {
        $k = $kv.Key
        $v = $kv.Value
        if (-not ($envLines | Where-Object { $_ -match "^$k=" })) {
            $envLines += "$k=$v"
            $changed = $true
        }
    }
    if ($changed) {
        Set-Content -Path $EnvFile -Value $envLines -Encoding UTF8
        Write-Host "  aegis.env actualizado con rutas de módulos y bridges." -ForegroundColor Green
    }
}

Write-Host "[6/6] Iniciando servicio $ServiceName..." -ForegroundColor Yellow
if (Get-Service $ServiceName -ErrorAction SilentlyContinue) {
    Start-Service $ServiceName
    Write-Host "=========================================================" -ForegroundColor Green
    Get-Service $ServiceName
    Write-Host "=========================================================" -ForegroundColor Green
} else {
    Write-Host "  Servicio $ServiceName no registrado; puedes iniciar manualmente con ank-server.exe." -ForegroundColor Yellow
}
Write-Host "¡Actualización completada con éxito!" -ForegroundColor Green

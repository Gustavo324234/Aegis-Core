# ==============================================================================
# AEGIS OS CLI — Windows (PowerShell)
# ==============================================================================
# Instalado en: C:\Program Files\Aegis\aegis.ps1
# Uso: aegis <comando> [args]
# ==============================================================================

param(
    [Parameter(Position=0)] [string]$Command = "help",
    [Parameter(Position=1)] [string]$Arg1 = "",
    [Parameter(Position=2)] [string]$Arg2 = "",
    [switch]$Stable,
    [switch]$Nightly
)

$SERVICE_NAME = "AegisOS"
$INSTALL_DIR  = "$env:ProgramFiles\Aegis"
$DATA_DIR     = "$env:ProgramData\Aegis"
$HTTP_PORT    = 8000
$envPath = "$DATA_DIR\aegis.env"
# aegis.env has admin-only ACLs; degrade gracefully (default port) when the
# CLI runs unelevated instead of spilling an access-denied error.
if (Test-Path $envPath -ErrorAction SilentlyContinue) {
    $envContent = Get-Content $envPath -ErrorAction SilentlyContinue
    foreach ($line in $envContent) {
        if ($line -match "^(?:AEGIS_HTTP_PORT|ANK_HTTP_PORT)=(.*)$") {
            $HTTP_PORT = [int]($matches[1].Trim('"').Trim())
            break
        }
    }
}
$BIN_PATH     = "$INSTALL_DIR\ank-server.exe"
$GITHUB_ORG   = "Gustavo324234"
$GITHUB_REPO  = "Aegis-Core"

function Write-Cyan  { param($msg) Write-Host $msg -ForegroundColor Cyan }
function Write-Green { param($msg) Write-Host $msg -ForegroundColor Green }
function Write-Red   { param($msg) Write-Host $msg -ForegroundColor Red }
function Write-Yellow{ param($msg) Write-Host $msg -ForegroundColor Yellow }

function Get-ServiceStatus {
    return Get-Service -Name $SERVICE_NAME -ErrorAction SilentlyContinue
}

# ── start ──────────────────────────────────────────────────────────────────────
function cmd_start {
    Write-Cyan "Starting Aegis OS..."
    $svc = Get-ServiceStatus
    if (-not $svc) { Write-Red "Service '$SERVICE_NAME' not found. Run the installer first."; return }
    if ($svc.Status -eq 'Running') { Write-Yellow "Already running."; return }
    Start-Service $SERVICE_NAME
    Write-Green "Aegis OS started."
}

# ── stop ───────────────────────────────────────────────────────────────────────
function cmd_stop {
    Write-Cyan "Stopping Aegis OS..."
    $svc = Get-ServiceStatus
    if (-not $svc) { Write-Red "Service '$SERVICE_NAME' not found."; return }
    if ($svc.Status -eq 'Stopped') { Write-Yellow "Already stopped."; return }
    Stop-Service $SERVICE_NAME -Force
    Write-Green "Aegis OS stopped."
}

# ── restart ────────────────────────────────────────────────────────────────────
function cmd_restart {
    Write-Cyan "Restarting Aegis OS..."
    $svc = Get-ServiceStatus
    if (-not $svc) { Write-Red "Service '$SERVICE_NAME' not found."; return }
    Restart-Service $SERVICE_NAME -Force
    Write-Green "Aegis OS restarted."
}

# ── status ─────────────────────────────────────────────────────────────────────
function cmd_status {
    Write-Cyan "--- Aegis OS Status ---"
    $svc = Get-ServiceStatus
    if (-not $svc) {
        Write-Red "Service '$SERVICE_NAME' not found. Run the installer first."
        return
    }

    $color = if ($svc.Status -eq 'Running') { 'Green' } else { 'Red' }
    Write-Host "  Service:  " -NoNewline
    Write-Host $svc.Status -ForegroundColor $color
    Write-Host "  Startup:  $($svc.StartType)"

    Write-Cyan "`n--- API Health Check ---"
    try {
        $res = Invoke-WebRequest -Uri "http://localhost:$HTTP_PORT/health" -UseBasicParsing -TimeoutSec 3 -ErrorAction Stop
        if ($res.Content -match "Online") {
            Write-Green "  API is UP  (http://localhost:$HTTP_PORT)"
        } else {
            Write-Yellow "  API responded but status unclear."
        }
    } catch {
        Write-Red "  API is DOWN or unreachable."
    }
}

# ── logs ───────────────────────────────────────────────────────────────────────
function cmd_logs {
    param([int]$n = 100)
    Write-Cyan "--- Aegis OS Logs (last $n entries) ---"
    try {
        Get-EventLog -LogName Application -Source "ank-server" -Newest $n -ErrorAction Stop |
            Sort-Object TimeGenerated |
            ForEach-Object { Write-Host "[$($_.TimeGenerated)] $($_.Message)" }
    } catch {
        Write-Yellow "No application logs found. Trying system log..."
        try {
            Get-EventLog -LogName System -Source "Service Control Manager" -Newest 20 -ErrorAction Stop |
                Where-Object { $_.Message -like "*Aegis*" } |
                Sort-Object TimeGenerated |
                ForEach-Object { Write-Host "[$($_.TimeGenerated)] $($_.Message)" }
        } catch {
            Write-Red "No logs found."
        }
    }
}

# ── trace ──────────────────────────────────────────────────────────────────────
# Filtered, one-line-per-event view of the kernel routing path. The Windows
# event log doesn't carry the structured kernel logs the way journalctl does
# on Linux, so this just streams ank-server.log if present and applies the
# same regex filters as the bash version.
function cmd_trace {
    # NOTE: the filter parameter must NOT be named $Pid — that collides with
    # PowerShell's read-only automatic variable and fails at invocation time.
    param([int]$n = 500, [string]$PidFilter = "")
    $logFile = "C:\ProgramData\Aegis\logs\ank-server.log"
    if (-not (Test-Path $logFile)) {
        Write-Red "Log file not found at $logFile"
        Write-Yellow "On Windows the kernel must be configured to write to ank-server.log for trace to work."
        return
    }
    # Patterns we keep (must mirror the awk script in installer/aegis).
    $keep = @(
        'CognitiveRouter: routing decision',
        'CognitiveRouter: routing to model',
        'key marcada como rate-limited',
        'Cloud API returned error status',
        'trying key rotation then fallback chain',
        'alternate key also failed',
        'fallback model also failed',
        'model returned 0 content tokens',
        'ReAct: tool ejecutado',
        'ProcessCompleted \{ pid:',
        'ProjectSupervisor created',
        'LLM execution failed'
    ) -join '|'
    Get-Content $logFile -Tail $n -Wait |
        Where-Object { $_ -match $keep -and ($PidFilter -eq "" -or $_ -match $PidFilter) } |
        ForEach-Object {
            if ($_ -match 'key marcada como rate-limited') { Write-Host -ForegroundColor Yellow $_ }
            elseif ($_ -match 'Cloud API returned error|0 content tokens|LLM execution failed') { Write-Host -ForegroundColor Red $_ }
            elseif ($_ -match 'rotation|alternate key|fallback model') { Write-Host -ForegroundColor Magenta $_ }
            elseif ($_ -match 'tool ejecutado|ProjectSupervisor') { Write-Host -ForegroundColor Green $_ }
            else { Write-Host -ForegroundColor Cyan $_ }
        }
}

# ── version ────────────────────────────────────────────────────────────────────
function cmd_version {
    if (Test-Path $BIN_PATH) {
        & $BIN_PATH --version
    } else {
        Write-Red "Binary not found at $BIN_PATH"
    }
}

# ── token ──────────────────────────────────────────────────────────────────────
function cmd_token {
    Write-Cyan "Retrieving setup token from logs..."
    try {
        $entries = Get-EventLog -LogName Application -Source "ank-server" -Newest 500 -ErrorAction Stop
        $token = $entries |
            ForEach-Object { $_.Message } |
            Select-String -Pattern 'setup_token=([a-f0-9]{32})' |
            Select-Object -Last 1

        if ($token) {
            $t = $token.Matches[0].Groups[1].Value
            $ip = (Get-NetIPAddress -AddressFamily IPv4 |
                   Where-Object { $_.IPAddress -notlike "127.*" -and $_.IPAddress -notlike "169.*" } |
                   Select-Object -First 1).IPAddress
            Write-Green "`nSetup URL: http://${ip}:${HTTP_PORT}?setup_token=$t"
        } else {
            Write-Yellow "No setup token found. System may already be initialized."
        }
    } catch {
        Write-Yellow "Could not read logs. Try restarting the service."
    }
}

# ── diag ───────────────────────────────────────────────────────────────────────
function cmd_diag {
    Write-Cyan "================================================================"
    Write-Cyan "   AEGIS OS - DIAGNOSTIC REPORT"
    Write-Cyan "================================================================"

    Write-Yellow "`n[1] SERVICE STATE"
    $svc = Get-ServiceStatus
    if ($svc) {
        Write-Host "  Status:  $($svc.Status)"
        Write-Host "  Startup: $($svc.StartType)"
    } else {
        Write-Red "  Service not found."
    }

    Write-Yellow "`n[2] BINARY"
    if (Test-Path $BIN_PATH) {
        $ver = & $BIN_PATH --version 2>&1
        Write-Host "  $BIN_PATH"
        Write-Host "  Version: $ver"
    } else {
        Write-Red "  Binary not found at $BIN_PATH"
    }

    Write-Yellow "`n[3] CONFIGURATION"
    $envPath = "$DATA_DIR\aegis.env"
    if (Test-Path $envPath) {
        Write-Green "  $envPath (exists)"
        $keys = Get-Content $envPath | Where-Object { $_ -match "^[^#=\s][^=]*=" } | ForEach-Object { ($_ -split "=")[0] }
        Write-Host "  Keys: $($keys -join ', ')"
    } else {
        Write-Red "  $envPath NOT FOUND"
    }

    Write-Yellow "`n[4] PORTS"
    $ports = netstat -ano | Select-String "$HTTP_PORT|50051"
    if ($ports) { $ports | ForEach-Object { Write-Host "  $_" } }
    else { Write-Yellow "  Aegis ports not listening." }

    Write-Yellow "`n[5] RECENT ERRORS (Event Log)"
    try {
        Get-EventLog -LogName System -Source "Service Control Manager" -Newest 5 -EntryType Error -ErrorAction Stop |
            Where-Object { $_.Message -like "*Aegis*" } |
            ForEach-Object { Write-Host "  [$($_.TimeGenerated)] $($_.Message)" }
    } catch {
        Write-Host "  No recent service errors."
    }

    Write-Yellow "`n[6] SERVICE REGISTRY ENV VARS"
    $regPath = "HKLM:\SYSTEM\CurrentControlSet\Services\$SERVICE_NAME"
    if (Test-Path $regPath) {
        $regEnv = (Get-ItemProperty $regPath -ErrorAction SilentlyContinue).Environment
        if ($regEnv) {
            $regEnv | ForEach-Object { Write-Host "  $_" }
        } else {
            Write-Red "  No Environment vars in service registry."
        }
    } else {
        Write-Red "  Service registry key not found."
    }

    Write-Cyan "`n================================================================"
}

# ── update ─────────────────────────────────────────────────────────────────────
function cmd_update {
    # CORE-329 parity with the Linux wrapper: default = stable, nightly is
    # opt-in. Accepts both PowerShell switches (-Nightly / -Stable) and
    # bash-style flags (--nightly / --beta / --stable), which PowerShell
    # binds as positional args instead of switches.
    $wantsNightly = $Nightly -or ($Arg1 -in @('--nightly', '--beta'))
    if ($Stable -or $Arg1 -eq '--stable') { $wantsNightly = $false }
    $channel = if ($wantsNightly) { "nightly" } else { "stable" }

    $tag = "nightly"
    if (-not $wantsNightly) {
        # There is no literal "latest" tag: GitHub's /releases/latest endpoint
        # is polluted by per-crate release-please releases (ank-server-vX.Y.Z)
        # that carry no OS binaries — only v* tags do. Mirror the bash
        # wrapper: pick the newest non-prerelease v* tag, with a pinned
        # fallback if the API is unreachable or rate-limited.
        $tag = "v0.2.0"
        try {
            [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
            $releases = Invoke-RestMethod -Uri "https://api.github.com/repos/$GITHUB_ORG/$GITHUB_REPO/releases" -UseBasicParsing
            $stable = $releases | Where-Object { -not $_.prerelease -and $_.tag_name -like 'v*' } | Select-Object -First 1
            if ($stable) { $tag = $stable.tag_name }
        } catch {
            Write-Yellow "  Could not resolve latest stable release from GitHub API - falling back to $tag"
        }
    }

    Write-Cyan "--- Aegis OS Update ---"
    Write-Host "  Channel: " -NoNewline; Write-Yellow "$channel (tag: $tag)"
    Write-Host "  Current: " -NoNewline; & $BIN_PATH --version 2>&1 | Write-Host

    Write-Cyan "`n  Stopping service..."
    Stop-Service $SERVICE_NAME -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2

    $zipUrl  = "https://github.com/$GITHUB_ORG/$GITHUB_REPO/releases/download/$tag/ank-server-windows-x86_64.zip"
    $zipPath = "$env:TEMP\ank-update.zip"

    Write-Cyan "  Downloading binary..."
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $zipUrl -OutFile $zipPath -UseBasicParsing
    } catch {
        Write-Red "  Failed to download: $_"
        Start-Service $SERVICE_NAME -ErrorAction SilentlyContinue
        return
    }

    Expand-Archive -Path $zipPath -DestinationPath $env:TEMP -Force
    Move-Item "$env:TEMP\ank-server-windows-x86_64.exe" $BIN_PATH -Force
    Remove-Item $zipPath -Force

    Write-Cyan "  Downloading UI..."
    $tarPath = "$env:TEMP\ui-update.tar.gz"
    try {
        Invoke-WebRequest -Uri "https://github.com/$GITHUB_ORG/$GITHUB_REPO/releases/download/$tag/ui-dist.tar.gz" -OutFile $tarPath -UseBasicParsing
        tar -xzf $tarPath -C "$INSTALL_DIR\ui"
        Remove-Item $tarPath -Force
        Write-Green "  UI updated."
    } catch {
        Write-Yellow "  UI not available in this release — keeping current."
    }

    Write-Cyan "  Downloading agent config..."
    $agentPath = "$env:TEMP\agents-update.tar.gz"
    try {
        Invoke-WebRequest -Uri "https://github.com/$GITHUB_ORG/$GITHUB_REPO/releases/download/$tag/agents-config.tar.gz" -OutFile $agentPath -UseBasicParsing
        tar -xzf $agentPath -C "$DATA_DIR\agents"
        Remove-Item $agentPath -Force
        Write-Green "  Agent config updated."
    } catch {
        Write-Yellow "  Agent config not available — using binary fallbacks."
    }

    Write-Cyan "  Starting service..."
    Start-Service $SERVICE_NAME
    Write-Green "`nUpdate complete."
    Write-Host "  New version: " -NoNewline; & $BIN_PATH --version 2>&1 | Write-Host
}

# ── uninstall ──────────────────────────────────────────────────────────────────
function cmd_uninstall {
    Write-Yellow "This will remove Aegis OS from this system."
    Write-Yellow "Your data in $DATA_DIR will be preserved."
    $confirm = Read-Host "Type YES to confirm"
    if ($confirm -ne "YES") { Write-Host "Aborted."; return }

    Stop-Service $SERVICE_NAME -Force -ErrorAction SilentlyContinue
    sc.exe delete $SERVICE_NAME | Out-Null

    Remove-Item $INSTALL_DIR -Recurse -Force -ErrorAction SilentlyContinue
    Write-Green "Aegis OS uninstalled. Data preserved at $DATA_DIR"
}

# ── help ───────────────────────────────────────────────────────────────────────
function cmd_help {
    Write-Host ""
    Write-Cyan "Aegis OS CLI (Windows)"
    Write-Host ""
    Write-Host "Usage: aegis <command> [options]"
    Write-Host ""
    Write-Host "Commands:"
    Write-Host "  start              Start Aegis service"
    Write-Host "  stop               Stop Aegis service"
    Write-Host "  restart            Restart Aegis service"
    Write-Host "  status             Check service and API health"
    Write-Host "  logs [N]           Show last N event log entries (default 100)"
    Write-Host "  trace [N] [PID]    Filtered model-routing trace (no JSON walls); default 500 lines"
    Write-Host "  version            Show installed version"
    Write-Host "  token              Get setup URL with fresh token"
    Write-Host "  diag               Full diagnostic report"
    Write-Host "  update             Update to latest stable release"
    Write-Host "  update --nightly   Update to latest nightly build from main"
    Write-Host "  uninstall          Remove Aegis from system"
    Write-Host ""
}

# ── router ─────────────────────────────────────────────────────────────────────
switch ($Command.ToLower()) {
    "start"     { cmd_start }
    "stop"      { cmd_stop }
    "restart"   { cmd_restart }
    "status"    { cmd_status }
    "logs"      { $n = if ($Arg1 -match '^\d+$') { [int]$Arg1 } else { 100 }; cmd_logs $n }
    "trace"     {
        # `??` is PS7-only and powershell.exe (5.1) runs this script via
        # aegis.cmd, so stick to 5.1-compatible syntax here.
        $n = 500; $pidFilter = ""
        if ($Arg1 -match '^\d+$') { $n = [int]$Arg1 } elseif ($Arg1) { $pidFilter = $Arg1 }
        if ($Arg2) { $pidFilter = $Arg2 }
        cmd_trace -n $n -PidFilter $pidFilter
    }
    "version"   { cmd_version }
    "token"     { cmd_token }
    "diag"      { cmd_diag }
    "update"    { cmd_update }
    "uninstall" { cmd_uninstall }
    default     { cmd_help }
}

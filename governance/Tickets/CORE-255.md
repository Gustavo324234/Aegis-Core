# CORE-255 — Installer: registro robusto del servicio Windows + opción de reparación

**Tipo:fix
**Prioridad:** Crítica  
**Épica:** Infraestructura  
**Asignado a:DevOps Engineer
**Componente:** `installer/install.ps1`

**Estado:** ✅ Done
---

## Contexto

El installer registra el servicio `AegisOS` correctamente pero falla al iniciarlo porque las variables de entorno del `aegis.env` no llegan al proceso del servicio en el contexto de Windows Service (que corre sin variables de entorno de usuario).

Además, si el usuario reinicia la PC y el servicio no arrancó automáticamente, no hay forma de reactivarlo sin usar comandos PowerShell.

---

## Cambios requeridos

### 1. Fix: variables de entorno en el servicio

El problema actual es que `Set-ItemProperty -Type MultiString` a veces no persiste correctamente con `New-Service`. La solución robusta es usar `sc.exe` directamente para escribir el environment, o mejor aún: crear un wrapper `.bat` que cargue el `aegis.env` antes de lanzar el binario.

Reemplazar el `BinaryPathName` del servicio para que apunte a un wrapper:

**`C:\Program Files\Aegis\start-aegis.bat`:**
```bat
@echo off
for /f "usebackq tokens=1,* delims==" %%A in ("C:\ProgramData\Aegis\aegis.env") do (
    if not "%%A"=="" if not "%%A:~0,1%"=="#" set "%%A=%%B"
)
"C:\Program Files\Aegis\ank-server.exe"
```

Registrar el servicio apuntando al wrapper:
```powershell
New-Service `
    -Name "AegisOS" `
    -BinaryPathName "`"C:\Program Files\Aegis\start-aegis.bat`"" `
    -StartupType Automatic
```

**Nota:** Windows Service no puede ejecutar `.bat` directamente. Usar en cambio un wrapper con `cmd.exe`:
```powershell
$wrapperPath = "cmd.exe /c `"C:\Program Files\Aegis\start-aegis.bat`""
```

O la solución más limpia: escribir las vars directo al bloque de entorno del proceso via el registro de forma correcta antes de llamar `Start-Service`:

```powershell
# Leer aegis.env y construir array tipado correctamente
$envLines = [System.Collections.Generic.List[string]]::new()
Get-Content "$DataDir\aegis.env" | ForEach-Object {
    $line = $_.Trim()
    if ($line -ne "" -and -not $line.StartsWith("#")) {
        $envLines.Add($line)
    }
}
$regPath = "HKLM:\SYSTEM\CurrentControlSet\Services\AegisOS"
$key = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey(
    "SYSTEM\CurrentControlSet\Services\AegisOS", $true)
$key.SetValue("Environment", $envLines.ToArray(), 
    [Microsoft.Win32.RegistryValueKind]::MultiString)
$key.Close()
```

El Kernel Engineer / DevOps debe probar cuál de los dos enfoques (wrapper bat o Registry API) funciona de forma confiable en Windows 10/11.

### 2. Nuevo: modo `--repair` en el installer

Agregar un parámetro `-Repair` al script que:
- No descarga ni sobreescribe binarios ni UI
- No toca el `aegis.env` existente
- Solo re-registra el servicio y lo inicia

```powershell
param(
    ...
    [switch]$Repair
)

if ($Repair) {
    Write-Step "Modo reparacion — re-registrando servicio..."
    Install-AegisService   # funcion ya existente
    Write-OK "Servicio reparado y reiniciado."
    exit 0
}
```

**Uso:**
```powershell
powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/.../install.ps1 | iex" -Repair
```

O si ya está descargado:
```powershell
powershell -ExecutionPolicy Bypass -File install.ps1 -Repair
```

### 3. Nuevo: verificación de inicio automático

Al final del installer, verificar que el servicio quedó configurado como `Automatic` y está en estado `Running`. Si no, mostrar instrucciones claras:

```powershell
$svc = Get-Service AegisOS
if ($svc.Status -ne "Running") {
    Write-Warn "El servicio no pudo iniciarse automaticamente."
    Write-Warn "Para iniciarlo manualmente ejecuta (como Administrador):"
    Write-Host '    powershell -ExecutionPolicy Bypass -c "irm https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.ps1 | iex" -Repair' -ForegroundColor Cyan
}
```

## Criterios de aceptación

- [x] El servicio `AegisOS` arranca automáticamente al reiniciar Windows sin intervención manual.
- [x] Las variables de entorno del `aegis.env` están disponibles para el proceso del servicio.
- [x] El parámetro `-Repair` re-registra y reinicia el servicio sin tocar datos ni configuración.
- [x] Si el inicio automático falla, el installer muestra el comando `-Repair` claramente.
- [x] Probado en Windows 10 y Windows 11.

## Dependencias

Ninguna.

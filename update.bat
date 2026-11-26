@echo off
chcp 65001 >nul
:: Verificar si se ejecuta como Administrador
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo Solicitando elevacion de Administrador...
    powershell -Command "Start-Process '%~f0' -Verb RunAs"
    exit /b
)

echo ===================================================
echo   Actualizando Aegis OS (Servidor y UI)
echo ===================================================

echo [1/4] Cerrando ank-server.exe...
taskkill /F /IM ank-server.exe >nul 2>&1
timeout /t 2 /nobreak >nul

echo [2/4] Copiando ank-server.exe compilado...
copy /Y "E:\Aegis\Aegis-Core\target\release\ank-server.exe" "C:\Program Files\Aegis\ank-server.exe"
if %errorlevel% neq 0 (
    echo [ERROR] No se pudo copiar ank-server.exe. Verifique que no este bloqueado.
    pause
    exit /b 1
)

echo [3/4] Copiando UI frontend actualizada...
xcopy /E /Y /I "E:\Aegis\Aegis-Core\shell\ui\dist\*" "C:\Program Files\Aegis\ui\" >nul
if %errorlevel% neq 0 (
    echo [ERROR] No se pudo copiar la UI frontend.
    pause
    exit /b 1
)

echo [4/4] Iniciando servicio AegisOS...
net start AegisOS

echo.
echo ===================================================
echo   Estado de AegisOS:
sc query AegisOS | findstr "STATE"
echo ===================================================
echo.
echo Actualizacion completada con exito.
pause

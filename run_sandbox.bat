@echo off
setlocal EnableExtensions
title Aegis Agent Sandbox
cd /d "%~dp0"
echo ========================================================
echo   Iniciando Aegis Agent Sandbox ^& Server Web de Prueba
echo ========================================================
echo.

python tools\agent_sandbox\launch_web_server.py %*
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Ocurrio un error al iniciar el server web de prueba.
    pause
    exit /b %ERRORLEVEL%
)

if /I "%~1"=="--server-only" (
    echo.
    echo Modo --server-only: CLI interactiva omitida.
    exit /b 0
)

echo.
python tools\agent_sandbox\sandbox.py --interactive -v
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Ocurrio un error al ejecutar el sandbox.
    pause
)

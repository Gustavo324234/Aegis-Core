@echo off
setlocal
echo [Aegis Voice] Iniciando demonio de escucha continua para PC (navegador cerrado)...
python "%~dp0tools\desktop_voice\aegis_voice_daemon.py" %*
endlocal

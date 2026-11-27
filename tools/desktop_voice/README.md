# Aegis PC Always-On Voice Daemon (`EPIC 63 / CORE-414`)

Escuchá y hablá con Aegis **aunque tengas el navegador completamente cerrado**.

## Instalación rápida

```bash
pip install -r tools/desktop_voice/requirements.txt
```

## Uso en Windows

Con `run_voice_daemon.bat` o directamente:

```bash
python tools/desktop_voice/aegis_voice_daemon.py --url http://127.0.0.1:8000 --tenant default --passphrase TU_CLAVE
```

- **Activación continua:** Decí `"Aegis escuchame <pedido>"` (o el nombre personalizado de tu asistente). Si registraste voces en *Sistema → Voz*, rechaza cualquier voz desconocida sin gastar tokens.
- **Atajo global:** Presioná **`Alt + Shift + A`** desde cualquier programa para que Aegis empiece a escucharte directo.
- **Inicio automático con Windows:**

```bash
python tools/desktop_voice/aegis_voice_daemon.py --url http://127.0.0.1:8000 --tenant default --install-startup
```

# CORE-414: Demonio de Voz de Escritorio Siempre Activo con Navegador Cerrado (`tools/desktop_voice/`)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 63
- **Asignado a:** Kernel Engineer + Shell Engineer

---

## Description

Proveer un compañero liviano en segundo plano para Windows/Linux/macOS (`tools/desktop_voice/aegis_voice_daemon.py` + `run_voice_daemon.bat`) que mantiene la escucha activa `"Aegis escuchame"` y atajo global (`Alt+Espacio` / `Ctrl+Shift+Space`) aun cuando el navegador esté completamente cerrado, validando huellas vocales contra `ank-server` y respondiendo por voz nativa del sistema.

## Acceptance Criteria

- [x] `aegis_voice_daemon.py` implementa `BreathAwareVadSegmenter` (`0` tokens en silencio), atajo global Win32, verificación biométrica via `/api/siren/utterance`, respuesta hablada nativa e instalador `--install-startup` en Windows.
- [x] `run_voice_daemon.bat`, `requirements.txt`, `README.md` y tests unitarios `test_voice_daemon.py` incluidos.

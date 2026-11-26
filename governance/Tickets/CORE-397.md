# CORE-397: Paridad Sandbox → Producción en Instaladores, Updates y `ide_hook.py`

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 61
- **Asignado a:** DevOps Engineer

---

## Description

Cerrar las brechas de empaquetado e instalación detectadas en la auditoría para que todo lo validado en el sandbox (`:8090`) funcione igual en instalaciones de producción (`:8000`):
- Empaquetar e instalar `kernel/modules/` (`aegis-biz`, `aegis-creator`, `aegis-dev`, `aegis-video`), `kernel/config/agents/`, `tools/whatsapp_bridge/` y `tools/antigravity_bridge/` en `.github/workflows/publish-native.yml`, `installer/install.ps1` e `installer/install.sh`.
- Actualizar `update.ps1` (añadir UTF-8 BOM para PowerShell 5.1, usar `$PSScriptRoot` en vez de rutas hardcodeadas, `Stop-Service`/`Start-Service` en lugar de `taskkill /F`, copiar también `modules/` y `agents/`) y `deploy.py` (`shutil.which`, verificación de binario OS, `StrictHostKeyChecking=accept-new`, despliegue de `modules/` y `agents/`).
- Cambiar la URL por defecto de `tools/antigravity_bridge/ide_hook.py` de `:8090` a `:8000` y soportar `AEGIS_HOOK_TOKEN` (`X-Aegis-Hook-Token`).

## Acceptance Criteria

- [x] `publish-native.yml`, `install.ps1`, `install.sh`, `update.ps1` y `deploy.py` distribuyen `modules/`, `agents/` y bridges de forma consistente.
- [x] `update.ps1` posee UTF-8 BOM y pasa el parse check de PowerShell 5.1.
- [x] `ide_hook.py` apunta por defecto a `http://127.0.0.1:8000` y envía `X-Aegis-Hook-Token` cuando está configurado.

# CORE-388: Launcher Web Aislado para Sandbox (`:8090`) y Scripts de Actualización Windows (`tools/agent_sandbox/`, raíz)

- **Tipo:** feat
- **Estado:** ✅ Done
- **Prioridad:** Alta
- **Épica:** EPIC 60
- **Asignado a:** DevOps Engineer

---

## Description
Ejecutar `run_sandbox.bat` levantaba únicamente la CLI en Python, obligando a compilar o levantar manualmente el servidor web de pruebas aparte y generando riesgo de colisionar con el servicio `AegisOS` de producción (puerto `8000`) o su configuración en `C:\ProgramData\Aegis\aegis.env`. Además, faltaban utilidades rápidas en Windows para desplegar binarios `release` y `shell/ui/dist`.

## Archivos Creados / Modificados
1. **`tools/agent_sandbox/launch_web_server.py`** (Nuevo):
   - Resuelve entorno de pruebas (`AEGIS_SANDBOX_ENV` → `tools/agent_sandbox/.env` → fallback aislado; sin tocar `ProgramData`), valida UI/binario y levanta `ank-server` en una consola dedicada sobre puerto HTTP `8090` / gRPC `50059`.
2. **`run_sandbox.bat`** (Nuevo):
   - Invoca `python tools\agent_sandbox\launch_web_server.py %*`, soporta el flag `--server-only` y, sin ese flag, continúa con `python tools\agent_sandbox\sandbox.py --interactive -v`.
3. **`tools/agent_sandbox/README.md`**:
   - Documentado el uso de `run_sandbox.bat` y `--server-only`.
4. **`update.bat` y `update.ps1`** (Nuevos):
   - Scripts de actualización administrativa en Windows para detener `ank-server.exe`, copiar `target\release\ank-server.exe` y `shell\ui\dist\*` hacia `C:\Program Files\Aegis\` e iniciar el servicio `AegisOS`.

## Acceptance Criteria
- [x] `run_sandbox.bat --server-only` levanta el servidor web de prueba en `http://127.0.0.1:8090` sin afectar el puerto `8000` ni `C:\ProgramData\Aegis\aegis.env`.
- [x] `run_sandbox.bat` sin flags levanta el servidor web de prueba y la CLI interactiva de sandbox.
- [x] Existen `update.bat` y `update.ps1` para actualizar ejecutable y UI en Windows.

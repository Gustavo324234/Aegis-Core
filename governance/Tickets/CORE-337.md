# CORE-337 — B6 — Citadel: Generación de firmas ed25519 automáticas al arranque local (keygen)

**Tipo:** feat
**Prioridad:** Crítica
**Épica:** EPIC 56 — Public MVP / Thesis Validation
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

Actualmente, el script de instalación escribe `AEGIS_ALLOW_INSECURE_PLUGINS=1` de forma predeterminada porque el sistema requiere firmas ed25519 en los plugins Wasm, pero no existe una forma automática o un comando sencillo para que el usuario o el instalador generen su propio par de llaves criptográficas al inicializar el sistema por primera vez. Esto contradice de forma directa la narrativa "Zero-Trust" y Citadel del proyecto.

## Solución propuesta

Implementar una utilidad de generación de llaves en el kernel y la CLI:
1. Crear un comando en `ank-cli` (ej. `ank-cli keygen` o `aegis keygen`) para generar un par de llaves ed25519 locales seguras.
2. Integrar lógica en la inicialización de `ank-server` para que, en caso de no detectar una llave de firmado de plugins en el archivo de configuración cifrado de Citadel, genere una llave local de forma transparente en el primer arranque, establezca `AEGIS_PLUGIN_ROOT_KEY` y configure el sistema de forma segura por defecto.
3. Eliminar el flag `AEGIS_ALLOW_INSECURE_PLUGINS=1` del instalador estable y forzar la validación de firmas por defecto, a menos que el usuario lo desactive de forma explícita y manual en la configuración.

## Criterios de aceptación

- [x] Comando de CLI o lógica de auto-boot funcional que crea un par de llaves ed25519 válidas y guarda la llave pública en la configuración local de Citadel.
- [x] El sistema rechaza la carga de plugins sin firma válida una vez inicializado, a menos que el flag inseguro sea activado manualmente por el usuario.
- [x] El instalador no inyecta el flag de omisión de seguridad por defecto.
- [x] Compilación y suite de pruebas de Citadel en verde.

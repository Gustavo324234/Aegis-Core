# Guía del Asistente de IA (CLAUDE.md) — Aegis OS

Este archivo sirve como memoria y guía de referencia rápida para **Antigravity** (o cualquier agente de IA de desarrollo) en sus sesiones de pair-programming con el usuario en el monorepo **Aegis-Core**. Define de manera estricta la metodología de trabajo, los comandos frecuentes y el flujo de integración continua.

---

## 🛠️ Metodología y Flujo de Trabajo

### 1. Gestión Rigurosa de Gobernanza (Tickets)
* **Gobernanza Primero:** Cada tarea o modificación en el código debe estar respaldada y justificada bajo un ticket formal en `governance/Tickets/CORE-XXX.md`.
* **Consistencia del Master:** El listado general en `governance/TICKETS_MASTER.md` debe mantenerse en sincronía absoluta de estados (`✅ Done` / `🚧 In Progress` / `📥 Todo`) con el disco.
* **Script Anti-Drift:** Siempre correr la validación de consistencia antes de dar por completado un ciclo de desarrollo:
  ```powershell
  python tools/sync_tickets_master.py --report
  ```

### 2. Ramas (Branches) y Ciclo de Vida de Git
* **Prohibido Main Directo:** Nunca realizar commits directos sobre la rama `main`.
* **Ramas Descriptivas:** Crear siempre una rama de trabajo nueva con un prefijo que coincida con el tipo de Conventional Commits (`fix/...` o `feat/...`).
  * *Ejemplo:* `fix/onboarding-security-and-orchestrator`

### 3. Conventional Commits (Release Please)
* **Estándar Riguroso:** Todos los commits y los títulos de Pull Requests deben seguir la convención de **Conventional Commits** para interactuar limpiamente con la automatización de **release-please**:
  * Formato: `<type>(<scope>): <description>`
  * *Tipos Comunes:* `fix` (bug fix - genera versión patch), `feat` (nueva feature - genera versión minor), `chore` (mantenimiento), `docs` (documentación).
  * *Ámbitos Comunes (Scopes):* `core`, `agents`, `installer`, `governance`, `cli`, `server`.
  * *Ejemplo:* `fix(core): resolve infinite synthesis loop in agent reports (CORE-288)`

---

## 💻 Comandos Frecuentes y Guías Rápidas

### Compilación y Construcción (Rust)
```powershell
cargo check --workspace
cargo build --workspace
```

### Ejecución de Pruebas (Cargo Test)
* **Suite Completa:**
  ```powershell
  cargo test --workspace
  ```
* **Suite de Agentes Encontrada (Ultra-rápida y enfocada):**
  ```powershell
  cargo test -p ank-core --lib -- agents::
  ```

### Herramientas de Scripting y Sincronización (Python)
* **Unicode en Windows:** Ante terminales PowerShell de Windows con encodificados CP1252 heredados, siempre anteponer el forzado UTF-8 para evitar caídas al procesar emojis o unicode:
  ```powershell
  $env:PYTHONIOENCODING="utf-8"
  ```
* **Sincronización de PinchBench en modelos.yaml:**
  ```powershell
  python tools/update_models.py --only-scores
  ```

---

## 🛡️ Principios y Reglas de Diseño SRE

1. **Zero-Panic:** El código del Kernel en Rust y los scripts de instalación de producción deben tolerar fallos en caliente y degradar con gracia (*graceful degradation*).
2. **Soberanía y Zero-Trust:** Citadel protocol debe requerir llaves locales auto-generadas de forma transparente y rechazar plugins sin firma válida (`.wasm.sig`) a menos que el usuario lo solicite explícitamente.
3. **Idempotencia de Actores:** El ciclo de vida de los supervisores y especialistas jerárquicos debe ser acotado y de un solo reporte (synthesis_done), terminando su canal cuando finalizan su labor en lugar de permanecer zombis.

# Aegis OS

> **Un sistema operativo cognitivo.** Un binario. Sin dependencias de runtime. LLMs como ALUs bajo un motor de ejecución determinístico.

[![Licencia: MIT](https://img.shields.io/badge/Licencia-MIT-blue.svg)](LICENSE)
[![Build](https://github.com/Gustavo324234/Aegis-Core/actions/workflows/publish-native.yml/badge.svg)](https://github.com/Gustavo324234/Aegis-Core/actions)
[![GitHub Sponsors](https://img.shields.io/badge/Sponsor-%E2%9D%A4-pink?logo=github)](https://github.com/sponsors/Gustavo324234)

---

## ¿Qué es Aegis?

Aegis es un sistema operativo cognitivo self-hosted — una plataforma donde los agentes de IA corren como procesos de primera clase, con memoria, scheduling, multi-tenancy y ejecución de herramientas integrados en el kernel.

El propósito fundamental de Aegis es ser el **asistente personal definitivo que gestiona un ecosistema de agentes especializados, actuando como el "CIO (Chief Information Officer) de la empresa de tu vida"**. Está diseñado para instalarse localmente en tu propia máquina o servidor, garantizando seguridad absoluta (local-first con datos cifrados por inquilino) para albergar toda tu información personal o corporativa, permitiendo que cualquier persona o empresa tenga su propio asistente autónomo e independiente.

No es un wrapper de chatbot. No es un pipeline de LangChain. Es un runtime a nivel de kernel para cargas de trabajo cognitivas autónomas.

**Ideas centrales:**

- **LLMs como ALUs** — los modelos de lenguaje son unidades de cómputo probabilísticas bajo un scheduler determinístico, no oráculos
- **Kernel Zero-Panic** — escrito en Rust con `clippy::unwrap_used` rechazado en CI
- **Protocolo Citadel** — autenticación multi-tenant Zero-Trust en cada capa
- **Un binario** — `ank-server` sirve la API HTTP, WebSocket y la UI React sin runtime externo
- **Listo para distro** — diseñado para correr como servicio de sistema, eventualmente embebido en una distribución Linux mínima

---

## Arquitectura

```
Browser / App Mobile
        │  HTTP + WebSocket
        ▼
 ank-server  (binario único Rust)
        │
        ├── ank-http    HTTP :8000  — API REST, WebSocket, UI React embebida
        ├── ank-core    Motor cognitivo — scheduler, VCM, agentes, DAG, plugins
        └── gRPC :50051 — comunicación interna, federación multi-nodo
```

El sistema es multi-tenant: cada tenant tiene un entorno cognitivo aislado con sus propias capas de memoria (L1/L2/L3), árbol de agentes y almacenamiento cifrado (SQLCipher).

Ver [ARCHITECTURE.md](ARCHITECTURE.md) para detalle completo.

---

## Instalación Rápida

Aegis distribuye binarios nativos pre-compilados para todas las plataformas principales. No se requiere compilación.

### Linux (Ubuntu 22.04+ / Debian 12+)

```bash
curl -fsSL https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.sh | sudo bash
```

El instalador te guía por:
1. **Modo de instalación** — Nativo (recomendado) o Docker
2. **Perfil de inferencia** — Cloud (API keys), Local (Ollama), o Híbrido
3. **Tier de hardware** — Laptop/VPS, Workstation, o servidor SRE-grade

Después de la instalación, Aegis arranca automáticamente e imprime tu URL de configuración:

```
################################################################
#          AEGIS OS — INSTALACIÓN COMPLETA                     #
################################################################

  Acceso Remoto (HTTPS): https://tu-tunel.trycloudflare.com
  URL Local:             http://192.168.1.x:8000?setup_token=...

  El token expira en 30 minutos.
  Para regenerar: sudo aegis token
################################################################
```

### macOS (Apple Silicon e Intel)

```bash
curl -fsSL https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.sh | sudo bash
```

El mismo `install.sh` detecta la plataforma y descarga el binario correcto (`macos-arm64` o `macos-x86_64`).

### Windows (x86_64)

Ejecutá PowerShell **como Administrador**:

```powershell
irm https://raw.githubusercontent.com/Gustavo324234/Aegis-Core/main/installer/install.ps1 | iex
```

Aegis se instala como un Servicio de Windows (`AegisOS`) y arranca automáticamente. Gestionalo con comandos estándar de PowerShell:

```powershell
Start-Service AegisOS
Stop-Service AegisOS
Restart-Service AegisOS
Get-Service AegisOS
```

---

## Plataformas soportadas

Se publican binarios pre-compilados para cada commit a `main` (nightly) y cada release con tag:

| Plataforma | Arquitectura | Binario |
|---|---|---|
| Linux | x86_64 | `ank-server-linux-x86_64.tar.gz` |
| Linux | ARM64 | `ank-server-linux-arm64.tar.gz` |
| macOS | Apple Silicon (ARM64) | `ank-server-macos-arm64.zip` |
| macOS | Intel (x86_64) | `ank-server-macos-x86_64.zip` |
| Windows | x86_64 | `ank-server-windows-x86_64.zip` |

Todos los releases están disponibles en [github.com/Gustavo324234/Aegis-Core/releases](https://github.com/Gustavo324234/Aegis-Core/releases).

---

## Aegis CLI

Después de la instalación en **Linux/macOS**, el comando `aegis` está disponible en todo el sistema.

> **Windows:** No se instala CLI separado. Usá PowerShell para gestionar el Servicio de Windows `AegisOS`. Ver [docs/CLI_REFERENCE.md](docs/CLI_REFERENCE.md) para la tabla de equivalencias completa.

### Estado e información

```bash
aegis status          # Salud del servicio y conectividad API
aegis version         # Versión instalada
aegis logs            # Seguir logs en vivo (últimas 100 líneas)
aegis logs 200        # Seguir últimas 200 líneas
aegis diag            # Reporte diagnóstico SRE completo
```

### Control del servicio

```bash
aegis start           # Iniciar el servicio
aegis stop            # Detener el servicio
aegis restart         # Reiniciar el servicio
aegis token           # Imprimir URL de configuración con token fresco
aegis tunnel          # Iniciar manualmente el túnel Cloudflare
```

### Actualizaciones

```bash
aegis update          # Actualizar al último build nightly
aegis update --stable # Actualizar al último release estable
```

Ver [docs/CLI_REFERENCE.md](docs/CLI_REFERENCE.md) para referencia completa y equivalentes Windows.

---

## Compilar desde el código fuente

**Requisitos:** Rust 1.80+, Node.js 20+, `protoc`

```bash
git clone https://github.com/Gustavo324234/Aegis-Core.git
cd Aegis-Core

# Build completo: UI + binario embebido
make build-embed

# Ejecutar
./target/release/ank-server
```

---

## Estructura del Repositorio

```
aegis-core/
├── kernel/          Kernel Rust
│   ├── crates/      Arquitectura Rust modular:
│   │   ├── ank-server       Punto de entrada principal (Axum + gRPC)
│   │   ├── ank-core         Motor cognitivo — scheduler, VCM, agentes, DAG
│   │   ├── ank-http         Servidor HTTP/WebSocket (Axum) con UI React embebida
│   │   ├── ank-cli          CLI administrativa
│   │   ├── ank-mcp          Cliente Model Context Protocol
│   │   ├── aegis-supervisor Process manager basado en Rust
│   │   ├── aegis-sdk        SDK de plugins en Wasm
│   │   └── ank-proto        Contratos Protobuf y stubs de Rust generados
│   └── proto/       Contratos Protobuf (gRPC y protocolo de audio Siren)
├── shell/ui/        Interfaz web — React 18 / Vite / TypeScript / Tailwind
├── app/             Cliente mobile — React Native / Expo (modos Satélite y Cloud)
├── installer/       Deployment — install.sh, install.ps1, aegis CLI, servicio systemd
├── governance/      Tickets, epics activos, docs de arquitectura, codex
└── distro/          (futuro) distribución Linux
```

---

## Hitos Completados

| Epic | Título | Estado |
|---|---|---|
| Epic 32 | Unificación — binario único Rust | ✅ Listo |
| Epic 42 | Realignment — auth, OAuth, router de modelos | ✅ Listo |
| Epic 43 | Orquestación Jerárquica Multi-Agente | ✅ Listo |
| Epic 44 | Developer Workspace (terminal, explorador de archivos, Git, PR manager) | ✅ Listo |
| Epic 45 | Arquitectura de Agentes Cognitivos | ✅ Listo |
| Epic 46 | Lanzamiento Público (docs, comunidad, salud open source) | ✅ Listo |

---

## Roadmap

- [ ] Epic 51 — Inteligencia de Modelos (PinchBench, Ollama Cloud, scoring de contexto CMR v2)
- [ ] Epic 52 — Calidad de Voz (estabilización de Siren audio stream y lógica de silenciar micrófono)
- [ ] Epic 53 — Estabilización (loop real de LLM de agentes, observabilidad y reparaciones de infraestructura)
- [ ] Scripting en Sandbox (Maker Capability) — CORE-150
- [ ] Integración de contexto de proyecto (Git/VCM) — CORE-151
- [ ] App móvil completa (modos Satélite y Cloud)
- [ ] `distro/` — distribución mínima de Linux

---

## Contribuir

Aegis es open source y da la bienvenida a contribuciones.

Leé [CONTRIBUTING.md](CONTRIBUTING.md) para empezar. Se valoran contribuciones de código, documentación, traducciones y reportes de bugs.

El proyecto usa un flujo basado en tickets. Revisá [governance/TICKETS_MASTER.md](governance/TICKETS_MASTER.md) para trabajo abierto.

---

## Apoyar el Proyecto

Aegis es construido y mantenido por un solo desarrollador. Si te resulta útil, considerá apoyar su desarrollo:

- ⭐ **Dale una estrella** — ayuda con la visibilidad
- 🐛 **Reportá bugs** — abrí un issue
- 💬 **Difundilo** — compartilo con personas que construyen sistemas de IA
- ❤️ **Sponsoreá** — [github.com/sponsors/Gustavo324234](https://github.com/sponsors/Gustavo324234)

Los sponsoreos van directamente a la infraestructura de desarrollo: cómputo, costos de API y herramientas.

---

## Filosofía

**Los LLMs son ALUs, no oráculos.** Un modelo de lenguaje es una unidad de cómputo probabilística que transforma tokens. La inteligencia del sistema viene de la capa determinística que orquesta esas transformaciones — el scheduler, la jerarquía de memoria, el árbol de agentes. El modelo es una herramienta, no la mente.

**Cognición a nivel de kernel.** Las cargas de trabajo de IA deberían manejarse igual que un SO maneja procesos: scheduling, aislamiento, límites de recursos, comunicación inter-proceso. No como una llamada de librería, sino como un servicio de kernel.

**Un binario.** La complejidad operacional es deuda técnica. Un sistema que corre como un único ejecutable, sin runtime Python, sin daemon Node, sin Docker requerido, es un sistema que puede mantenerse de verdad.

---

## Licencia

MIT — ver [LICENSE](LICENSE)

Copyright (c) 2026 Gustavo Aversente

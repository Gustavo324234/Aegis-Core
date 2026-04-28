# Aegis OS

> **Un sistema operativo cognitivo.** Un binario. Sin dependencias de runtime. LLMs como ALUs bajo un motor de ejecución determinístico.

[![Licencia: MIT](https://img.shields.io/badge/Licencia-MIT-blue.svg)](LICENSE)
[![Build](https://github.com/Gustavo324234/Aegis-Core/actions/workflows/ci.yml/badge.svg)](https://github.com/Gustavo324234/Aegis-Core/actions)
[![GitHub Sponsors](https://img.shields.io/badge/Sponsor-%E2%9D%A4-pink?logo=github)](https://github.com/sponsors/Gustavo324234)

---

## ¿Qué es Aegis?

Aegis es un sistema operativo cognitivo self-hosted — una plataforma donde los agentes de IA corren como procesos de primera clase, con memoria, scheduling, multi-tenancy y ejecución de herramientas integrados en el kernel.

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
        └── gRPC :50051 — CLI externa, federación multi-nodo
```

El sistema es multi-tenant: cada tenant tiene un entorno cognitivo aislado con sus propias capas de memoria (L1/L2/L3), árbol de agentes y almacenamiento cifrado (SQLCipher).

Ver [ARCHITECTURE.md](ARCHITECTURE.md) para detalle completo.

---

## Instalación Rápida

**Requisitos:** Linux (Ubuntu 22.04+ / Debian 12+), `sudo`, `curl`

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

Abrí la URL en el navegador para completar el onboarding.

---

## Aegis CLI

Después de la instalación, el comando `aegis` está disponible en todo el sistema.

### Estado e información

```bash
aegis status          # Salud del servicio y conectividad API
aegis version         # Versión instalada
aegis logs            # Seguir logs en vivo (últimas 50 líneas)
aegis logs 100        # Seguir últimas 100 líneas
aegis diag            # Reporte diagnóstico SRE completo
```

### Control del servicio

```bash
aegis start           # Iniciar el servicio
aegis stop            # Detener el servicio
aegis restart         # Reiniciar el servicio
aegis token           # Imprimir URL de configuración con token fresco
```

### Actualizaciones

```bash
aegis update          # Actualizar al último release estable
aegis update --beta   # Actualizar al último build nightly (desde main)
aegis update --stable # Apuntar explícitamente al canal estable
```

Ver [docs/CLI_REFERENCE.md](docs/CLI_REFERENCE.md) para referencia completa.

---

## Compilar desde el código fuente

**Requisitos:** Rust 1.80+, Node.js 20+

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
├── kernel/          Kernel Rust — ank-server, ank-core, ank-http, ank-cli
├── shell/ui/        Interfaz web — React 18 / Vite / TypeScript / Tailwind
├── app/             Cliente mobile — React Native / Expo
├── installer/       Deployment — install.sh, aegis CLI, servicio systemd
├── governance/      Tickets, docs de arquitectura, codex
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

---

## Contribuir

Aegis es open source y da la bienvenida a contribuciones.

Leé [CONTRIBUTING.md](CONTRIBUTING.md) para empezar. Se valoran contribuciones de código, documentación, traducciones y reportes de bugs.

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

# Roadmap: De Servidor Aegis a Sistema Operativo Cognitivo Nativo (Aegis OS)

Este documento detalla la visión, arquitectura y fases de implementación para evolucionar el servidor Aegis (`ank-server`) hacia un **Sistema Operativo Cognitivo** nativo ejecutado de forma directa sobre el hardware (*bare-metal*), utilizando una computadora portátil antigua como plataforma física de pruebas.

---

## 1. Visión Estratégica

Aegis OS representa la transición de una aplicación de usuario convencional a una **plataforma de hardware cognitivo**. La meta final es un sistema donde la Inteligencia Artificial (los agentes) no corra como un proceso de fondo en un sistema operativo comercial (Windows/macOS), sino que **el kernel cognitivo controle directamente los recursos del sistema** bajo principios estrictos de inmutabilidad, privacidad y ejecución local.

Para lograr esto con un bajo nivel de fricción, se establece un modelo de **entrega dual**: el mismo motor cognitivo en Rust (`ank-server`), pero desplegado en dos factores de forma diferentes.

---

## 2. Arquitectura de Entrega Dual

El sistema operativo se compilará declarativamente mediante **NixOS** para soportar dos modos de funcionamiento intercambiables:

```
                      ┌─────────────────────────┐
                      │    Aegis OS (NixOS)     │
                      └────────────┬────────────┘
                                   │
                    ┌──────────────┴──────────────┐
                    ▼                             ▼
       [Modo Servidor (Headless)]    [Modo Usuario (Kiosk GUI)]
       - Sin entorno gráfico         - Compositor Wayland (Cage)
       - Acceso remoto (Web/App)     - Navegador en pantalla completa
       - Bajo consumo de RAM         - Audio local mapeado (PipeWire)
```

### A. Modo Servidor (Headless)
Diseñado para convertir la laptop en un nodo de red silencioso y eficiente (Home Server).
* **Características:** Sin servidor gráfico (sin X11 ni Wayland), mínimo consumo de recursos, consola de texto pura.
* **Acceso:** Interfaz Web expuesta en el puerto `8000`, API gRPC en el puerto `50051`, túneles inversos seguros mediante `Aegis Connect`.
* **Comportamiento en Laptop:** Configuración de energía declarativa para que al cerrar la tapa la pantalla se apague, pero el procesador siga ejecutando el bucle de agentes en segundo plano.

### B. Modo Usuario con Interfaz (Kiosk)
Diseñado para convertir la laptop en una consola física interactiva de uso local.
* **Compositor Gráfico Ligero:** Uso de `cage` (un compositor Wayland minimalista que ejecuta una única aplicación en pantalla completa sin barra de tareas ni menús del sistema).
* **Navegador Embebido:** Lanzamiento automático de un navegador (ej: Chromium) en modo quiosco:
  ```bash
  chromium --kiosk --app=http://localhost:8000
  ```
* **Integración de Voz (Siren):** Configuración de **PipeWire** a nivel del sistema operativo para mapear el micrófono e altavoces integrados de la laptop directamente al flujo de audio WebRTC del protocolo Siren, permitiendo comandos por voz nativos sin configuraciones manuales.

---

## 3. Hardware Objetivo: Computadora Portátil Antigua (x86_64)

El prototipado se realizará sobre una laptop antigua. Esta elección proporciona ventajas técnicas significativas frente a placas de desarrollo (como Raspberry Pi):

* **Integración Completa de Sensores:** Cuenta de forma nativa con pantalla, teclado, panel táctil, micrófono, altavoces y webcam. Todo controlado por controladores estables del kernel de Linux.
* **Batería como UPS:** La batería integrada actúa como un respaldo de energía automático, previniendo la corrupción de las bases de datos cifradas ante apagones.
* **Arquitectura x86_64:** Acceso directo a instrucciones vectoriales optimizadas (AVX/AVX2) en la CPU, lo que maximiza la velocidad de ejecución de modelos locales en formato GGUF mediante `llama.cpp`.

---

## 4. Estructura Declarativa en NixOS

La configuración en [distro/nixos/](file:///e:/Aegis/Aegis-Core/distro/nixos) se modularizará para admitir este doble modo de forma limpia:

* **[configuration.nix](file:///e:/Aegis/Aegis-Core/distro/nixos/configuration.nix):** Contiene las políticas de seguridad de Citadel, particiones LUKS2, swap cifrado y el servicio del demonio Aegis en Rust.
* **`profile-server.nix`:** Habilita el firewall, SSH con llaves criptográficas y optimizaciones de ahorro de energía para cuando la tapa de la laptop esté cerrada.
* **`profile-kiosk.nix`:** Habilita los controladores de GPU (Mesa/Intel/AMD), el servidor de audio PipeWire y el compositor Wayland `cage` para la interfaz visual.
* **[flake.nix](file:///e:/Aegis/Aegis-Core/distro/nixos/flake.nix):** Define las configuraciones de salida del sistema para compilar la imagen correspondiente (ej: `#aegis-laptop-server` o `#aegis-laptop-kiosk`).

---

## 5. Fases de Implementación del Roadmap

### Fase 1: Estabilización del Servidor y Lanzamiento del MVP (Completada)
* **Objetivo:** Pulir y lanzar el servidor Aegis (`ank-server`) como un software instalable sobre sistemas operativos existentes (Windows, Linux, macOS).
* **Estado:** **COMPLETO**. Toda la base de código compila limpiamente, se implementó el backend en Rust (`ank-core` / `ank-http`) y la interfaz React (`shell/ui`). Además, se repararon y optimizaron los flujos de integración continua (CI) en GitHub Actions para plataformas nativas (incluyendo Windows con enlaces estáticos de OpenSSL vía vcpkg).

### Fase 2: Virtualización y Generación de la Distro (Completada)
* **Objetivo:** Crear la infraestructura de compilación de NixOS.
* **Estado:** **COMPLETO**. 
  * Se modularizó la distro NixOS agregando el perfil gráfico de quiosco ([profile-kiosk.nix](file:///e:/Aegis/Aegis-Core/distro/nixos/profile-kiosk.nix)) con soporte para Cage Wayland, auto-inicio de Chromium a pantalla completa y configuración de audio local con PipeWire.
  * Se diseñó el entorno declarativo de Machine Learning ([shell.nix](file:///e:/Aegis/Aegis-Core/tools/fine-tuning/shell.nix) y [requirements.txt](file:///e:/Aegis/Aegis-Core/tools/fine-tuning/requirements.txt)) para encapsular de forma limpia dependencias dinámicas como PyTorch, PEFT y drivers OpenGL/NVIDIA.

### Fase 3: Despliegue en Hardware Real - Laptop Vieja (En Desarrollo / Foco Actual)
* **Objetivo:** Instalación física de la distro en la laptop de pruebas.
* **Entregables:** Optimización de la gestión de energía al cerrar la tapa de la laptop (suspensión selectiva del panel gráfico sin apagar el daemon en segundo plano), configuración fina de adaptadores de red y drivers Wi-Fi de hardware real, y pruebas de estabilidad del daemon funcionando 24/7.

### Fase 4: Integración de Sensores como Herramientas (Planeado)
* **Objetivo:** Exponer el hardware de la laptop directamente al kernel de agentes.
* **Entregables:** Desarrollo de herramientas de sistema en Rust (`ank-core`) que permitan a los agentes interactuar con el entorno físico del dispositivo (leer carga de batería, silenciar/desmutear micrófonos, ajustar brillo de la pantalla o tomar fotos de verificación con la webcam local mediante autorización del protocolo Citadel).

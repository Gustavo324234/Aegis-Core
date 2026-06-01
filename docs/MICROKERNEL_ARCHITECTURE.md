# Arquitectura de Microkernel y Módulos Autónomos de Aegis (Propuesta de Diseño)

Este documento define la visión y especificación arquitectónica para transformar a **Aegis OS** en un sistema operativo cognitivo basado en el paradigma de **Microkernel** con **Módulos Extensibles (Aegis Modules / Miniapps)**. 

La idea central es mantener un **Kernel Core** ultra-ligero y seguro que se dedique exclusivamente a tareas fundamentales, permitiendo instalar y desacoplar "Módulos de Dominio" (Negocios, Ciberseguridad, Programación, etc.) que cuenten con sus propias bases de datos, interfaces locales (móviles/computadoras) y herramientas específicas, pero integrados a nivel cognitivo y de datos mediante Aegis.

---

## 🏛️ 1. El Paradigma de Microkernel en Aegis

En los sistemas operativos tradicionales de microkernel (como L4 o QNX), el kernel solo maneja los servicios mínimos indispensables: manejo de memoria, hilos y comunicación entre procesos (IPC). Todos los demás servicios (sistemas de archivos, drivers de red, etc.) corren en el "espacio de usuario" como servidores independientes.

Llevando este concepto a un **Sistema Operativo Cognitivo (Aegis)**:

```mermaid
graph TD
    subgraph "ESPACIO DE USUARIO (Módulos de Dominio / Servidores)"
        Biz[Módulo Negocios: Aegis-Biz<br/>SQLite Local / App Móvil / Escáner]
        Sec[Módulo Ciberseguridad: Aegis-Sec<br/>Herramientas Red Team / Redes]
        Dev[Módulo Programación: Aegis-Dev<br/>Compiladores / Test Runners]
    end

    subgraph "MICROKERNEL CORE (Aegis Core Kernel)"
        Cognitive[Orquestación Cognitiva<br/>CMR / Supervisor / Specialist]
        Security[Seguridad y Aislamiento<br/>Enclaves / Sandbox / Jail]
        IPC[Canal de Comunicación Universal<br/>gRPC / MCP Bridge]
    end

    Biz <-->|gRPC / MCP Protocols| IPC
    Sec <-->|gRPC / MCP Protocols| IPC
    Dev <-->|gRPC / MCP Protocols| IPC

    IPC <--> Cognitive
    Cognitive <--> Security
```

### A. El Microkernel Core (Aegis Kernel)
Se mantiene minimalista, seguro y rápido. Sus únicas responsabilidades son:
1.  **Seguridad y Aislamiento (Sandboxing):** Controlar los enclaves de datos de inquilinos (Tenant Enclaves), límites de recursos (timeouts, CPUs) y jail de sistemas de archivos.
2.  **Orquestación Cognitiva (Cognitive Routing):** El enrutamiento de tareas (CMR) y la gestión del ciclo de vida de los agentes (Chat Agent, Supervisors, Specialists).
3.  **Universal IPC Bridge:** Un canal de comunicación estandarizado para que los módulos se registren y expongan sus herramientas al kernel de forma segura.

### B. Los Módulos de Dominio (Domain Modules)
Son aplicaciones o servicios independientes y autocontenidos que se "instalan" en Aegis. Cada módulo:
*   Tiene su **propia base de datos aislada** (ej. SQLite / SQLCipher cifrada).
*   Tiene sus **propias herramientas especializadas** (ej. wrappers de escáneres, llamadas a APIs externas).
*   Tiene su **propia UI/Cliente independiente** (ej. una app móvil para lectura de códigos de barra o un dashboard web de inventario) que puede correr de forma local sin conexión directa a Aegis.
*   **Se integra a Aegis** registrándose a través de un manifiesto estándar.

---

## 📦 2. Casos de Uso Concretos

### Caso de Uso A: Módulo de Negocios (`Aegis-Biz`)
*   **Independencia:** Tienes una aplicación en tu celular que lee códigos de barras mediante la cámara para registrar ventas y stock de tu tienda de alimentos. Funciona de manera local y rápida con una base de datos SQLite en el dispositivo.
*   **Integración Cognitiva:** El módulo móvil se sincroniza periódicamente con el enclave cifrado de Aegis en tu servidor.
*   **Control por Lenguaje Natural:**
    *   Le dices a Aegis por chat: *"El proveedor de lácteos me trajo 10 quesos hoy a $500 c/u"*.
    *   Aegis Core intercepta el prompt, reconoce que la tarea pertenece al dominio de Negocios, y llama a la herramienta expuesta por `Aegis-Biz`: `update_inventory(product="queso", qty=10, cost=500, supplier="lácteos")`.
    *   Aegis actualiza el stock en la base de datos centralizada, calcula el balance del mes en tu libro contable (ledger), y la app de tu celular se sincroniza reflejando los 10 quesos nuevos inmediatamente.

### Caso de Uso B: Módulo de Ciberseguridad y Red Team (`Aegis-Sec`)
*   **Independencia:** Cuenta con herramientas y scripts especializados en pentesting (Nmap, Metasploit wrappers, rastreadores de vulnerabilidades).
*   **Integración Cognitiva:** Expone herramientas estructuradas a Aegis de forma aislada.
*   **Control Seguro:** Aegis permite lanzar comandos de auditoría ética únicamente bajo un entorno de Sandbox sumamente vigilado, previniendo que un agente tome decisiones destructivas sobre redes ajenas sin autorización explícita.

---

## 🔧 3. Arquitectura Técnica de Integración (¿Cómo interactúan?)

Para que un módulo sea "independiente pero integrable", la comunicación debe estar totalmente desacoplada. Utilizaremos el estándar de la industria **Model Context Protocol (MCP)** creado por Anthropic, combinado con interfaces **gRPC** de alto rendimiento.

### 1. El Manifiesto del Módulo (`module.json`)
Cada módulo se define mediante un archivo de configuración que le dice a Aegis qué herramientas aporta y cómo comunicarse:

```json
{
  "module_id": "aegis.domain.business",
  "display_name": "Aegis Business & Store Manager",
  "version": "1.0.0",
  "ipc_transport": {
    "protocol": "gRPC",
    "endpoint": "localhost:50071"
  },
  "database": {
    "driver": "sqlite",
    "encryption": true
  },
  "exposed_tools": [
    {
      "name": "biz_add_product",
      "description": "Register a new product in the store database",
      "parameters": {
        "type": "object",
        "properties": {
          "barcode": { "type": "string" },
          "name": { "type": "string" },
          "price": { "type": "number" }
        },
        "required": ["name", "price"]
      }
    },
    {
      "name": "biz_update_stock",
      "description": "Update the inventory stock count for a specific item",
      "parameters": {
        "type": "object",
        "properties": {
          "barcode": { "type": "string" },
          "name": { "type": "string" },
          "quantity_change": { "type": "integer" }
        },
        "required": ["quantity_change"]
      }
    }
  ]
}
```

### 2. Flujo de Descubrimiento y Ejecución Dinámica

```
[Usuario] ────> ( "Agrega 10 quesos al stock" )
                       │
                       ▼
               [Aegis Core Kernel] 
                       │
                       ├─(1) Revisa registro de módulos instalados (Aegis-Biz está activo)
                       ├─(2) Inyecta dinámicamente las 'exposed_tools' de Aegis-Biz al LLM
                       │
                       ▼
               [Modelo de Lenguaje (LLM)]
                       │
                       ├─(3) Razona y genera llamada a herramienta:
                       │     "biz_update_stock" { "name": "queso", "quantity_change": 10 }
                       │
                       ▼
               [Aegis Core Kernel]
                       │
                       ├─(4) Intercepta llamada de herramienta
                       ├─(5) La redirige vía gRPC/IPC al proceso autónomo de [Aegis-Biz]
                       │
                       ▼
                 [Módulo Aegis-Biz]
                       │
                       ├─(6) Modifica localmente su base de datos SQLite cifrada
                       ├─(7) Retorna confirmación JSON de éxito al Kernel
                       │
                       ▼
[Usuario] <──── ( "¡Listo! Se añadieron 10 quesos al inventario..." )
```

---

## 🖥️ 4. Interfaces Dinámicas basadas en Servidor (Server-Driven UI / SDUI)

Para habilitar a los módulos independientes a renderizar interfaces gráficas interactivas en el shell sin necesidad de recompilar el cliente React o la app móvil, implementamos el estándar de **Server-Driven UI (SDUI)**.

### A. Estructura Declarativa en el Manifiesto (`module.json`)
Los manifiestos ahora pueden declarar esquemas de vistas visuales mediante el arreglo `ui_views`. Cada vista especifica el formulario, los campos requeridos, su tipo (`text`, `number`), placeholders y la herramienta gRPC final a invocar:

```json
"ui_views": [
  {
    "view_id": "add_product_form",
    "type": "form",
    "title": "Registrar Producto",
    "description": "Dar de alta un nuevo producto en el catálogo general",
    "icon": "Plus",
    "tool_name": "biz_add_product",
    "fields": [
      { "name": "barcode", "label": "Código de Barras", "type": "text", "placeholder": "Ej: 779...", "required": false },
      { "name": "name", "label": "Nombre del Producto", "type": "text", "placeholder": "Ej: Queso Gouda", "required": true },
      { "name": "price", "label": "Precio Unitario ($)", "type": "number", "placeholder": "Ej: 450", "required": true }
    ]
  }
]
```

### B. Endpoints de Enrutamiento REST en el Servidor Axum (`router_api.rs`)
Para enlazar la interfaz y el microkernel, expusimos tres endpoints dedicados bajo `/api/router`:
1. **`GET /api/router/modules`**: Retorna el catálogo de módulos y sus esquemas de vistas `ui_views`, además de consultar el enclave criptográfico (`TenantDB`) mediante credenciales Zero-Trust (`CitadelAuthenticated`) para resolver si el módulo está activo para el tenant actual (`module_active:<module_id>`).
2. **`POST /api/router/modules/:module_id/enable`**: Habilita o deshabilita la persistencia cifrada del módulo en `TenantDB`.
3. **`POST /api/router/modules/:module_id/execute`**: Recibe argumentos ingresados manualmente desde la interfaz, valida que el módulo esté activo para el inquilino, abre un canal gRPC con el proceso del módulo y ejecuta la acción en el ledger correspondiente.

### C. Panel React Premium (`DynamicModulePanel.tsx`)
Un componente de UI interactivo, montado en el dashboard general, que realiza las siguientes funciones:
* **Catálogo de Conmutación:** Panel lateral que lista los módulos descubiertos, sus versiones y un slider animado con Framer Motion para activarlos/desactivarlos en tiempo real.
* **Form Builder Dinámico:** Genera de manera reactiva formularios con validaciones completas basadas en el esquema del backend.
* **Consola de Resultados:** Panel de salida donde se imprimen los registros formateados en JSON retornados por el microkernel.

---

## 🗺️ 5. Roadmap de Implementación y Estado de Avance

### ├─ Fase 1: Protocolo de Descubrimiento Dinámico (100% Completado)
* **Estado:** Totalmente integrado y verificado.
* **Detalle:** El cargador de Rust escanea dinámicamente `/kernel/modules`, registra los archivos `module.json` y el `CognitiveRouter` inyecta las herramientas semánticamente en el prompt del sistema.

### ├─ Fase 2: Puente de Comunicación IPC/gRPC (100% Completado)
* **Estado:** Totalmente integrado y verificado.
* **Detalle:** Redireccionamiento universal de llamadas a herramientas (`SYS_MCP_EXEC`) mediante gRPC (`tonic`) utilizando el contrato proto de `ank-proto`.

### ├─ Fase 4: Interfaces Locales y Vistas Dinámicas - SDUI (100% Completado)
* **Estado:** Totalmente integrado y verificado en producción.
* **Detalle:** Renderizado reactivo en el Dashboard de React (`shell/ui`), API de conmutación en base de datos cifrada Zero-Trust (`TenantDB`) y llamadas gRPC manuales desde la interfaz con 0 warnings/errores de TypeScript.

### └─ Fase 3: Sincronización Semisíncrona y Replicación (En Desarrollo)
* **Estado:** Próximo paso prioritario.
* **Detalle:** Diseñar la API delta de réplica parcial del Ledger cifrado de inquilino para permitir a las aplicaciones externas satélites operar localmente offline y sincronizarse en caliente al recuperar conexión.

---

## 🎯 Conclusión
Esta arquitectura transforma a Aegis de ser un asistente conversacional a consolidarse como un verdadero **Ecosistema de Aplicaciones Inteligentes e Interconectadas (Aegis OS)**. La implementación del Server-Driven UI (SDUI) permite escalar las capacidades visuales de forma descentralizada y segura manteniendo la privacidad criptográfica provista por Citadel.

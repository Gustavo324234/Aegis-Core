# EPIC 54 — Aegis Connect: Persistent WebSocket Tunneling

**Estado:** ✅ Completa

## 🎯 Objetivo

Implementar una plataforma de túneles y gestión de conexiones propia y segura para los usuarios de Aegis OS. 

En lugar de depender de Cloudflare Quick Tunnels (que generan URLs aleatorias y efímeras que cambian en cada reinicio), Aegis Connect proporciona a cada usuario una **URL permanente, legible y altamente segura** (ej. `tu-usuario.aegis.orioncrea.com`) vinculada directamente a su cuenta de **Orion ID**.

---

## 🏗️ Arquitectura de Conectividad

Aegis Connect funciona bajo un modelo de **túnel inverso persistente de dos extremos** que no requiere redirección de puertos (port forwarding) ni configuraciones complejas de red local:

```
Navegador / App Móvil ────► HTTPS ────► aegis.orioncrea.com (Relay Proxy)
                                                    │
                                          WebSocket persistente (Túnel)
                                                    ▼
                                            Aegis OS (Local Host)
```

1.  **Relay Proxy Server (`aegis.orioncrea.com`):** Un servicio desplegado en VPS (Hostinger) que recibe conexiones HTTPS del exterior, valida la autenticación multi-inquilino del Protocolo Citadel y mantiene streams WebSocket abiertos y persistentes con cada cliente registrado.
2.  **Connect Agent (Kernel Crate `ank-http`):** Un agente integrado en el kernel de Aegis OS que se conecta por WebSocket inverso hacia el Relay. Una vez autenticado, el agente encapsula y multiplexa todo el tráfico REST y WebSocket de la UI local a través de ese único túnel de comunicación.
3.  **Mapeo Orion ID:** El subdominio asignado al túnel del usuario (ej. `john.aegis.orioncrea.com`) se autentica y enlaza dinámicamente mediante el token del usuario emitido por la plataforma de identidad Orion ID.

---

## 🛠️ Detalle de Implementación

### CORE-307 — Protocolo de Túnel y Servidor de Relay
*   Desarrollo de un servidor proxy reverso multiplexor de alto rendimiento escrito en Rust.
*   Enrutamiento dinámico de requests entrantes: mapea el prefijo de host HTTP (ej: `john`) a la conexión activa de túnel correspondiente.
*   Gestión de sesiones concurrentes y aislamiento de tráfico a nivel binario utilizando buffers asíncronos eficientes.

### CORE-308 — Cliente del Túnel (Kernel Agent)
*   Integración del agente de túnel dentro de la inicialización de `ank-server` como un hilo background administrado.
*   **Heartbeat Activo:** Envío periódico de tramas ping/pong (cada 15 segundos) para asegurar que los routers intermediarios no cierren la conexión por inactividad.
*   **Reconexión Exponencial Automática:** Si la conexión de red se interrumpe, el agente realiza reintentos automáticos aplicando un retraso con retroceso exponencial (1s, 2s, 4s, 8s... hasta un máximo de 60s) para evitar sobrecargar el Relay.
*   Enlace seguro de tokens Citadel y cifrado de los payloads del túnel.

### CORE-309 — UI e Integración de Zustand (Shell)
*   **Settings Widget:** Agregado de un widget de estado interactivo en la pestaña **Ajustes (Settings) → Conexión** de la interfaz web.
*   **Orion ID Linkage:** Formulario para vincular y desvincular la instancia local con una cuenta Orion ID ingresando un token de acceso seguro.
*   **Feedback en Tiempo Real:** Muestra dinámica del estado del túnel (*Desconectado / Conectando / Conectado*) y el subdominio permanente asignado.

---

## 🔒 Seguridad (Citadel Hardening)

*   **Autenticación Mutua:** El túnel solo se establece si el token Orion ID provisto coincide con la firma del par de llaves registrado en el Relay Server.
*   **Cifrado TLS End-to-End:** Todo el tráfico entre el navegador externo del usuario y la instancia local de Aegis viaja cifrado bajo HTTPS/WSS provisto por Cloudflare SSL.
*   **Aislamiento:** El Relay no inspecciona ni puede descifrar los datos cifrados de bases de datos `aegis.db` que fluyen en las conexiones, ya que el cifrado final Citadel por tenant se realiza en local.

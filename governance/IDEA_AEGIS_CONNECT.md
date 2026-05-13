# IDEA — Aegis Connect: Plataforma de Conectividad Propia

**Tipo:** idea / epic futura  
**Origen:** conversación con Tavo, 2026-05-12  
**Estado:** Registrada — no planificada

---

## Visión

Plataforma propia de tunneling y gestión de conexiones para usuarios de Aegis.
En lugar de depender de Cloudflare Quick Tunnels (URLs efímeras y aleatorias),
cada usuario tiene una URL permanente y legible gestionada desde una cuenta en
`aegis.orioncrea.com`.

Referencia conceptual: Tailscale + ngrok.

---

## Flujo objetivo

```
Usuario final → aegis.orioncrea.com/u/john
                        ↓
                Relay Server (VPS Hostinger)
                        ↓
            WebSocket Tunnel persistente
                        ↓
                Aegis en casa de John
```

---

## Componentes requeridos

### VPS — Relay + Plataforma web
- Servicio de registro/autenticación de usuarios
- Relay server: mantiene tunnels WebSocket persistentes entre clientes Aegis y navegadores
- Router: mapea `usuario` → conexión activa
- Dashboard web: estado de instancia, URL asignada, configuración

### Aegis (cliente) — Agente de tunnel
- Agente que abre y mantiene conexión persistente hacia el relay
- Autenticación con la plataforma central (token por usuario)
- Heartbeat + reconexión automática
- UI en Settings: vincular/desvincular cuenta, ver URL pública asignada

### DNS / SSL
- `*.aegis.orioncrea.com` o `aegis.orioncrea.com/u/:usuario`
- SSL wildcard vía Cloudflare o Let's Encrypt
- `orioncrea.com` ya en Cloudflare — ventaja

---

## Por qué es un Epic separado

No es una feature del kernel ni del installer — es infraestructura de plataforma nueva
con backend propio en el VPS, posiblemente en un repositorio o servicio separado.
Scope mínimo estimado: 4-6 semanas de desarrollo.

---

## Próximo paso (cuando se planifique)

Definir si el relay vive como servicio independiente en el VPS o como parte de Aegis-Core.
Evaluar librerías existentes (frp, rathole) vs. implementación propia en Rust/Node.


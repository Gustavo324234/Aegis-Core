# CORE-296 — Installer: puerto HTTP configurable + soporte entorno multi-servicio

**Tipo:** feat  
**Prioridad:** Crítica  
**Épica:** Infraestructura  
**Asignado a:DevOps Engineer
**Componente:** `installer/install.sh`, `installer/.env.example`

**Estado:** ✅ Done
---

## Contexto

El installer asume que el puerto `8000` está disponible y lo hardcodea en múltiples lugares
(health check, Caddyfile, mensaje final). En entornos VPS con servicios preexistentes —
el caso concreto es un VPS Debian con nginx-proxy-manager en `:80`/`:443` y una app
ocupando `:8000`— la instalación se completa sin error pero Aegis queda inutilizable.

El kernel **ya soporta** puerto configurable vía `ANK_HTTP_PORT` (`ank-http/src/config.rs`
línea 27). El problema está íntegramente en el installer.

### Estado real del VPS objetivo

```
:80   → nginx-proxy-manager (docker)   ← no tocar
:443  → nginx-proxy-manager (docker)   ← no tocar
:8000 → relevando-app (docker)         ← CONFLICTO
:8001 → simulador_app (docker)         ← ocupado
:5432 → postgres x2 (internos)
:50051 → libre                         ← gRPC interno Aegis, sin conflicto
```

El servidor ya tiene **nginx-proxy-manager (NPM)** funcionando como reverse proxy.
Aegis debe integrarse como un proxy host adicional en NPM, **no instalar Caddy**.

---

## Cambios requeridos

### 1. `run_system_audit()` — detectar y preguntar puerto

La función existe pero **nunca se invoca** en el flujo principal (`main` salta directo a
`show_main_menu`). Corregir el flujo y expandir la detección:

```bash
# En run_system_audit(), reemplazar el bloque de port check actual:
detect_http_port() {
    local candidate=8000
    if ss -tulpn 2>/dev/null | grep -q ":${candidate} "; then
        warn "Puerto ${candidate} en uso."
        echo ""
        echo -e "${YELLOW}¿Qué puerto HTTP debe usar Aegis?${NC}"
        echo "  Puertos en uso detectados: $(ss -tulpn 2>/dev/null | grep LISTEN | grep -oP ':\K\d+(?= )' | sort -n | tr '\n' ' ')"
        read -rp "Puerto HTTP para Aegis [default: 8080]: " user_port
        candidate="${user_port:-8080}"
        # Verificar que el candidato también esté libre
        while ss -tulpn 2>/dev/null | grep -q ":${candidate} "; do
            warn "Puerto ${candidate} también en uso."
            read -rp "Elegí otro puerto: " candidate
        done
    fi
    AEGIS_HTTP_PORT="$candidate"
    log "Puerto HTTP Aegis: ${AEGIS_HTTP_PORT}"
}
```

Invocar `detect_http_port` dentro de `run_system_audit`, y llamar `run_system_audit`
en el `main` **antes** de `show_main_menu`.

### 2. `show_main_menu()` — preguntar si hay reverse proxy existente

```bash
echo ""
echo -e "${YELLOW}--- REVERSE PROXY ---${NC}"
echo "¿Este servidor ya tiene un reverse proxy (nginx, Caddy, NPM, Traefik)?"
echo "  [1] No — Aegis instala y configura Caddy automáticamente"
echo "  [2] Sí — Solo expongo Aegis en un puerto interno, yo configuro el proxy"
read -rp "Selección [1-2, default 2]: " proxy_choice
case "${proxy_choice:-2}" in
    1) SETUP_CADDY="true" ;;
    *) SETUP_CADDY="false" ;;
esac
```

Si `SETUP_CADDY="false"`, no instalar ni configurar Caddy aunque `SETUP_HTTPS="true"`.
Mostrar en el resumen final las instrucciones para agregar el proxy host en NPM.

### 3. `install_native()` — escribir `ANK_HTTP_PORT` en el `.env`

En el bloque de generación del `ENV_FILE`, agregar:

```bash
echo "ANK_HTTP_PORT=${AEGIS_HTTP_PORT:-8000}" >> "$ENV_FILE"
```

### 4. `wait_and_show()` — usar el puerto real

Reemplazar todas las referencias hardcodeadas a `:8000` por `${AEGIS_HTTP_PORT:-8000}`:

```bash
# health check
curl ... "http://localhost:${AEGIS_HTTP_PORT:-8000}/health"

# token URL
echo "http://${ip}:${AEGIS_HTTP_PORT:-8000}?setup_token=${token}"

# acceso local
echo "http://${ip}:${AEGIS_HTTP_PORT:-8000}"
```

### 5. `configure_caddy()` — usar el puerto real

```bash
configure_caddy() {
    local domain="$1"
    local email="$2"
    local port="${AEGIS_HTTP_PORT:-8000}"
    # Reemplazar localhost:8000 por localhost:${port} en el Caddyfile
    ...
}
```

### 6. Mensaje final para entorno NPM/nginx existente

Si `SETUP_CADDY="false"`, mostrar al final:

```
  Aegis corriendo en: http://localhost:XXXX
  Para exponerlo públicamente, agregá un Proxy Host en tu reverse proxy:
    → Forward Hostname: localhost
    → Forward Port:     XXXX
    → Websockets:       habilitado (requerido para chat y agentes)
```

### 7. `installer/.env.example` — documentar la variable

```bash
# Puerto HTTP del servidor Aegis (default: 8000)
# Cambiar si el puerto 8000 está ocupado por otro servicio
ANK_HTTP_PORT=8000
```

---

## Criterios de aceptación

- [ ] Si el puerto `8000` está ocupado, el installer detecta el conflicto y pregunta al usuario qué puerto usar antes de continuar.
- [ ] El puerto elegido se escribe como `ANK_HTTP_PORT=XXXX` en `/etc/aegis/aegis.env`.
- [ ] El health check en `wait_and_show` usa el puerto configurado, no `:8000` hardcodeado.
- [ ] La URL final mostrada al usuario refleja el puerto real.
- [ ] Si el usuario indica que ya tiene un reverse proxy, el installer no instala Caddy.
- [ ] El mensaje final incluye instrucciones para configurar el proxy host (NPM/nginx/Caddy manual).
- [ ] `installer/.env.example` documenta `ANK_HTTP_PORT`.
- [ ] Probado en Debian con puerto `8000` ocupado — Aegis levanta en el puerto alternativo sin intervención manual post-instalación.

---

## Dependencias

- Ninguna de kernel — `ANK_HTTP_PORT` ya está implementado en `ank-http/src/config.rs`.
- CORE-255 es independiente (Windows only).

---

## Notas

El VPS objetivo tiene **nginx-proxy-manager** manejando 80/443. La integración correcta
es: Aegis en un puerto interno (ej. `8080`) + un Proxy Host en NPM apuntando al dominio
deseado → `localhost:8080`. WebSockets deben estar habilitados en NPM para que el chat
y los agentes funcionen.

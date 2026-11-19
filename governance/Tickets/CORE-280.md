# CORE-280 — Installer: Caddy HTTPS automático

**Tipo:** feat  
**Prioridad:** Alta  
**Asignado a:** Kernel Engineer (modificación del installer)  
**Componentes:**
- `installer/install.sh`
- `installer/aegis.service` (o servicio de Caddy separado)
**Estado:** ✅ Completo

---

## Contexto

El browser bloquea `getUserMedia()` (micrófono) y `Web Speech API` en contextos HTTP
no-localhost. Los usuarios que acceden por IP o dominio sin HTTPS no pueden usar voz.

Cloudflare tunnel temporal resuelve esto pero tiene timeouts agresivos y URLs que
cambian en cada restart. La solución correcta es HTTPS real en el servidor del usuario.

Caddy es un servidor web que obtiene y renueva certificados Let's Encrypt
automáticamente con un solo comando. El installer debe ofrecerlo como opción.

---

## Cambios requeridos

### 1. Pregunta en el installer: ¿configurar HTTPS?

En `show_main_menu()`, agregar después de la selección de modo:

```bash
echo ""
echo -e "${YELLOW}--- ACCESO HTTPS (recomendado para voz y acceso remoto) ---${NC}"
echo "  [1] Sin HTTPS — solo acceso local en http://IP:8000 [DEFAULT]"
echo "  [2] Con HTTPS — requiere un dominio apuntando a este servidor"
echo "      (Let's Encrypt automático via Caddy)"
read -rp "Selección [1-2]: " https_choice
case "${https_choice:-1}" in
    2) SETUP_HTTPS="true" ;;
    *) SETUP_HTTPS="false" ;;
esac

if [[ "$SETUP_HTTPS" == "true" ]]; then
    read -rp "Dominio (ej: aegis.midominio.com): " AEGIS_DOMAIN
    read -rp "Email para Let's Encrypt (para notificaciones de renovación): " AEGIS_EMAIL
fi
```

### 2. Instalación de Caddy

Nueva función `install_caddy()`:

```bash
install_caddy() {
    if command -v caddy &>/dev/null; then
        log "Caddy ya instalado — omitiendo"
        return
    fi
    log "Instalando Caddy..."
    apt-get install -y debian-keyring debian-archive-keyring apt-transport-https -qq >> "$LOG_FILE" 2>&1
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
        | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
        | tee /etc/apt/sources.list.d/caddy-stable.list
    apt-get update -qq >> "$LOG_FILE" 2>&1
    apt-get install -y caddy -qq >> "$LOG_FILE" 2>&1
    success "Caddy instalado"
}
```

### 3. Caddyfile para reverse proxy

Función `configure_caddy()`:

```bash
configure_caddy() {
    local domain="$1"
    local email="$2"

    cat > /etc/caddy/Caddyfile <<EOF
{
    email ${email}
}

${domain} {
    reverse_proxy localhost:8000

    # WebSocket support
    @websockets {
        header Connection *Upgrade*
        header Upgrade websocket
    }
    reverse_proxy @websockets localhost:8000
}
EOF

    systemctl enable caddy
    systemctl restart caddy
    success "Caddy configurado para ${domain}"
    success "HTTPS automático via Let's Encrypt"
}
```

**Importante:** Caddy maneja WebSockets automáticamente al hacer `reverse_proxy` —
no requiere configuración especial adicional. Los timeouts de Caddy para WebSockets
son configurables pero el default (no cierra conexiones idle) es correcto.

### 4. Agregar al `ENV_FILE`

Si HTTPS está configurado:

```bash
echo "AEGIS_DOMAIN=${AEGIS_DOMAIN}" >> "$ENV_FILE"
echo "AEGIS_BASE_URL=https://${AEGIS_DOMAIN}" >> "$ENV_FILE"
```

### 5. Actualizar `wait_and_show()`

Si `SETUP_HTTPS=true`, mostrar la URL HTTPS en lugar de la HTTP:

```bash
if [[ "$SETUP_HTTPS" == "true" && -n "$AEGIS_DOMAIN" ]]; then
    echo -e "  Acceso HTTPS: ${CYAN}https://${AEGIS_DOMAIN}${NC}"
    echo -e "  Certificado: Let's Encrypt (renovación automática)"
else
    echo -e "  Acceso local: ${CYAN}http://${ip}:8000${NC}"
    echo -e "  Para HTTPS: reinstalá con opción [2] en la configuración de acceso"
fi
```

### 6. Deshabilitar Cloudflare tunnel si Caddy está activo

Si `SETUP_HTTPS=true`, no instalar/iniciar cloudflared — no hace falta:

```bash
if [[ "$SETUP_HTTPS" != "true" ]]; then
    install_cloudflared
fi
```

---

## Requisitos del usuario para usar HTTPS

Para que Let's Encrypt funcione, el usuario necesita:
1. Un dominio apuntando a su IP pública (registro A en el DNS)
2. Puerto 80 y 443 abiertos en el router (port forwarding hacia el servidor)

El installer debe mostrar estos requisitos **antes** de pedir el dominio:

```bash
echo ""
echo -e "${YELLOW}Requisitos para HTTPS:${NC}"
echo "  1. Tu dominio debe apuntar a tu IP pública: $(curl -s ifconfig.me 2>/dev/null || echo 'desconocida')"
echo "  2. Puertos 80 y 443 deben estar abiertos en tu router"
echo ""
read -rp "¿Cumplís estos requisitos? [s/N]: " ready
[[ "$ready" != "s" && "$ready" != "S" ]] && SETUP_HTTPS="false"
```

---

## Criterios de aceptación

- [x] El installer pregunta si el usuario quiere HTTPS
- [x] Si elige HTTPS, instala Caddy y configura el Caddyfile con el dominio ingresado
- [x] Caddy obtiene el certificado Let's Encrypt automáticamente al iniciar
- [x] WebSockets funcionan a través de Caddy sin desconexiones por timeout
- [x] Si HTTPS está configurado, cloudflared no se instala
- [x] El mensaje final del installer muestra la URL correcta según la configuración elegida
- [x] Si el usuario elige sin HTTPS, el comportamiento actual no cambia

## Dependencias

- Ninguna — cambio aislado en `installer/install.sh`

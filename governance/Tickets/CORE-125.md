# CORE-125 — Fix: Micrófono bloqueado — Siren requiere HTTPS

**Status:** TODO — DevOps Engineer

## Síntoma

El botón de micrófono no funciona en producción. El browser bloquea
`navigator.mediaDevices.getUserMedia()` en contextos no seguros (HTTP).

## Causa

La API `getUserMedia` del browser requiere HTTPS o `localhost` por política
de seguridad de todos los browsers modernos. El servidor corre en HTTP
(`http://192.168.1.6:8000`) sin TLS.

## Solución recomendada — Caddy como reverse proxy (más simple)

Caddy genera certificados TLS automáticamente (Let's Encrypt o self-signed).

### Opción A — IP local con self-signed cert (sin dominio)

```bash
# 1. Instalar Caddy
apt install -y caddy

# 2. Crear Caddyfile
cat > /etc/caddy/Caddyfile << 'EOF'
{
    auto_https off
}

:443 {
    tls internal
    reverse_proxy localhost:8000
}
EOF

# 3. Reiniciar
systemctl restart caddy
```

El browser va a mostrar advertencia de cert self-signed la primera vez.
El usuario acepta una vez y el micrófono funciona.

### Opción B — Dominio real con Let's Encrypt (recomendado para producción pública)

```
tu-dominio.com {
    reverse_proxy localhost:8000
}
```

Caddy obtiene y renueva el cert automáticamente.

### Opción C — Variables de entorno en ank-server (si se quiere TLS nativo)

El Kernel ya soporta TLS via `AEGIS_TLS_CERT` y `AEGIS_TLS_KEY`:

```bash
# En /etc/aegis/aegis.env
AEGIS_TLS_CERT=/etc/aegis/cert.pem
AEGIS_TLS_KEY=/etc/aegis/key.pem
```

Generar self-signed:
```bash
openssl req -x509 -newkey rsa:4096 -keyout /etc/aegis/key.pem \
  -out /etc/aegis/cert.pem -days 365 -nodes \
  -subj "/CN=aegis-local"
chown aegis:aegis /etc/aegis/*.pem
chmod 640 /etc/aegis/*.pem
```

## Acceptance criteria

- [ ] La UI se sirve por HTTPS
- [ ] El botón de micrófono activa `getUserMedia` sin bloqueo del browser
- [ ] El WebSocket usa `wss://` (el store ya lo hace automáticamente por `window.location.protocol`)
- [ ] El instalador documenta u ofrece configurar TLS en el paso final

## Nota

El store de Zustand ya maneja `wss://` automáticamente:
```typescript
const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
```
No hay cambios en el frontend necesarios.

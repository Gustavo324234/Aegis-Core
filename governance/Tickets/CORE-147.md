# CORE-147 — Fix: TLS simplificado — Cloudflare Tunnel como HTTPS principal

**Epic:** 41 — UX & Onboarding
**Repo:** Aegis-Core — `installer/` + `kernel/`
**Tipo:** fix
**Prioridad:** CRÍTICA
**Asignado a:** Kernel Engineer

---

## Decisión arquitectónica (ADR-047)

**Cloudflare Tunnel reemplaza al certificado self-signed como solución de HTTPS.**

| | Self-signed (anterior) | Cloudflare Tunnel (nuevo) |
|---|---|---|
| Candado verde en browser | ❌ | ✅ |
| Micrófono funciona | ❌ | ✅ |
| Acceso desde otra red | ❌ | ✅ |
| Configuración extra | Ninguna | Ninguna |
| Costo | Gratis | Gratis |

El servidor Aegis sirve **HTTP puro** en `:8000` internamente.
Cloudflare pone el HTTPS por fuera vía tunnel.
Sin certificados, sin warnings del browser, sin `thisisunsafe`.

---

## Cambios requeridos

### 1. `installer/install.sh` — Instalar cloudflared, eliminar TLS self-signed

**Eliminar** `setup_tls_automatic()` y todas las referencias a `ENABLE_TLS`, `cert.pem`, `key.pem`.

**Agregar** `install_cloudflared()` en `install_dependencies()`:

```bash
install_cloudflared() {
    if command -v cloudflared &>/dev/null; then
        log "cloudflared ya instalado — omitiendo"
        return
    fi
    log "Instalando cloudflared (acceso remoto HTTPS)..."
    local arch_str="amd64"
    [[ "$(uname -m)" == "aarch64" ]] && arch_str="arm64"
    local url="https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-${arch_str}"
    if curl -L --fail --silent "$url" -o /usr/local/bin/cloudflared; then
        chmod +x /usr/local/bin/cloudflared
        success "cloudflared instalado → acceso remoto HTTPS habilitado"
    else
        warn "No se pudo instalar cloudflared — acceso solo por red local"
    fi
}
```

**En `install_native()`**, en la generación del env file, eliminar las líneas de TLS:
```bash
# ELIMINAR estas líneas:
# echo "AEGIS_TLS_CERT=..."
# echo "AEGIS_TLS_KEY=..."
```

**En `wait_and_show()`**, cambiar el protocolo siempre a `http`:
```bash
# El acceso local es HTTP — Cloudflare Tunnel provee HTTPS externamente
local PROTOCOL="http"
```

### 2. `installer/aegis` — Simplificar cmd_update, eliminar tls-regen

**En `cmd_update()`**, eliminar el bloque de verificación de TLS.
Agregar instalación de cloudflared si no está:

```bash
# Dentro de cmd_update(), antes de arrancar el servicio:
if ! command -v cloudflared &>/dev/null; then
    printf "  → Instalando cloudflared...\n"
    local arch_str="amd64"
    [[ "$(uname -m)" == "aarch64" ]] && arch_str="arm64"
    curl -L --fail --silent \
        "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-${arch_str}" \
        -o /usr/local/bin/cloudflared \
        && chmod +x /usr/local/bin/cloudflared \
        && printf '%b  → cloudflared instalado%b\n' "$GREEN" "$NC" \
        || printf '%b  → cloudflared no disponible — acceso solo LAN%b\n' "$YELLOW" "$NC"
fi
```

**Eliminar** el comando `tls-regen` del case y del help — ya no aplica.

**Actualizar el help:**
```
update           Update to latest nightly build (installs cloudflared if missing)
```

### 3. `ank-server/main.rs` — Arrancar en HTTP puro

El servidor ya arranca en HTTP cuando no hay vars `AEGIS_TLS_CERT`/`AEGIS_TLS_KEY`.
Verificar que el env file del servidor no las tenga.

**En el servidor de producción:**
```bash
# Limpiar las vars de TLS del env file:
sudo sed -i '/AEGIS_TLS_CERT/d' /etc/aegis/aegis.env
sudo sed -i '/AEGIS_TLS_KEY/d' /etc/aegis/aegis.env
sudo systemctl restart aegis
```

### 4. Log informativo al arrancar

En `main.rs`, reemplazar el warning de TLS por un mensaje informativo:

```rust
info!("🌐 Aegis serving HTTP on port 8000");
info!("   For HTTPS access: cloudflared tunnel --url http://localhost:8000");
info!("   Or run: sudo aegis tunnel");
```

---

## Criterios de aceptación

- [ ] `shellcheck installer/aegis installer/install.sh`
- [ ] `cargo build --workspace` sin errores
- [ ] Nueva instalación: arranca en HTTP, cloudflared instalado automáticamente
- [ ] `aegis update` instala cloudflared si no está
- [ ] El servidor NO intenta cargar certificados TLS por defecto
- [ ] Los logs al arrancar muestran el mensaje de HTTP + instrucción del tunnel

---

## Commit message

```
fix(installer,ank-server): CORE-147 Cloudflare tunnel replaces self-signed TLS — HTTP internally, HTTPS via tunnel
```

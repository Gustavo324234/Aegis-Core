# CORE-147 — Fix: TLS no levanta tras `aegis update`

**Epic:** 41 — UX & Onboarding
**Repo:** Aegis-Core — `installer/`
**Tipo:** fix
**Prioridad:** CRÍTICA — HTTPS no funciona tras update
**Asignado a:** DevOps Engineer (o Kernel Engineer)

---

## Diagnóstico

El comando `aegis update --beta` descarga el nuevo binario pero **no regenera ni
verifica el certificado TLS**. El resultado: el servidor arranca en HTTP aunque
el env file tenga `AEGIS_TLS_CERT` configurado, porque:

1. El update no verifica si `cert.pem` y `key.pem` existen antes de arrancar
2. Si el servidor ya tenía TLS en el env file pero el cert expiró o fue borrado,
   arranca igualmente en HTTP sin avisar
3. El SAN del certificado tiene la IP del momento de instalación — si la IP cambió,
   el browser rechaza el certificado aunque el servidor lo cargue

**Verificación del problema actual en producción:**
```bash
# Verificar si el cert existe
ls -la /etc/aegis/cert.pem /etc/aegis/key.pem

# Verificar si el env file tiene las vars
grep TLS /etc/aegis/aegis.env

# Verificar si el servidor arrancó con TLS
sudo journalctl -u aegis -n 50 | grep -i tls
```

---

## Fix 1 — `installer/aegis` — cmd_update regenera certificado

En `cmd_update()`, después de instalar el nuevo binario y antes de arrancar el servicio,
verificar y regenerar el certificado si es necesario:

```bash
# Dentro de cmd_update(), antes de "systemctl start":

# 1. Verificar si TLS está configurado en el env file
if grep -q "AEGIS_TLS_CERT" /etc/aegis/aegis.env 2>/dev/null; then
    local cert_path
    cert_path=$(grep "AEGIS_TLS_CERT" /etc/aegis/aegis.env | cut -d= -f2)

    # 2. Verificar si el cert existe y es válido
    if [[ ! -f "$cert_path" ]]; then
        printf "${YELLOW}  → Certificado TLS no encontrado — regenerando...${NC}\n"
        regenerate_tls_cert
    else
        # 3. Verificar expiración (alerta si vence en menos de 30 días)
        local expiry_date
        expiry_date=$(openssl x509 -enddate -noout -in "$cert_path" 2>/dev/null \
            | cut -d= -f2 || echo "")
        if [[ -n "$expiry_date" ]]; then
            local expiry_epoch
            expiry_epoch=$(date -d "$expiry_date" +%s 2>/dev/null || echo 0)
            local now_epoch
            now_epoch=$(date +%s)
            local days_left=$(( (expiry_epoch - now_epoch) / 86400 ))
            if [[ $days_left -lt 30 ]]; then
                printf "${YELLOW}  → Certificado TLS vence en %d días — regenerando...${NC}\n" "$days_left"
                regenerate_tls_cert
            else
                printf "  → Certificado TLS válido (%d días restantes)\n" "$days_left"
            fi
        fi
    fi
fi
```

### Función `regenerate_tls_cert()` en el script `aegis`:

```bash
regenerate_tls_cert() {
    local local_ip
    local_ip=$(hostname -I 2>/dev/null | awk '{print $1}') || local_ip="127.0.0.1"
    local cert_dir="/etc/aegis"

    printf "  → Generando certificado TLS para IP %s...\n" "$local_ip"
    openssl req -x509 -newkey rsa:4096 \
        -keyout "${cert_dir}/key.pem" \
        -out "${cert_dir}/cert.pem" \
        -days 365 -nodes \
        -subj "/CN=aegis-local" \
        -addext "subjectAltName=IP:${local_ip},IP:127.0.0.1,DNS:localhost" \
        2>/dev/null

    if id -u aegis >/dev/null 2>&1; then
        chown aegis:aegis "${cert_dir}"/*.pem 2>/dev/null || true
    fi
    chmod 640 "${cert_dir}"/*.pem

    # Asegurar que el env file tiene las vars correctas
    local env_file="/etc/aegis/aegis.env"
    if [[ -f "$env_file" ]]; then
        grep -q "AEGIS_TLS_CERT" "$env_file" \
            || echo "AEGIS_TLS_CERT=${cert_dir}/cert.pem" >> "$env_file"
        grep -q "AEGIS_TLS_KEY" "$env_file" \
            || echo "AEGIS_TLS_KEY=${cert_dir}/key.pem" >> "$env_file"
    fi

    printf "${GREEN}  → Certificado TLS regenerado para IP %s${NC}\n" "$local_ip"
    printf "${YELLOW}  → Abrí el browser en https://%s:8000 y aceptá el certificado self-signed${NC}\n" "$local_ip"
}
```

---

## Fix 2 — `ank-server/main.rs` — Log explícito del estado TLS al arrancar

El servidor debería imprimir claramente si arrancó con o sin TLS:

```rust
// En main(), donde se configura el servidor HTTP:
match (std::env::var("AEGIS_TLS_CERT"), std::env::var("AEGIS_TLS_KEY")) {
    (Ok(cert), Ok(key)) => {
        if std::path::Path::new(&cert).exists() && std::path::Path::new(&key).exists() {
            info!("🔒 TLS enabled — serving HTTPS on port 8000");
            info!("   cert: {}", cert);
        } else {
            warn!("⚠️  TLS vars set but cert/key files NOT FOUND — falling back to HTTP");
            warn!("   Expected cert: {}", cert);
            warn!("   Expected key: {}", key);
            warn!("   Run: sudo aegis update  (will regenerate TLS)");
        }
    }
    _ => {
        warn!("🔓 TLS not configured — serving HTTP on port 8000");
        warn!("   To enable HTTPS: sudo aegis update");
    }
}
```

---

## Fix 3 — `installer/aegis` — Nuevo comando `aegis tls-regen`

Para que el usuario pueda regenerar el certificado manualmente sin hacer un update completo:

```bash
cmd_tls_regen() {
    printf "${CYAN}%s${NC}\n" "--- Aegis TLS Certificate Regeneration ---"
    if [[ "$EUID" -ne 0 ]]; then
        printf "${RED}Error: requiere sudo${NC}\n"
        exit 1
    fi
    regenerate_tls_cert
    printf "  → Reiniciando servicio para aplicar nuevo certificado...\n"
    sudo systemctl restart aegis
    printf "${GREEN}Listo. Abrí: https://%s:8000${NC}\n" \
        "$(hostname -I 2>/dev/null | awk '{print $1}')"
}
```

Agregar en el case del CLI:
```bash
tls-regen) shift; cmd_tls_regen "$@" ;;
```

Y en el help:
```
tls-regen   Regenerate TLS certificate (run after IP change or expiry)
```

---

## Fix 4 — `installer/install.sh` — Verificar permisos del cert en modo update

El update descarga el binario con el usuario `root` pero el servicio corre como `aegis`.
Si el cert se regeneró pero los permisos no se actualizaron, el servidor no puede leerlo:

```bash
# En install_native(), después de setup_tls_automatic():
if [[ -f "$CONFIG_DIR/cert.pem" ]]; then
    if id -u aegis >/dev/null 2>&1; then
        chown aegis:aegis "$CONFIG_DIR"/*.pem 2>/dev/null || true
    fi
    chmod 640 "$CONFIG_DIR"/*.pem
fi
```

---

## Pasos para el operador mientras este ticket no está mergeado

Para recuperar TLS en el servidor actual:

```bash
# 1. Verificar estado
grep TLS /etc/aegis/aegis.env
ls -la /etc/aegis/*.pem 2>/dev/null || echo "No existen los certs"

# 2. Si no existen los certs, regenerarlos:
local_ip=$(hostname -I | awk '{print $1}')
sudo openssl req -x509 -newkey rsa:4096 \
    -keyout /etc/aegis/key.pem \
    -out /etc/aegis/cert.pem \
    -days 365 -nodes \
    -subj "/CN=aegis-local" \
    -addext "subjectAltName=IP:${local_ip},IP:127.0.0.1,DNS:localhost"
sudo chown aegis:aegis /etc/aegis/*.pem
sudo chmod 640 /etc/aegis/*.pem

# 3. Asegurar las vars en el env file
grep -q "AEGIS_TLS_CERT" /etc/aegis/aegis.env || \
    echo "AEGIS_TLS_CERT=/etc/aegis/cert.pem" | sudo tee -a /etc/aegis/aegis.env
grep -q "AEGIS_TLS_KEY" /etc/aegis/aegis.env || \
    echo "AEGIS_TLS_KEY=/etc/aegis/key.pem" | sudo tee -a /etc/aegis/aegis.env

# 4. Reiniciar
sudo systemctl restart aegis

# 5. Verificar
sudo journalctl -u aegis -n 20 | grep -i tls
```

---

## Criterios de aceptación

- [ ] `shellcheck installer/aegis` sin warnings
- [ ] `sudo aegis update --beta` regenera el certificado si no existe o si vence en < 30 días
- [ ] `sudo aegis tls-regen` regenera el certificado y reinicia el servicio
- [ ] El servidor imprime en los logs si arrancó con TLS o HTTP
- [ ] El servidor imprime un warning si las vars TLS apuntan a archivos que no existen
- [ ] Tras el update en producción: `journalctl -u aegis | grep TLS` muestra "TLS enabled"

---

## Dependencias

Ninguna — fix autónomo.

---

## Commit message

```
fix(installer): CORE-147 aegis update regenerates TLS cert + tls-regen command + server TLS logging
```

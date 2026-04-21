# CORE-142 — Feature: SystemConfig en MasterEnclave + TLS automático

**Epic:** 40 — Connected Accounts (OAuth)
**Repo:** Aegis-Core — `kernel/` + `shell/` + `installer/`
**Crates:** `ank-core`, `ank-http`
**Tipo:** feat
**Prioridad:** CRÍTICA — fundación de CORE-134, CORE-138 y toda la Epic 40
**Asignado a:** Kernel Engineer + Shell Engineer

---

## Problema

Hoy, configurar TLS requiere que el operador edite `/etc/aegis/aegis.env` a mano
y reinicie el servicio. Inaceptable para un producto de usuario final.

**Causa raíz:** El sistema no tiene mecanismo para persistir configuración del
sistema post-instalación. Todo está en variables de entorno inmutables en runtime.

**Nota sobre OAuth Client IDs:** Los Client IDs de Google y Spotify son
**constantes compiladas en el binario** — los registra el autor del proyecto
(Tavo) una sola vez. Los usuarios finales nunca los ven ni los tocan.
Ver ADR-042 y CORE-143.

---

## ADR-042: SystemConfig en MasterEnclave

La configuración del sistema (TLS, comportamiento del servidor) se persiste en
una tabla `system_config` del `MasterEnclave` (SQLCipher, llave = `AEGIS_ROOT_KEY`).

El env file queda solo para parámetros de bootstrap irrecuperables:
`AEGIS_ROOT_KEY`, `AEGIS_DATA_DIR`. Todo lo demás se configura desde la UI o
se compila en el binario.

**Claves de configuración en este ticket:**

| Clave | Descripción | Default |
|---|---|---|
| `tls_enabled` | `"true"` / `"false"` | `"true"` (generado por installer) |
| `tls_cert_path` | Path al cert PEM | `/etc/aegis/cert.pem` |
| `tls_key_path` | Path a la key PEM | `/etc/aegis/key.pem` |

---

## Cambios requeridos

### 1. `ank-core` — Tabla `system_config` en `MasterEnclave`

En `init_schema()` de `master.rs`, agregar:

```rust
conn.execute(
    "CREATE TABLE IF NOT EXISTS system_config (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )",
    [],
).context("Failed to init system_config table")?;
```

Agregar métodos públicos en `MasterEnclave`:

```rust
pub async fn set_config(&self, key: &str, value: &str) -> Result<()> {
    let conn = self.connection.lock().await;
    conn.execute(
        "INSERT OR REPLACE INTO system_config (key, value, updated_at)
         VALUES (?1, ?2, CURRENT_TIMESTAMP)",
        rusqlite::params![key, value],
    ).map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

pub async fn get_config(&self, key: &str) -> Result<Option<String>> {
    let conn = self.connection.lock().await;
    match conn.query_row(
        "SELECT value FROM system_config WHERE key = ?1", [key], |r| r.get(0)
    ) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow::anyhow!(e)),
    }
}
```

### 2. `ank-server/main.rs` — Leer TLS del enclave al arrancar

Después de inicializar el `MasterEnclave`, antes de `HttpConfig::from_env()`:

```rust
{
    let c = citadel.lock().await;
    if let Ok(Some(enabled)) = c.enclave.get_config("tls_enabled").await {
        if enabled == "true" {
            if let Ok(Some(cert)) = c.enclave.get_config("tls_cert_path").await {
                std::env::set_var("AEGIS_TLS_CERT", cert);
            }
            if let Ok(Some(key_path)) = c.enclave.get_config("tls_key_path").await {
                std::env::set_var("AEGIS_TLS_KEY", key_path);
            }
        }
    }
}
// HttpConfig::from_env() ya ve los valores del enclave
```

### 3. `ank-http` — Endpoint `POST /api/admin/system-config/tls/generate`

Crear `kernel/crates/ank-http/src/routes/system_config_api.rs`:

**Solo un endpoint en este ticket** — generación y activación de TLS desde la UI:

```rust
// POST /api/admin/system-config/tls/generate
// Auth: Master Admin
// Genera certificado self-signed con SAN de la IP del request,
// persiste en enclave, retorna { "success": true, "restart_required": true }
```

Implementación:
```rust
async fn generate_tls(
    State(state): State<AppState>,
    auth: CitadelMasterAuth,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<Json<Value>, AegisHttpError> {
    let cert_path = "/etc/aegis/cert.pem";
    let key_path  = "/etc/aegis/key.pem";
    let ip = addr.ip().to_string();

    // Generar certificado con SAN
    let status = std::process::Command::new("openssl")
        .args([
            "req", "-x509", "-newkey", "rsa:4096",
            "-keyout", key_path,
            "-out", cert_path,
            "-days", "365", "-nodes",
            "-subj", "/CN=aegis-local",
            "-addext", &format!(
                "subjectAltName=IP:{},IP:127.0.0.1,DNS:localhost", ip
            ),
        ])
        .status()
        .map_err(|e| AegisHttpError::Internal(anyhow::anyhow!(e)))?;

    if !status.success() {
        return Err(AegisHttpError::Internal(
            anyhow::anyhow!("openssl failed")
        ));
    }

    // Permisos
    std::process::Command::new("chmod")
        .args(["640", cert_path, key_path])
        .status().ok();

    // Persistir en enclave
    let c = state.citadel.lock().await;
    c.enclave.set_config("tls_enabled", "true").await
        .map_err(|e| AegisHttpError::Internal(e))?;
    c.enclave.set_config("tls_cert_path", cert_path).await
        .map_err(|e| AegisHttpError::Internal(e))?;
    c.enclave.set_config("tls_key_path", key_path).await
        .map_err(|e| AegisHttpError::Internal(e))?;

    Ok(Json(json!({
        "success": true,
        "restart_required": true,
        "message": format!("Certificado generado para IP {}. Reiniciá Aegis para activar HTTPS.", ip)
    })))
}
```

También agregar `GET /api/admin/system-config/tls/status`:
```json
{ "tls_enabled": true, "cert_path": "/etc/aegis/cert.pem", "cert_exists": true }
```

Y `POST /api/admin/restart`:
```rust
// Ejecuta systemctl restart aegis (nativo) o docker compose restart (Docker)
// Lee el modo de /etc/aegis/mode
// Retorna { "success": true }
```

Registrar en `routes/mod.rs`:
```rust
pub mod system_config_api;
.nest("/api/admin/system-config", system_config_api::router())
```

### 4. `installer/install.sh` — TLS automático, sin preguntar

Eliminar `show_tls_menu()`. Reemplazar `setup_tls()` por `setup_tls_automatic()`:

```bash
setup_tls_automatic() {
    log "Generando certificado TLS self-signed (HTTPS activado por defecto)..."
    mkdir -p "$CONFIG_DIR"
    local local_ip
    local_ip=$(hostname -I 2>/dev/null | awk '{print $1}') || local_ip="127.0.0.1"

    openssl req -x509 -newkey rsa:4096 \
        -keyout "$CONFIG_DIR/key.pem" \
        -out "$CONFIG_DIR/cert.pem" \
        -days 365 -nodes \
        -subj "/CN=aegis-local" \
        -addext "subjectAltName=IP:${local_ip},IP:127.0.0.1,DNS:localhost" \
        >> "$LOG_FILE" 2>&1 || warn "TLS generation failed — continuing with HTTP"

    if [[ -f "$CONFIG_DIR/cert.pem" ]]; then
        if id -u aegis >/dev/null 2>&1; then
            chown aegis:aegis "$CONFIG_DIR"/*.pem 2>/dev/null || true
        fi
        chmod 640 "$CONFIG_DIR"/*.pem
        ENABLE_TLS="true"
        success "Certificado TLS generado para IP ${local_ip}"
    fi
}
```

En `install_native()`, siempre escribir las vars de TLS:
```bash
if [[ "$ENABLE_TLS" == "true" ]]; then
    echo "AEGIS_TLS_CERT=${CONFIG_DIR}/cert.pem" >> "$ENV_FILE"
    echo "AEGIS_TLS_KEY=${CONFIG_DIR}/key.pem" >> "$ENV_FILE"
fi
```

En `wait_and_show()`, después de que el servidor esté up, persistir TLS en el enclave
usando la API (con el setup_token):
```bash
# Persiste la config de TLS en el enclave via API (best-effort)
if [[ "$ENABLE_TLS" == "true" ]] && [[ -n "$token" ]]; then
    curl -sk -X POST "${PROTOCOL}://localhost:8000/api/admin/system-config" \
        -H "Content-Type: application/json" \
        -H "x-citadel-tenant: root" \
        -d "{\"key\":\"tls_enabled\",\"value\":\"true\",\"setup_token\":\"${token}\"}" \
        >> "$LOG_FILE" 2>&1 || true
fi
```

### 5. Shell — Sección TLS en SystemTab del Admin Dashboard

En el `SystemTab`, agregar sección "Seguridad del Servidor" debajo de las métricas:

```tsx
// GET /api/admin/system-config/tls/status → mostrar estado
// Si tls_enabled: badge verde "HTTPS Activo"
// Si no: badge rojo "HTTP (inseguro)" + botón "Activar HTTPS"
//   → llama POST /api/admin/system-config/tls/generate
//   → muestra mensaje "Reiniciá Aegis para aplicar"
//   → botón "Reiniciar ahora" → POST /api/admin/restart
```

---

## Criterios de aceptación

- [ ] `cargo build --workspace` sin errores
- [ ] `set_config / get_config` en MasterEnclave persisten entre reinicios
- [ ] Installer genera TLS automáticamente sin preguntar, con SAN de IP local
- [ ] Al arrancar: si `tls_enabled=true` en enclave → Axum sirve HTTPS
- [ ] `POST /api/admin/system-config/tls/generate` genera cert, persiste en enclave
- [ ] `POST /api/admin/restart` reinicia el servicio correctamente
- [ ] SystemTab muestra estado TLS y permite activarlo desde la UI

---

## Dependencias

Ninguna — es fundación pura.

## Tickets que desbloquea

CORE-134, CORE-138, CORE-139, CORE-140, CORE-141.

---

## Commit message

```
feat(ank-core,ank-http,installer): CORE-142 SystemConfig in MasterEnclave + automatic TLS
```

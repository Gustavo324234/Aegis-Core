# CORE-146 — Feature: Conexión app por QR + acceso remoto via tunnel

**Epic:** 41 — UX & Onboarding
**Repo:** Aegis-Core — `shell/` + `kernel/` + `app/`
**Tipo:** feat
**Prioridad:** Alta
**Asignado a:** Shell Engineer + Kernel Engineer

---

## Problema

La app mobile hoy pide que el usuario escriba manualmente la IP del servidor.
Esto es inutilizable porque:
1. Los usuarios no saben su IP local
2. Fuera de la red local (otra red WiFi, datos móviles) la IP interna no es accesible

---

## Solución en dos partes

### Parte 1 — Conexión por QR (red local)

La Shell web muestra un QR con la URL de conexión. El usuario abre la app,
escanea el QR, y queda conectado. Sin tipear nada.

### Parte 2 — Acceso remoto via tunnel (fuera de casa)

Aegis levanta un tunnel seguro usando **Cloudflare Tunnel** (gratuito, sin puertos abiertos,
sin IP pública necesaria). El tunnel genera una URL pública `https://xxx.trycloudflare.com`
que se incluye en el QR. La app se conecta a esa URL tanto en LAN como en internet.

**Por qué Cloudflare Tunnel y no otros:**
- Gratuito y sin registro para tunnels temporales (`trycloudflare.com`)
- Sin configuración de router ni ports forwarding
- TLS incluido — el tunnel ya tiene HTTPS
- Un binario (`cloudflared`) que el installer puede descargar automáticamente
- Funciona con cualquier servidor Linux

---

## Arquitectura

```
App mobile
    │  escanea QR desde la Shell web
    ▼
URL del tunnel: https://abc123.trycloudflare.com
    │
    ▼
Cloudflare Edge → tunnel → ank-server :8000 (LAN)
```

El tunnel corre como proceso hijo del servidor, manejado por el kernel.
La URL se actualiza en el QR cada vez que el tunnel se reconecta.

---

## Cambios requeridos

### 1. `ank-http` — Endpoint `GET /api/system/connection-info`

Nuevo endpoint sin auth (o con auth básica) que retorna la info de conexión:

```rust
// GET /api/system/connection-info
// Retorna:
{
    "local_url": "https://192.168.1.6:8000",
    "tunnel_url": "https://abc123.trycloudflare.com",  // null si tunnel no activo
    "tunnel_status": "active" | "connecting" | "disabled",
    "qr_url": "https://abc123.trycloudflare.com"  // preferir tunnel > local
}
```

El estado del tunnel se lee de una variable en el `AppState`:
```rust
pub struct AppState {
    // ... existing fields ...
    pub tunnel_url: Arc<RwLock<Option<String>>>,
}
```

### 2. `ank-server/main.rs` — Tunnel manager

Al arrancar, si `cloudflared` está disponible, lanzar el tunnel:

```rust
// En main(), después de arrancar el servidor HTTP:
{
    let tunnel_url_state = Arc::clone(&state.tunnel_url);
    tokio::spawn(async move {
        loop {
            match start_cloudflare_tunnel(8000).await {
                Ok(url) => {
                    tracing::info!("Cloudflare tunnel active: {}", url);
                    *tunnel_url_state.write().await = Some(url);
                }
                Err(e) => {
                    tracing::warn!("Tunnel unavailable: {}", e);
                    *tunnel_url_state.write().await = None;
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
            }
        }
    });
}

async fn start_cloudflare_tunnel(port: u16) -> anyhow::Result<String> {
    use tokio::process::Command;
    use tokio::io::{AsyncBufReadExt, BufReader};

    // cloudflared tunnel --url http://localhost:PORT
    // Lee stdout hasta encontrar "trycloudflare.com" URL
    let mut child = Command::new("cloudflared")
        .args(["tunnel", "--url", &format!("http://localhost:{}", port)])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let stderr = child.stderr.take()
        .ok_or_else(|| anyhow::anyhow!("No stderr"))?;
    let mut lines = BufReader::new(stderr).lines();

    // cloudflared imprime la URL en stderr
    while let Some(line) = lines.next_line().await? {
        if let Some(url) = extract_tunnel_url(&line) {
            return Ok(url);
        }
    }
    anyhow::bail!("cloudflared exited without printing URL")
}

fn extract_tunnel_url(line: &str) -> Option<String> {
    // cloudflared imprime algo como:
    // "https://abc123.trycloudflare.com"
    if line.contains("trycloudflare.com") {
        line.split_whitespace()
            .find(|s| s.starts_with("https://") && s.contains("trycloudflare.com"))
            .map(|s| s.to_string())
    } else {
        None
    }
}
```

### 3. `installer/install.sh` — Instalar cloudflared

Agregar en `install_dependencies()`:

```bash
install_cloudflared() {
    if command -v cloudflared &>/dev/null; then
        log "cloudflared ya instalado — omitiendo"
        return
    fi
    log "Instalando cloudflared (tunnel remoto)..."
    local arch_str="amd64"
    [[ "$(uname -m)" == "aarch64" ]] && arch_str="arm64"
    local url="https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-${arch_str}"
    curl -L --fail --silent "$url" -o /usr/local/bin/cloudflared \
        && chmod +x /usr/local/bin/cloudflared \
        && success "cloudflared instalado" \
        || warn "No se pudo instalar cloudflared — acceso remoto deshabilitado"
}
```

Llamar a `install_cloudflared` dentro de `install_native()`.

### 4. Shell — Pantalla QR en la UI

Crear `shell/ui/src/components/ConnectionQR.tsx`:

```tsx
// Componente que muestra el QR de conexión
// Usa la librería 'qrcode.react' o genera el QR via API

// Al cargar: GET /api/system/connection-info
// Muestra:
//   - Si tunnel activo: QR con tunnel_url + badge "Acceso remoto ✓"
//   - Si solo LAN: QR con local_url + badge "Solo red local"
//   - Si tunnel conectando: spinner + "Activando acceso remoto..."

// El QR se refresca automáticamente cada 30 segundos
// (el tunnel_url puede cambiar si cloudflared se reconecta)
```

Agregar el botón QR en el header del ChatTerminal:
```tsx
// En el header, junto al botón de Settings:
<button onClick={() => setShowQR(true)} title="Conectar app móvil">
    <QrCode className="w-5 h-5" />
</button>

// Modal con el QR:
{showQR && <ConnectionQR onClose={() => setShowQR(false)} />}
```

Instalar dependencia:
```bash
cd shell/ui && npm install qrcode.react
```

### 5. App mobile — Pantalla de escaneo QR en el login

En `app/app/(auth)/login.tsx`, agregar botón "Escanear QR":

```tsx
import { CameraView, useCameraPermissions } from 'expo-camera';

// Botón alternativo al input manual de IP:
<TouchableOpacity onPress={() => setShowScanner(true)}>
  <Text>Escanear QR de la Shell</Text>
</TouchableOpacity>

// Scanner:
{showScanner && (
  <CameraView
    onBarcodeScanned={({ data }) => {
      // data = URL de Aegis (tunnel o local)
      // Extraer el host y guardarlo como serverUrl
      const url = new URL(data);
      setServerUrl(url.origin);
      setShowScanner(false);
    }}
    barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
  />
)}
```

Instalar dependencia:
```bash
cd app && npx expo install expo-camera
```

Actualizar `app.json` con permisos de cámara:
```json
{
  "expo": {
    "plugins": [
      ["expo-camera", { "cameraPermission": "Aegis necesita la cámara para escanear el QR de conexión." }]
    ]
  }
}
```

### 6. Persistencia de la URL en la app

Una vez conectada, la app guarda la URL (tunnel o local) en `expo-secure-store`.
Al reconectar, si la URL guardada falla, mostrar el scanner de QR nuevamente.

En `authStore.ts`:
```typescript
// Al hacer login exitoso, guardar la serverUrl
// Si la conexión falla al reintentar, limpiar serverUrl y mostrar pantalla de conexión
```

---

## Criterios de aceptación

- [ ] `cargo build --workspace` sin errores
- [ ] `GET /api/system/connection-info` retorna local_url y tunnel_url (si disponible)
- [ ] Si cloudflared está instalado: tunnel levanta en arranque del servidor y URL aparece en el endpoint
- [ ] Shell muestra botón QR en el header
- [ ] El QR contiene la tunnel_url si está activa, local_url si no
- [ ] App mobile muestra botón "Escanear QR" en la pantalla de conexión
- [ ] Al escanear el QR: la app se conecta automáticamente sin tipear nada
- [ ] La conexión via tunnel funciona desde otra red WiFi o datos móviles
- [ ] Si el tunnel no está disponible (cloudflared no instalado): el QR usa la IP local y funciona en LAN
- [ ] `npx expo export` sin errores TypeScript

---

## Dependencias

- CORE-142 (TLS) — DONE ✅

---

## Commit message

```
feat(ank-server,shell,app): CORE-146 QR connection + Cloudflare tunnel for remote access
```

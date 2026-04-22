# CORE-146 — Feature: Conexión app por QR + Cloudflare Tunnel automático

**Epic:** 41 — UX & Onboarding
**Repo:** Aegis-Core — `shell/` + `kernel/` + `installer/` + `app/`
**Tipo:** feat
**Prioridad:** Alta
**Asignado a:** Kernel Engineer + Shell Engineer
**Depende de:** CORE-147 (servidor corriendo en HTTP)

---

## Arquitectura final

```
Browser / App mobile
    ↓
https://abc123.trycloudflare.com  ← HTTPS válido, candado verde, micrófono OK
    ↓
Cloudflare Edge
    ↓  (tunnel cifrado)
ank-server :8000 HTTP  ← servidor interno, sin certificados
```

El tunnel:
- Lo levanta Aegis automáticamente al arrancar si `cloudflared` está instalado
- La URL se expone via `GET /api/system/connection-info`
- La Shell la muestra como QR — el usuario escanea y queda conectado
- Funciona desde cualquier red (LAN, WiFi externa, datos móviles)
- Si cloudflared no está disponible: la app puede conectarse por IP local (fallback)

---

## Cambios requeridos

### 1. `ank-server/main.rs` — Tunnel manager automático

```rust
// En AppState, agregar:
pub tunnel_url: Arc<RwLock<Option<String>>>,

// En main(), después de arrancar el servidor HTTP:
{
    let tunnel_url_arc = Arc::clone(&state.tunnel_url);
    tokio::spawn(async move {
        loop {
            match start_cloudflare_tunnel(8000).await {
                Ok(url) => {
                    tracing::info!("🌍 Cloudflare tunnel active: {}", url);
                    *tunnel_url_arc.write().await = Some(url);
                    // Mantener el proceso vivo — si muere, reintentar
                    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                }
                Err(e) => {
                    tracing::warn!("Tunnel unavailable ({}), retrying in 60s", e);
                    *tunnel_url_arc.write().await = None;
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
            }
        }
    });
}
```

```rust
async fn start_cloudflare_tunnel(port: u16) -> anyhow::Result<String> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    let mut child = Command::new("cloudflared")
        .args(["tunnel", "--url", &format!("http://localhost:{}", port), "--no-autoupdate"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| anyhow::anyhow!("cloudflared not installed"))?;

    let stderr = child.stderr.take()
        .ok_or_else(|| anyhow::anyhow!("no stderr"))?;
    let mut lines = BufReader::new(stderr).lines();

    // cloudflared imprime la URL en stderr, generalmente en <5 segundos
    let timeout = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        async {
            while let Ok(Some(line)) = lines.next_line().await {
                if line.contains("trycloudflare.com") {
                    if let Some(url) = line
                        .split_whitespace()
                        .find(|s| s.starts_with("https://") && s.contains("trycloudflare.com"))
                    {
                        return Ok(url.to_string());
                    }
                }
            }
            Err(anyhow::anyhow!("cloudflared exited without URL"))
        }
    ).await;

    match timeout {
        Ok(result) => result,
        Err(_) => Err(anyhow::anyhow!("cloudflared timeout")),
    }
}
```

### 2. `ank-http/src/routes/status.rs` — Endpoint connection-info

Agregar a `status.rs` (o crear `connection_info.rs`):

```rust
// GET /api/system/connection-info — sin auth requerida
// Retorna info para que la app mobile pueda conectarse

#[derive(Serialize)]
struct ConnectionInfo {
    local_url: String,
    tunnel_url: Option<String>,
    tunnel_status: &'static str,
    qr_url: String,
}

pub async fn connection_info(State(state): State<AppState>) -> Json<ConnectionInfo> {
    let local_ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    let local_url = format!("http://{}:8000", local_ip);

    let tunnel_url = state.tunnel_url.read().await.clone();
    let tunnel_status = if tunnel_url.is_some() { "active" } else { "connecting" };
    let qr_url = tunnel_url.clone().unwrap_or_else(|| local_url.clone());

    Json(ConnectionInfo { local_url, tunnel_url, tunnel_status, qr_url })
}
```

Agregar en `routes/mod.rs`:
```rust
.route("/api/system/connection-info", get(status::connection_info))
```

Agregar dependencia en `ank-http/Cargo.toml`:
```toml
local-ip-address = "0.14"
```

### 3. `installer/aegis` — Nuevo comando `aegis tunnel`

```bash
cmd_tunnel() {
    if ! command -v cloudflared &>/dev/null; then
        printf '%bcloudflared no está instalado. Ejecutá: sudo aegis update%b\n' "$RED" "$NC"
        exit 1
    fi
    printf '%b--- Aegis Tunnel ---%b\n' "$CYAN" "$NC"
    printf 'Levantando tunnel HTTPS hacia el servidor Aegis...\n\n'
    cloudflared tunnel --url http://localhost:8000 --no-autoupdate
}
```

Agregar en el case:
```bash
tunnel)    shift; cmd_tunnel "$@" ;;
```

Agregar en el help:
```
tunnel        Start HTTPS tunnel (public URL via Cloudflare)
```

### 4. Shell — Componente `ConnectionQR.tsx`

```tsx
import React, { useState, useEffect } from 'react';
import { QRCodeSVG } from 'qrcode.react';
import { X, Wifi, Globe, Loader2, RefreshCw } from 'lucide-react';

interface ConnectionInfo {
  local_url: string;
  tunnel_url: string | null;
  tunnel_status: 'active' | 'connecting' | 'disabled';
  qr_url: string;
}

const ConnectionQR: React.FC<{ onClose: () => void }> = ({ onClose }) => {
  const [info, setInfo] = useState<ConnectionInfo | null>(null);
  const [loading, setLoading] = useState(true);

  const fetchInfo = async () => {
    setLoading(true);
    try {
      const res = await fetch('/api/system/connection-info');
      if (res.ok) setInfo(await res.json());
    } catch { /* ignore */ }
    finally { setLoading(false); }
  };

  useEffect(() => {
    fetchInfo();
    // Refrescar cada 30s — la URL del tunnel puede cambiar
    const interval = setInterval(fetchInfo, 30_000);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="fixed inset-0 z-[200] flex items-center justify-center bg-black/80 backdrop-blur-md p-4">
      <div className="bg-aegis-steel border border-white/10 rounded-2xl p-8 max-w-sm w-full">

        {/* Header */}
        <div className="flex justify-between items-center mb-6">
          <h3 className="text-sm font-mono font-bold text-aegis-cyan uppercase tracking-widest">
            Conectar App
          </h3>
          <button onClick={onClose} className="text-white/20 hover:text-white">
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* QR */}
        <div className="flex justify-center mb-6">
          {loading ? (
            <div className="w-48 h-48 flex items-center justify-center">
              <Loader2 className="w-8 h-8 text-aegis-cyan animate-spin" />
            </div>
          ) : info ? (
            <div className="p-3 bg-white rounded-xl">
              <QRCodeSVG value={info.qr_url} size={180} />
            </div>
          ) : (
            <div className="w-48 h-48 flex items-center justify-center text-white/30 text-xs font-mono">
              Error al cargar
            </div>
          )}
        </div>

        {/* Status badge */}
        {info && (
          <div className="space-y-3 mb-6">
            {info.tunnel_url ? (
              <div className="flex items-center gap-2 p-3 bg-green-500/10 border border-green-500/20 rounded-xl">
                <Globe className="w-4 h-4 text-green-400 shrink-0" />
                <div>
                  <p className="text-[10px] font-mono text-green-400 uppercase tracking-widest">
                    Acceso remoto activo ✓
                  </p>
                  <p className="text-[9px] font-mono text-white/30 mt-0.5 break-all">
                    {info.tunnel_url}
                  </p>
                </div>
              </div>
            ) : (
              <div className="flex items-center gap-2 p-3 bg-yellow-500/10 border border-yellow-500/20 rounded-xl">
                <Wifi className="w-4 h-4 text-yellow-400 shrink-0" />
                <div>
                  <p className="text-[10px] font-mono text-yellow-400 uppercase tracking-widest">
                    Solo red local
                  </p>
                  <p className="text-[9px] font-mono text-white/30 mt-0.5">
                    Activando tunnel remoto...
                  </p>
                </div>
              </div>
            )}
          </div>
        )}

        {/* Instrucción */}
        <p className="text-[10px] font-mono text-white/30 text-center mb-4">
          Abrí la app Aegis en tu teléfono y escaneá el QR
        </p>

        {/* Refresh */}
        <button
          onClick={fetchInfo}
          className="w-full flex items-center justify-center gap-2 py-2 border border-white/10 rounded-lg text-[10px] font-mono text-white/30 hover:text-white hover:border-white/30 transition-colors"
        >
          <RefreshCw className="w-3 h-3" />
          Actualizar
        </button>

      </div>
    </div>
  );
};

export default ConnectionQR;
```

**En `ChatTerminal.tsx`**, agregar el botón QR junto al botón de Settings:

```tsx
import { QrCode } from 'lucide-react';
import ConnectionQR from './ConnectionQR';

// Estado:
const [showQR, setShowQR] = useState(false);

// En el header, junto al botón Settings:
<button
  onClick={() => setShowQR(true)}
  className="p-2 rounded-lg bg-white/5 text-white/40 hover:text-aegis-cyan hover:bg-aegis-cyan/10 transition-all"
  title="Conectar app móvil"
>
  <QrCode className="w-5 h-5" />
</button>

// Al final del componente:
{showQR && <ConnectionQR onClose={() => setShowQR(false)} />}
```

Instalar dependencia:
```bash
cd shell/ui && npm install qrcode.react
```

### 5. App mobile — Scanner QR en login

En `app/app/(auth)/login.tsx`, agregar botón y scanner:

```tsx
import { CameraView, useCameraPermissions } from 'expo-camera';

// Botón debajo del input de IP:
<TouchableOpacity onPress={() => setShowScanner(true)}
  style={styles.qrButton}>
  <Text style={styles.qrButtonText}>📷  Escanear QR desde la Shell</Text>
</TouchableOpacity>

// Scanner modal:
{showScanner && (
  <View style={StyleSheet.absoluteFill}>
    <CameraView
      style={StyleSheet.absoluteFill}
      onBarcodeScanned={({ data }) => {
        try {
          const url = new URL(data);
          setServerUrl(url.origin);  // Guarda https://xxx.trycloudflare.com
          setShowScanner(false);
        } catch { /* URL inválida */ }
      }}
      barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
    />
    <TouchableOpacity
      onPress={() => setShowScanner(false)}
      style={styles.cancelScan}>
      <Text style={styles.cancelScanText}>Cancelar</Text>
    </TouchableOpacity>
  </View>
)}
```

Instalar dependencia:
```bash
cd app && npx expo install expo-camera
```

En `app.json`:
```json
["expo-camera", {
  "cameraPermission": "Aegis necesita la cámara para escanear el código QR de conexión."
}]
```

---

## Criterios de aceptación

- [ ] `cargo build --workspace` sin errores
- [ ] Al arrancar con cloudflared instalado: tunnel activo en < 30 segundos
- [ ] `GET /api/system/connection-info` retorna tunnel_url cuando está activo
- [ ] Shell muestra botón QR en el header del chat
- [ ] El QR muestra badge verde "Acceso remoto activo" cuando el tunnel está activo
- [ ] Al escanear el QR desde la app: conexión automática sin tipear nada
- [ ] La conexión via tunnel_url funciona desde otra red
- [ ] Sin cloudflared: QR usa IP local, badge amarillo "Solo red local"
- [ ] `npm run build && npm run lint` sin errores (Shell)
- [ ] `npx expo export` sin errores (App)

---

## Dependencias

- CORE-147 (servidor en HTTP, cloudflared instalado por el update)

---

## Commit message

```
feat(ank-server,shell,app,installer): CORE-146 Cloudflare tunnel auto-start + QR connection
```

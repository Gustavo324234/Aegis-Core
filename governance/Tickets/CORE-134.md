# CORE-134 — Fix: TLS en Axum (puerto 8000) — Siren funciona desde otros dispositivos

**Epic:** 38 — Agent Persona System (blocker Siren)
**Repo:** Aegis-Core — `kernel/`
**Crates:** `ank-http`, `ank-server`
**Tipo:** fix
**Prioridad:** CRÍTICA — Siren bloqueado en todos los dispositivos no-localhost
**Asignado a:** Kernel Engineer

---

## Contexto y diagnóstico

`getUserMedia()` (micrófono) solo funciona en contextos seguros:
`https://` o `localhost`/`127.0.0.1`. Desde otra PC via `http://192.168.x.x:8000`
el browser bloquea el acceso al micrófono por diseño del estándar Web.

El instalador (`install.sh`) ya tiene `show_tls_menu()` y `setup_tls()` que generan
un certificado self-signed en `/etc/aegis/cert.pem` y `/etc/aegis/key.pem` y
escriben en el env file:

```
AEGIS_TLS_CERT=/etc/aegis/cert.pem
AEGIS_TLS_KEY=/etc/aegis/key.pem
```

**El bug:** `main.rs` aplica TLS solo al servidor **gRPC (Tonic, puerto 50051)**.
El servidor **HTTP/WS (Axum, puerto 8000)** — el que usa el browser — nunca lee
esas variables y siempre sirve HTTP plano.

**El fix:** Leer `AEGIS_TLS_CERT`/`AEGIS_TLS_KEY` en `AegisHttpServer::serve()` y
si están presentes, servir con TLS usando `axum-server` + `rustls`.

---

## Cambios requeridos

### 1. Agregar dependencias en `kernel/crates/ank-http/Cargo.toml`

```toml
axum-server = { version = "0.7", features = ["tls-rustls"] }
```

> `axum-server` es el wrapper estándar de axum para TLS con rustls.
> Ya existe `rustls` en el workspace — verificar que no haya conflicto de versiones.

### 2. `HttpConfig` — campos TLS opcionales

En `kernel/crates/ank-http/src/config.rs`:

```rust
#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub port: u16,
    pub static_dir: String,
    pub dev_mode: bool,
    pub ui_dist_path: Option<PathBuf>,
    pub data_dir: PathBuf,
    /// Path al certificado TLS (PEM). Si está presente junto con tls_key,
    /// Axum servirá HTTPS en lugar de HTTP.
    pub tls_cert: Option<PathBuf>,
    /// Path a la clave privada TLS (PEM).
    pub tls_key: Option<PathBuf>,
}

impl HttpConfig {
    pub fn from_env() -> Self {
        // ... campos existentes ...
        let tls_cert = std::env::var("AEGIS_TLS_CERT").ok().map(PathBuf::from);
        let tls_key  = std::env::var("AEGIS_TLS_KEY").ok().map(PathBuf::from);

        Self {
            // ... campos existentes ...
            tls_cert,
            tls_key,
        }
    }

    /// Retorna true si ambos archivos TLS están configurados y existen en disco.
    pub fn tls_enabled(&self) -> bool {
        match (&self.tls_cert, &self.tls_key) {
            (Some(c), Some(k)) => c.exists() && k.exists(),
            _ => false,
        }
    }
}
```

### 3. `AegisHttpServer::serve()` — branching HTTP vs HTTPS

En `kernel/crates/ank-http/src/lib.rs`:

```rust
use axum_server::tls_rustls::RustlsConfig;

impl AegisHttpServer {
    pub async fn serve(self) -> Result<()> {
        let port = self.state.config.port;
        let app = routes::build_router(self.state.clone());
        let addr: SocketAddr = format!("0.0.0.0:{port}").parse()?;

        if self.state.config.tls_enabled() {
            let cert_path = self.state.config.tls_cert.as_ref().unwrap();
            let key_path  = self.state.config.tls_key.as_ref().unwrap();

            let tls_config = RustlsConfig::from_pem_file(cert_path, key_path)
                .await
                .map_err(|e| anyhow::anyhow!("TLS config failed: {}", e))?;

            tracing::info!("Aegis HTTP/TLS server listening on https://{}", addr);
            axum_server::bind_rustls(addr, tls_config)
                .serve(app.into_make_service_with_connect_info::<SocketAddr>())
                .await?;
        } else {
            tracing::warn!("Aegis HTTP server (insecure) listening on http://{}", addr);
            tracing::warn!("Siren (microphone) will not work from other devices over HTTP.");
            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await?;
        }

        Ok(())
    }
}
```

### 4. Verificar que WebSockets funcionan sobre TLS

`axum-server` con `tls-rustls` soporta WebSocket upgrade sobre TLS nativamente.
El cliente debe conectar a `wss://` en lugar de `ws://`. El store de Zustand
ya construye la URL del WebSocket desde `window.location` — si la página es
`https://`, `window.location.protocol` es `"https:"` y la URL del WS debe
ser `wss://`. Verificar en `useAegisStore.ts` que la construcción sea:

```ts
const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
const wsUrl = `${wsProtocol}//${window.location.host}/ws/chat/${tenantId}`;
```

Si no está así, corregirlo en este mismo ticket (es un cambio de 1 línea).

### 5. Nota sobre certificado self-signed

El browser mostrará una advertencia "No seguro" la primera vez con certificado
self-signed. El usuario debe aceptar la excepción de seguridad. Esto es
comportamiento esperado y documentado en el instalador.

Para producción real, el operador puede reemplazar el certificado en
`/etc/aegis/cert.pem` y `/etc/aegis/key.pem` con uno de Let's Encrypt.

---

## Criterios de aceptación

- [ ] `cargo build --workspace` sin errores ni warnings Clippy
- [ ] Sin `AEGIS_TLS_CERT`/`AEGIS_TLS_KEY`: servidor arranca en HTTP (comportamiento actual)
- [ ] Con las variables seteadas y archivos existentes: servidor arranca en HTTPS
- [ ] Con HTTPS activo: WebSocket de chat conecta via `wss://`
- [ ] Con HTTPS activo: el micrófono es accesible desde otro dispositivo en la LAN
- [ ] Log al arrancar indica claramente si está en modo seguro o inseguro
- [ ] Si los archivos TLS no existen (variables seteadas pero archivos borrados): el servidor
     loga un warning y arranca en HTTP (no crashea)

---

## Dependencias

Ninguna — ticket autónomo desde el punto de vista de otros tickets de la Epic 38.

---

## Nota para el Kernel Engineer

El instalador ya genera el certificado self-signed y escribe las variables en el
env file cuando el usuario elige TLS en el menú. Este ticket solo conecta el lado
del servidor que faltaba. No hace falta tocar el instalador.

---

## Commit message

```
fix(ank-http): CORE-134 TLS for Axum — HTTPS on port 8000 enables Siren from LAN devices
```

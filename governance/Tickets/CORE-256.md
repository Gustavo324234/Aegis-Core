# CORE-256 — Admin: panel de gestión del servicio del sistema (start/stop/restart/status)

**Tipo:** feat  
**Prioridad:** Alta  
**Estado:✅ Done
**Épica:** EPIC 48 — Shell Observability  
**Asignado a:Shell + Kernel
**Componentes:**
- `kernel/crates/ank-http/src/routes/` — nuevos endpoints de gestión del sistema
- `shell/ui/src/components/AdminDashboard.tsx` — nueva tab "Sistema"

---

## Contexto

Actualmente no hay forma de gestionar el servicio de Aegis desde la UI. Si el sistema se reinicia y el servicio no arrancó automáticamente, el usuario tiene que usar comandos PowerShell o ir al Administrador de Servicios de Windows.

El panel de admin debe tener una sección donde pueda ver el estado del servidor y gestionarlo sin salir del browser.

---

## Cambios requeridos

### Parte 1 — Kernel: endpoints de gestión del sistema

**`GET /api/system/status`** — ya existe parcialmente, extender para incluir:

```json
{
  "status": "online",
  "version": "0.8.1",
  "uptime_seconds": 3642,
  "platform": "windows",
  "pid": 12345,
  "data_dir": "C:\\ProgramData\\Aegis",
  "service_mode": true,
  "tunnel_active": false,
  "tunnel_url": null
}
```

Campos nuevos: `uptime_seconds`, `platform`, `pid`, `data_dir`, `service_mode`, `tunnel_active`, `tunnel_url`.

**`POST /api/system/restart`** — solo admin — envía señal de restart al proceso:

```rust
// En Linux: signal(SIGTERM) + systemd lo reinicia
// En Windows: sc.exe restart AegisOS via Command::new("sc")
// Fallback: proceso se termina y el servicio lo reinicia automáticamente (si está configurado)
async fn restart_system() -> impl IntoResponse {
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(1)).await;
        std::process::exit(0); // El service manager lo reinicia
    });
    Json(json!({ "status": "restarting" }))
}
```

Ambos endpoints requieren header de master admin (`x-citadel-master-key` o equivalente actual).

### Parte 2 — Shell: tab "Sistema" en AdminDashboard

Agregar una tab "Sistema" (o "System") en el `AdminDashboard`, entre las tabs existentes.

**Layout de la tab:**

```
┌─────────────────────────────────────────────────────────┐
│  ESTADO DEL SERVIDOR                                    │
│                                                         │
│  ● Online          Uptime: 1h 2m 22s                   │
│  Version: 0.8.1    Platform: Windows x86_64             │
│  PID: 12345        Data: C:\ProgramData\Aegis           │
│                                                         │
│  [Reiniciar servidor]                                   │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│  TUNNEL CLOUDFLARE                                      │
│                                                         │
│  ● Inactivo — cloudflared no encontrado                 │
│                                                         │
│  Para acceso remoto HTTPS instalá cloudflared:          │
│  https://developers.cloudflare.com/cloudflare-one/...   │
└─────────────────────────────────────────────────────────┘
```

**Comportamiento del botón "Reiniciar servidor":**
- Muestra confirmación: "¿Reiniciar el servidor? La sesión actual se cerrará."
- Al confirmar, llama `POST /api/system/restart`
- Muestra spinner "Reiniciando..." y después de 3s intenta hacer polling a `GET /api/system/status` hasta que responda
- Cuando responde, recarga la página

**Polling de estado:**
La tab hace `GET /api/system/status` cada 10 segundos y actualiza el uptime y estado en tiempo real.

### Estado del tunnel

Si `tunnel_active: false`, mostrar instrucciones para instalar cloudflared con link directo.
Si `tunnel_active: true`, mostrar la URL pública del tunnel con botón "Copiar".

---

## Criterios de aceptación

- [x] La tab "Sistema" muestra estado real del servidor (uptime, versión, plataforma, PID).
- [x] El botón "Reiniciar servidor" funciona y el servidor vuelve a estar online después del restart.
- [x] El estado se actualiza automáticamente cada 10 segundos sin recargar la página.
- [x] Si el tunnel de Cloudflare está activo, muestra la URL pública.
- [x] Si cloudflared no está instalado, muestra el link de descarga.
- [x] Solo el master admin puede ver y usar esta tab.
- [x] TypeScript y Rust sin errores de compilación.

## Dependencias

- `GET /api/system/status` (extender el existente)
- `POST /api/system/restart` (nuevo)

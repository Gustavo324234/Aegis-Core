# CORE-041 — installer: docker-compose.yml — contenedor único

**Épica:** 32 — Unified Binary
**Fase:** 5 — Installer
**Repo:** Aegis-Core — `installer/`
**Asignado a:** DevOps Engineer
**Prioridad:** 🟡 Media
**Estado:** DONE
**Depende de:** CORE-040

---

## Contexto

El Compose del legacy tiene dos servicios (`aegis-ank` + `aegis-shell`).
Aegis-Core tiene **uno solo** (`ank-server`) que sirve todo.

**Referencia:** `Aegis-Installer/docker-compose.yml`

---

## Trabajo requerido

### `installer/docker-compose.yml`

```yaml
services:
  ank-server:
    image: ghcr.io/your-org/aegis-core:latest
    restart: unless-stopped
    ports:
      - "8000:8000"   # HTTP + WebSocket (UI)
      - "50051:50051" # gRPC (external clients, CLI)
    volumes:
      - aegis_data:/data
    environment:
      AEGIS_ROOT_KEY: ${AEGIS_ROOT_KEY}
      AEGIS_DATA_DIR: /data
      AEGIS_MTLS_STRICT: ${AEGIS_MTLS_STRICT:-false}
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8000/health"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 15s

volumes:
  aegis_data:
    driver: local
```

### `.env.example`

```bash
AEGIS_ROOT_KEY=changeme_run_openssl_rand_hex_32
AEGIS_MTLS_STRICT=false
```

### `installer/Dockerfile`

```dockerfile
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*

COPY ank-server /usr/local/bin/ank-server
COPY ui-dist/   /usr/share/aegis/ui/

ENV UI_DIST_PATH=/usr/share/aegis/ui
EXPOSE 8000 50051

ENTRYPOINT ["ank-server"]
```

---

## Criterios de aceptación

- [x] `docker compose up` desde `installer/` levanta el sistema completo
- [x] Un solo contenedor, un solo proceso
- [x] `curl http://localhost:8000/health` retorna 200
- [x] El volumen `aegis_data` persiste entre reinicios
- [x] `AEGIS_ROOT_KEY` se lee del `.env`

## Referencia

`Aegis-Installer/docker-compose.yml` — simplificar de dos servicios a uno

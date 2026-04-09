# CORE-061 — CI: Docker publish — imagen única

**Épica:** 32 — Unified Binary
**Fase:** 7 — CI/CD y Governance
**Repo:** Aegis-Core — `.github/workflows/`
**Asignado a:** DevOps Engineer
**Prioridad:** 🟡 Media
**Estado:** DONE
**Depende de:** CORE-041, CORE-060

---

## Contexto

Publicar la imagen Docker de Aegis-Core en GHCR al hacer merge a `main`.
Una sola imagen en lugar de dos (antes: `aegis-ank` + `aegis-shell`).

**Referencia:** `Aegis-ANK/.github/workflows/docker-publish.yml`

---

## Workflow

### `.github/workflows/docker-publish.yml`

```yaml
name: Docker Publish

on:
  push:
    branches: [main]

jobs:
  build-push:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write
    steps:
      - uses: actions/checkout@v4

      - name: Build UI
        run: cd shell/ui && npm ci && npm run build

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Login to GHCR
        uses: docker/login-action@v3
        with:
          registry: ghcr.io
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Build and push
        uses: docker/build-push-action@v5
        with:
          context: .
          file: installer/Dockerfile
          push: true
          tags: |
            ghcr.io/${{ github.repository_owner }}/aegis-core:latest
            ghcr.io/${{ github.repository_owner }}/aegis-core:${{ github.sha }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
```

---

## Criterios de aceptación

- [ ] El workflow corre al hacer merge a `main`
- [ ] La imagen se publica en `ghcr.io/<org>/aegis-core:latest`
- [ ] La imagen incluye `ank-server` + `shell/ui/dist/`
- [ ] `docker pull ghcr.io/<org>/aegis-core:latest && docker run -e AEGIS_ROOT_KEY=test -p 8000:8000 ...` funciona

## Referencia

`Aegis-ANK/.github/workflows/docker-publish.yml`

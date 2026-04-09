# CORE-036 — shell/ui: build integrado — dist/ embebido en ank-server

**Épica:** 32 — Unified Binary
**Fase:** 4 — Web UI
**Repo:** Aegis-Core — cross-repo (shell/ui + kernel/crates/ank-server)
**Asignado a:** Shell Engineer + Kernel Engineer
**Prioridad:** 🔴 Alta — cierra el ciclo del binario único
**Estado:** DONE
**Depende de:** CORE-016, CORE-032, CORE-033, CORE-034, CORE-035

---

## Contexto

Para que `ank-server` sirva la UI sin archivos externos, el `dist/` de React
debe estar disponible en runtime. Hay dos estrategias — este ticket implementa
la más simple primero (path configurable) y deja la embedded como opción.

---

## Estrategia A — Path configurable (implementar ahora)

`ank-server` lee `UI_DIST_PATH` del entorno o busca `shell/ui/dist/`
relativo al binario. `CORE-016` ya implementa el handler — este ticket
conecta el build de la UI con ese path.

**Script de build completo** — `Makefile` o `build.sh` en la raíz:

```bash
#!/usr/bin/env bash
set -euo pipefail

echo "→ Building UI..."
cd shell/ui && npm ci && npm run build && cd ../..

echo "→ Building kernel..."
cargo build --release -p ank-server

echo "→ Setting UI_DIST_PATH..."
export UI_DIST_PATH="$(pwd)/shell/ui/dist"

echo "✓ Build complete. Run: UI_DIST_PATH=$UI_DIST_PATH ./target/release/ank-server"
```

## Estrategia B — Embebido en el binario (feature flag, opcional)

Usando `include_dir` crate con feature `embed-ui`:

```toml
# kernel/crates/ank-http/Cargo.toml
[dependencies]
include_dir = { version = "0.7", optional = true }

[features]
embed-ui = ["dep:include_dir"]
```

```rust
// static_files.rs
#[cfg(feature = "embed-ui")]
static UI_DIR: include_dir::Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../../shell/ui/dist");
```

Build con UI embebida:
```bash
npm run build --prefix shell/ui
cargo build --release -p ank-server --features embed-ui
```

El binario resultante no necesita `UI_DIST_PATH` — sirve la UI desde memoria.

---

## Criterios de aceptación

- [ ] `./build.sh` (o `make build`) compila UI + kernel en secuencia
- [ ] `ank-server` con `UI_DIST_PATH=shell/ui/dist` sirve la UI en `GET /`
- [ ] `GET /` retorna `index.html` con status 200
- [ ] `GET /assets/index-*.js` retorna el bundle con Content-Type correcto
- [ ] La feature `embed-ui` compila y el binario resultante sirve la UI sin env vars
- [ ] El `README.md` raíz documenta el proceso de build completo

## Archivos a crear

- `build.sh` en raíz de `aegis-core/`
- `Makefile` con targets `build`, `dev`, `clean`

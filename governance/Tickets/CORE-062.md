# CORE-062 — CI: Native binary publish — GitHub Releases

**Épica:** 32 — Unified Binary
**Fase:** 7 — CI/CD y Governance
**Repo:** Aegis-Core — `.github/workflows/`
**Asignado a:** DevOps Engineer
**Prioridad:** 🟡 Media
**Estado:** DONE
**Depende de:** CORE-060

---

## Contexto

Publicar binarios nativos de `ank-server` en GitHub Releases para
que el installer nativo pueda descargarlos sin Docker.

**Referencia:** `Aegis-ANK/.github/workflows/publish-native.yml`

---

## Targets de compilación

| Target | OS | Arch |
|---|---|---|
| `x86_64-unknown-linux-gnu` | Linux | x86_64 |
| `aarch64-unknown-linux-gnu` | Linux | ARM64 |
| `x86_64-pc-windows-msvc` | Windows | x86_64 |
| `x86_64-apple-darwin` | macOS | x86_64 |
| `aarch64-apple-darwin` | macOS | ARM64 (M1/M2/M3) |

## Workflow

### `.github/workflows/publish-native.yml`

```yaml
name: Native Binary Publish

on:
  push:
    tags: ['v*']        # release oficial
  schedule:
    - cron: '0 2 * * *' # nightly build

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: ank-server-linux-x86_64
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            artifact: ank-server-linux-arm64
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: ank-server-windows-x86_64.exe
          - os: macos-latest
            target: aarch64-apple-darwin
            artifact: ank-server-macos-arm64

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { targets: ${{ matrix.target }} }
      - run: cargo build --release -p ank-server --target ${{ matrix.target }}
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.artifact }}
          path: target/${{ matrix.target }}/release/ank-server*

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - name: Download all artifacts
        uses: actions/download-artifact@v4
      - name: Create/Update nightly release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: nightly
          files: '**/*'
```

---

## Criterios de aceptación

- [ ] El workflow compila para Linux x86_64 y ARM64 sin errores
- [ ] Los binarios aparecen en GitHub Releases bajo el tag `nightly`
- [ ] El installer nativo puede descargar el binario correcto para la arquitectura del host

## Referencia

`Aegis-ANK/.github/workflows/publish-native.yml`

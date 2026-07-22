# Aegis OS — Distro NixOS & Bare-Metal Installation Guide

> **Version:** 1.0.0
> **Target:** Declarative NixOS Building, Dual Delivery Modes, and Bare-Metal Deployment

---

## 1. Architecture Overview

Aegis OS provides a fully declarative Linux distribution built on NixOS. It supports **Dual Delivery**:

```
                       ┌─────────────────────────┐
                       │    Aegis OS (NixOS)     │
                       └────────────┬────────────┘
                                    │
                    ┌──────────────┴──────────────┐
                    ▼                             ▼
       [Modo Servidor (Headless)]    [Modo Usuario (Kiosk GUI)]
       - Sin entorno gráfico         - Compositor Wayland (Cage)
       - Acceso remoto (Web/App)     - Navegador en pantalla completa
       - Ahorro de energía en laptop - Audio local PipeWire (Siren)
```

---

## 2. Declarative Flake Structure (`distro/nixos/`)

* **[flake.nix](file:///e:/Aegis/Aegis-Core/distro/nixos/flake.nix)** — Main entry point exposing `#aegis-server`, `#aegis-kiosk`, and `#aegis-iso`.
* **[configuration.nix](file:///e:/Aegis/Aegis-Core/distro/nixos/configuration.nix)** — Core system security, Citadel policies, LUKS2 disk encryption, and `ank-server` systemd daemon.
* **[profile-server.nix](file:///e:/Aegis/Aegis-Core/distro/nixos/profile-server.nix)** — Headless power-saving mode (selectively suspends display server when laptop lid is closed).
* **[profile-kiosk.nix](file:///e:/Aegis/Aegis-Core/distro/nixos/profile-kiosk.nix)** — Wayland `cage` compositor launching Chromium in fullscreen Kiosk mode targeting `http://localhost:8000`.

---

## 3. Building the Bootable ISO Image

### Prerequisites
* A Linux system running Nix with Flakes enabled:
  ```bash
  mkdir -p ~/.config/nix
  echo "experimental-features = nix-command flakes" >> ~/.config/nix/nix.conf
  ```

### Step 1: Evaluate & Verify Flake Configuration
```bash
cd distro/nixos
nix flake check
```

### Step 2: Build the ISO Image
```bash
nix build .#nixosConfigurations.aegis-iso.config.system.build.isoImage
```
The compiled ISO artifact will be located under `./result/iso/aegis-os-x86_64.iso`.

---

## 4. Flashing and Disk Installation

### Flashing to USB Media
Replace `/dev/sdX` with your target USB drive device path:

```bash
sudo dd if=result/iso/aegis-os-x86_64.iso of=/dev/sdX bs=4M status=progress conv=fsync
```

### Disk Installation with LUKS2 Encryption
Boot the target laptop/server from the USB drive and run the automated installer:

```bash
sudo aegis-install --target /dev/nvme0n1 --encrypted
```

The installer configures:
1. **Partitioning:** EFI boot partition + LUKS2 encrypted root filesystem (SQLCipher alignment).
2. **Systemd Service:** `ank-server` starts automatically on boot as non-root service user `aegis`.
3. **Network Setup:** Firewall rules opening port `8000` (Web UI), `50051` (gRPC), and `Aegis Connect` persistent WebSocket tunnels.

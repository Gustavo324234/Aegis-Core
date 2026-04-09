# distro/

**Status: PLANNED — post-Epic 32**

Reserved for the Aegis Linux distribution.

## Vision

A minimal, immutable Linux base with the Aegis cognitive kernel embedded at the
OS level. ANK runs as a first-class system service with direct hardware access
for GPU inference.

## Target specs

- Base: Buildroot or NixOS (TBD)
- Architectures: x86_64, ARM64 (Raspberry Pi 5, NVIDIA Jetson)
- Boot: ANK starts at boot, serves the web UI immediately
- Storage: read-only root, writable `/data` partition (SQLCipher encrypted)
- No display manager — Aegis Shell is the UI

## Prerequisites

Epic 32 (unified binary) must be complete before this directory has content.
A single, dependency-free binary is what makes embedding into a distro feasible.

## Future repo

When this grows large enough, it will split into `aegis-distro` — a separate
repository that depends on `aegis-core` as a build input.

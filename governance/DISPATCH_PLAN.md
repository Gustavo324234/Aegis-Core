# DISPATCH — Prompts para agentes especialistas
# Aegis-Core — Epic 32: Unified Binary
# Fecha: 2026-04-08

---

## DIAGRAMA DE DEPENDENCIAS Y PARALELISMO

```
RONDA 1 (paralelo — sin dependencias entre sí):
  [Kernel Engineer A]  CORE-002  ank-proto
  [Kernel Engineer B]  CORE-004  ank-mcp
  [Kernel Engineer C]  CORE-005  aegis-sdk
  [Kernel Engineer D]  CORE-006  ank-cli
  [Shell Engineer]     CORE-030  shell/ui setup

RONDA 2 (requiere CORE-002 + CORE-004):
  [Kernel Engineer]    CORE-003  ank-core  ← bloquea todo lo demás del kernel

RONDA 3 (requiere CORE-003, paralelo entre sí):
  [Kernel Engineer A]  CORE-010  ank-http scaffolding
  [DevOps Engineer]    CORE-041  docker-compose.yml
  [DevOps Engineer]    CORE-042  systemd unit
  [Shell Engineer]     CORE-031  stores Zustand

RONDA 4 (requiere CORE-010):
  [Kernel Engineer]    CORE-011  CitadelLayer

RONDA 5 (requiere CORE-011, paralelo entre sí):
  [Kernel Engineer A]  CORE-012  endpoints REST auth+admin+engine
  [Kernel Engineer B]  CORE-014  WebSocket /ws/chat
  [Kernel Engineer C]  CORE-016  static file serving
  [Shell Engineer]     CORE-032  componentes core UI
  [Shell Engineer]     CORE-033  componentes auth UI

RONDA 6 (requiere CORE-012):
  [Kernel Engineer A]  CORE-013  endpoints REST router+status+workspace
  [Kernel Engineer B]  CORE-015  WebSocket /ws/siren
  [Shell Engineer]     CORE-034  componentes providers UI
  [Shell Engineer]     CORE-035  Siren UI

RONDA 7 (requiere CORE-010 + CORE-012 + CORE-013 + CORE-014 + CORE-015 + CORE-016):
  [Kernel Engineer]    CORE-020  ank-server main.rs (integración final)

RONDA 8 (requiere CORE-020):
  [Kernel Engineer]    CORE-021  aegis-supervisor simplificado
  [Shell Engineer]     CORE-036  build integrado dist/
  [DevOps Engineer]    CORE-040  install.sh unificado

RONDA 9 (requiere CORE-040):
  [DevOps Engineer]    CORE-043  aegis CLI

RONDA 10 (paralelo — requieren CORE-020 funcionando):
  [Mobile Engineer A]  CORE-050  app setup
  [CI Engineer]        CORE-060  GitHub Actions CI

RONDA 11 (requieren ronda 10):
  [Mobile Engineer]    CORE-051  app stores y servicios
  [CI Engineer]        CORE-061  Docker publish
  [CI Engineer]        CORE-062  Native binary publish

RONDA 12 (requiere CORE-051):
  [Mobile Engineer]    CORE-052  app pantallas

RONDA 13 (todo funcionando):
  [Arquitecto IA]      CORE-063  governance docs
```

---

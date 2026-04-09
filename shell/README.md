# shell/

Aegis Web Interface — React/Vite/TypeScript

Interfaz web del sistema. Thin client: renderiza estado, streamea output,
provee controles de admin. Toda la lógica cognitiva corre en el kernel.

**Diferencia clave vs. legacy:** No hay BFF Python. La UI habla HTTP/WS
directamente con `ank-server`. Los endpoints son idénticos a los que hoy
sirve el BFF — misma UI, distinto backend.

## Estructura

```
shell/
└── ui/          React 18 + Vite + TypeScript + Zustand + Tailwind
```

No hay `bff/`. El servidor HTTP vive en `kernel/crates/ank-http/`.

## Referencia legacy

`Aegis-Shell/ui/` — leer para entender stores Zustand, componentes y
lógica de UI existente. No modificar.

## Build

```bash
cd ui && npm install && npm run build
```

# CORE-030 — shell/ui: setup React + Vite + TypeScript + Zustand + Tailwind

**Épica:** 32 — Unified Binary
**Fase:** 4 — Web UI
**Repo:** Aegis-Core — `shell/ui/`
**Asignado a:** Shell Engineer
**Prioridad:** 🔴 Alta — bloquea CORE-031 a CORE-036
**Estado:** COMPLETED
**Depende de:** CORE-001

---

## Contexto

Inicializar el proyecto UI en `shell/ui/` con el mismo stack que el legacy.
La UI habla directamente con `ank-server` HTTP/WS — sin BFF Python.
Los endpoints son idénticos, el cambio es transparente para la UI.

**Referencia:** `Aegis-Shell/ui/package.json`, `vite.config.ts`, `tsconfig.json`

---

## Trabajo requerido

### 1. `shell/ui/package.json`

```json
{
  "name": "aegis-ui",
  "private": true,
  "version": "1.0.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "lint": "eslint . --ext ts,tsx --max-warnings 0",
    "preview": "vite preview"
  },
  "dependencies": {
    "clsx": "^2.1.0",
    "framer-motion": "^11.0.8",
    "lucide-react": "^0.344.0",
    "react": "^18.2.0",
    "react-dom": "^18.2.0",
    "react-markdown": "^9.0.1",
    "react-virtualized-auto-sizer": "^1.0.24",
    "react-window": "^1.8.10",
    "remark-gfm": "^4.0.0",
    "tailwind-merge": "^2.2.1",
    "zustand": "^4.5.2"
  },
  "devDependencies": {
    "@types/react": "^18.2.64",
    "@types/react-dom": "^18.2.21",
    "@types/react-window": "^1.8.8",
    "@typescript-eslint/eslint-plugin": "^7.1.1",
    "@typescript-eslint/parser": "^7.1.1",
    "@vitejs/plugin-react": "^4.2.1",
    "autoprefixer": "^10.4.18",
    "eslint": "^8.57.0",
    "postcss": "^8.4.35",
    "tailwindcss": "^3.4.1",
    "typescript": "^5.2.2",
    "vite": "^8.0.5"
  }
}
```

### 2. `shell/ui/vite.config.ts`

En modo dev, proxear `/api` y `/ws` a `ank-server` en :8000:

```typescript
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/api': 'http://localhost:8000',
      '/ws':  { target: 'ws://localhost:8000', ws: true },
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
})
```

### 3. Copiar configuración base

Portar desde `Aegis-Shell/ui/`:
- `tsconfig.json`
- `tsconfig.node.json`
- `tailwind.config.ts`
- `postcss.config.js`
- `index.html`
- `.eslintrc.cjs`

---

## Criterios de aceptación

- [x] `cd shell/ui && npm install` termina sin errores
- [x] `npm run build` produce `shell/ui/dist/` sin errores TypeScript
- [x] `npm run dev` inicia el servidor en :5173
- [x] En dev, `/api/health` proxea correctamente a `ank-server:8000/health`
- [x] `npm run lint` → 0 warnings

## Referencia

`Aegis-Shell/ui/` — configuración a portar

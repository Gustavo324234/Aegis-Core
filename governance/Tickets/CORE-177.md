# CORE-177 — feat: CodeViewer — Árbol de Archivos y Contenido en Dashboard

**Epic:** 44 — Developer Workspace  
**Repo:** Aegis-Core / shell/ui  
**Tipo:** feat  
**Estado:** ✅ Done  
**Prioridad:** Alta  
**Asignado a:** Shell Engineer  
**Dependencias:** CORE-170 (endpoints /api/fs/tree y /api/fs/file)

---

## Contexto

El tenant puede explorar el código del proyecto directamente desde el Dashboard. El CodeViewer tiene dos partes: un árbol de archivos a la izquierda y un visor de contenido a la derecha, con syntax highlighting básico.

Se integra en `Dashboard.tsx` como una sección de ancho completo con layout dividido.

---

## Cambios Requeridos

### 1. Nuevo archivo: `src/components/CodeViewer.tsx`

**Layout del componente:**

```
┌─ CÓDIGO DEL PROYECTO ─────────────────────────────────────────┐
│                                                               │
│  [árbol de archivos - 30%]  │  [contenido del archivo - 70%] │
│                             │                                 │
│  📁 kernel/                 │  // ank-core/src/agents/node.rs │
│    📁 crates/               │  use uuid::Uuid;                │
│      📁 ank-core/           │                                 │
│        📁 src/              │  pub type AgentId = Uuid;       │
│          📄 lib.rs          │                                 │
│          📁 agents/    ←sel │  #[derive(Debug, Clone...)]     │
│            📄 mod.rs        │  pub enum AgentRole {           │
│            📄 node.rs  ←sel │      ProjectSupervisor,         │
│        📄 Cargo.toml        │      DomainSupervisor,          │
│  📁 shell/                  │      Specialist,                │
│  📄 README.md               │  }                              │
│                             │                                 │
└───────────────────────────────────────────────────────────────┘
```

**Props:**

```typescript
interface CodeViewerProps {
  tenantId: string | null;
  sessionKey: string | null;
}
```

**Estado interno:**

```typescript
interface FsEntry {
  name: string;
  path: string;
  kind: 'File' | 'Directory';
  children?: FsEntry[];
  extension?: string;
}

const [tree, setTree] = useState<FsEntry[]>([]);
const [selectedPath, setSelectedPath] = useState<string | null>(null);
const [fileContent, setFileContent] = useState<{ content: string; language: string; lines: number } | null>(null);
const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set());
const [loadingFile, setLoadingFile] = useState(false);
const [treeError, setTreeError] = useState<string | null>(null);
```

**Fetch del árbol:**

```typescript
// Al montar el componente, fetch a GET /api/fs/tree?path=.
// Si retorna 400/500 o project_root no configurado, mostrar mensaje:
// "Configurá el directorio del proyecto en Workspace Settings"
```

**Fetch del archivo al clickear:**

```typescript
const loadFile = async (path: string) => {
  setLoadingFile(true);
  const res = await fetch(`/api/fs/file?path=${encodeURIComponent(path)}`, {
    headers: { 'x-citadel-tenant': tenantId!, 'x-citadel-key': sessionKey! }
  });
  if (res.ok) {
    const data = await res.json();
    setFileContent(data);
    setSelectedPath(path);
  }
  setLoadingFile(false);
};
```

**Syntax highlighting:**

Usar `highlight.js` si ya está en el proyecto, o coloreo manual por extensión con clases CSS. No instalar librerías pesadas. Si no hay highlighter disponible, mostrar el contenido en `<pre>` con fuente monospace — prioridad es que funcione, no que sea perfecto.

**Iconos por extensión:**

```typescript
const FILE_ICONS: Record<string, string> = {
  rs: '🦀',
  ts: '🔷',
  tsx: '⚛️',
  toml: '⚙️',
  json: '{}',
  md: '📝',
  yaml: '📋',
  sh: '💻',
};
const DEFAULT_ICON = '📄';
```

**Estado vacío (project_root no configurado):**

```
[ 📁 ]
No hay proyecto configurado
Configurá el directorio en Workspace Settings
```

### 2. Integración en `Dashboard.tsx`

```tsx
import CodeViewer from './CodeViewer';

{/* Code Viewer — Epic 44 */}
<div className="col-span-12 space-y-6">
  <div className="flex items-center gap-3">
    <Code2 className="w-5 h-5 text-aegis-cyan" />
    <h2 className="text-xl font-bold uppercase tracking-widest">Código del Proyecto</h2>
  </div>
  <CodeViewer tenantId={tenantId} sessionKey={sessionKey} />
</div>
```

Agregar `Code2` al import de `lucide-react`.

---

## Criterios de Aceptación

- [ ] El árbol de archivos se carga al montar el componente
- [ ] Hacer click en un directorio lo expande/colapsa
- [ ] Hacer click en un archivo carga su contenido en el panel derecho
- [ ] Archivos `.rs` muestran el contenido en fuente monospace con fondo oscuro
- [ ] Si el endpoint retorna `403` (archivo bloqueado), mostrar "Acceso denegado a este archivo"
- [ ] Si el endpoint retorna `404` o hay error de red, mostrar mensaje de error claro
- [ ] Si `project_root` no está configurado, mostrar el estado vacío con link a settings
- [ ] El número de líneas del archivo se muestra en el header del visor
- [ ] `npm run build` pasa sin errores TypeScript

---

*Arquitecto IA — 2026-04-24*

---

## Resolución (2026-06-12 — auditoría CORE-313)
Verificado ya implementado en main. Evidencia: components/workspace/CodeViewer.tsx existe.

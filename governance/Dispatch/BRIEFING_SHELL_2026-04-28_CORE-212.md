# BRIEFING — Shell Engineer — CORE-212
**Fecha:** 2026-04-28  
**Rama:** `fix/core-212-gemini-provider-catalog`  
**PR title:** `fix(shell): CORE-212 provider gemini en KeyManager y visibilidad de modelos en CatalogViewer`

---

## Contexto

Dos bugs en la UI de gestión de keys y catálogo de modelos. Ambos son cambios menores pero críticos
para que el usuario pueda configurar Gemini y ver todos los modelos disponibles.

---

## Fix 1 — `shell/ui/src/components/RouterConfig/GlobalKeyManager.tsx`

Leer el archivo. Buscar la línea:

```typescript
const PROVIDERS = ['anthropic', 'openai', 'groq', 'deepseek', 'mistral', 'google', 'openrouter', 'qwen', 'ollama'];
```

Reemplazar `'google'` por `'gemini'`:

```typescript
const PROVIDERS = ['anthropic', 'openai', 'gemini', 'groq', 'deepseek', 'mistral', 'openrouter', 'qwen', 'ollama'];
```

Luego leer `TenantKeyManager.tsx`. Si tiene la misma constante `PROVIDERS`, aplicar el mismo cambio.

---

## Fix 2 — `shell/ui/src/components/RouterConfig/ModelCatalogViewer.tsx`

Leer el archivo completo. Hacer los siguientes 3 cambios:

### 2a. Agregar estado `keyProviders`

Después de la línea `const [hasKeys, setHasKeys] = useState(false);`, agregar:

```typescript
const [keyProviders, setKeyProviders] = useState<Set<string>>(new Set());
```

### 2b. Modificar `fetchModels` para trackear providers con key

Dentro del bloque que procesa `globalRes` y `tenantRes`, reemplazar:

```typescript
let keyCount = 0;
if (globalRes.ok) {
    const globalData = await globalRes.json();
    keyCount += (globalData.keys || []).length;
}
if (tenantRes.ok) {
    const tenantData = await tenantRes.json();
    keyCount += (tenantData.keys || []).length;
}
setHasKeys(keyCount > 0);
```

Por:

```typescript
const providersWithKeys = new Set<string>();
if (globalRes.ok) {
    const globalData = await globalRes.json();
    (globalData.keys || []).forEach((k: { provider: string }) => providersWithKeys.add(k.provider));
}
if (tenantRes.ok) {
    const tenantData = await tenantRes.json();
    (tenantData.keys || []).forEach((k: { provider: string }) => providersWithKeys.add(k.provider));
}
setKeyProviders(providersWithKeys);
setHasKeys(providersWithKeys.size > 0);
```

### 2c. Corregir el mensaje de "sin modelos"

Buscar:
```tsx
{!hasKeys 
    ? t('no_models_in_catalog')
    : models.length === 0 
        ? t('catalog_pending_sync')
        : t('no_models_match')}
```

Reemplazar por:
```tsx
{models.length === 0
    ? t('catalog_pending_sync')
    : t('no_models_match')}
```

### 2d. Agregar columna "Key" a la tabla

En el `<thead>`, agregar después de la columna `{t('status')}`:
```tsx
<th className="text-center py-2 pl-2">Key</th>
```

En el `<tbody>`, en cada `<tr>`, agregar después de la celda de status:
```tsx
<td className="py-3 pl-2 text-center">
    {keyProviders.has(m.provider)
        ? <span className="text-[9px] font-mono text-green-400">✓</span>
        : <span className="text-[9px] font-mono text-white/20">—</span>
    }
</td>
```

---

## Verificación

```
npm run build
```

Sin errores TypeScript.

---

## Branch y commit

```
git checkout -b fix/core-212-gemini-provider-catalog

git commit -m "fix(shell): CORE-212 provider gemini en KeyManager y visibilidad de modelos en CatalogViewer"

git push origin fix/core-212-gemini-provider-catalog
```

Tavo hace el PR y merge manualmente.

---

*Arquitecto IA — 2026-04-28*

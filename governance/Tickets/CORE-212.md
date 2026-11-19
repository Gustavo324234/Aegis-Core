# CORE-212 — fix(shell): provider gemini en KeyManager y visibilidad de modelos en CatalogViewer

**Epic:** 42 — Vision Realignment  
**Repo:** Aegis-Core / shell/ui  
**Tipo:** fix  
**Estado:** ✅ Done  
**Prioridad:** Crítica  
**Asignado a:** Shell Engineer  
**Dependencias:** ninguna

---

## Contexto

Dos bugs detectados el 2026-04-28 que impiden configurar y visualizar los modelos Gemini 2.5/3.x
actualizados en `models.yaml`.

---

## Bug 1 — `GlobalKeyManager.tsx` tiene `'google'` en vez de `'gemini'`

**Archivo:** `shell/ui/src/components/RouterConfig/GlobalKeyManager.tsx`  
**Línea 17:**

```typescript
const PROVIDERS = ['anthropic', 'openai', 'groq', 'deepseek', 'mistral', 'google', 'openrouter', 'qwen', 'ollama'];
```

El `models.yaml` y el `entry_api_url()` del router identifican al provider de Gemini como `"gemini"`.
Cuando el usuario agrega su Gemini API key seleccionando `"google"`, el `KeyPool` la guarda con
`provider: "google"`. El router busca keys con `provider: "gemini"` → no encuentra ninguna → 401.

**Fix:**

```typescript
const PROVIDERS = ['anthropic', 'openai', 'gemini', 'groq', 'deepseek', 'mistral', 'openrouter', 'qwen', 'ollama'];
```

Reemplazar `'google'` por `'gemini'`. También verificar `TenantKeyManager.tsx` — si tiene la misma
lista hardcodeada, aplicar el mismo fix.

---

## Bug 2 — `ModelCatalogViewer.tsx` oculta modelos cuando no hay keys

**Archivo:** `shell/ui/src/components/RouterConfig/ModelCatalogViewer.tsx`

El viewer muestra el mensaje `"no_models_in_catalog"` cuando `hasKeys === false`, aunque el
catálogo del kernel sí tenga modelos. Esto confunde al usuario: no puede ver qué modelos están
disponibles para decidir qué key agregar.

**Comportamiento deseado:**
- Mostrar SIEMPRE todos los modelos del catálogo (`/api/router/models`).
- Agregar una columna o badge "Key" que indique si el provider tiene key activa.
- El mensaje `"no_models_in_catalog"` solo debe aparecer si `models.length === 0` (catálogo vacío).

**Fix en `fetchModels`:**

Mantener la lógica de `hasKeys` pero usarla solo como metadata para el badge, no para
ocultar el listado. Cambiar la condición de `filtered.length === 0`:

```tsx
// ANTES — oculta la tabla si no hay keys aunque haya modelos
{!hasKeys 
    ? t('no_models_in_catalog')
    : models.length === 0 
        ? t('catalog_pending_sync')
        : t('no_models_match')}

// DESPUÉS — solo muestra "catálogo vacío" si no hay modelos en absoluto
{models.length === 0
    ? t('catalog_pending_sync')
    : t('no_models_match')}
```

**Agregar columna "Key" a la tabla:**

En el `thead`:
```tsx
<th className="text-center py-2">Key</th>
```

En el `tbody`, necesitás saber qué providers tienen key. Crear un Set de providers con key:

```typescript
const [keyProviders, setKeyProviders] = useState<Set<string>>(new Set());
```

Popularlo en `fetchModels` después de obtener las keys:
```typescript
const providersWithKeys = new Set<string>();
if (globalRes.ok) {
    const d = await globalRes.json();
    (d.keys || []).forEach((k: { provider: string }) => providersWithKeys.add(k.provider));
}
if (tenantRes.ok) {
    const d = await tenantRes.json();
    (d.keys || []).forEach((k: { provider: string }) => providersWithKeys.add(k.provider));
}
setKeyProviders(providersWithKeys);
setHasKeys(providersWithKeys.size > 0);
```

En cada fila:
```tsx
<td className="py-3 text-center">
    {keyProviders.has(m.provider)
        ? <span className="text-[9px] font-mono text-green-400">✓</span>
        : <span className="text-[9px] font-mono text-white/20">—</span>
    }
</td>
```

---

## Criterios de aceptación

- [x] Al agregar una key con provider `gemini`, el backend la acepta y la lista correctamente
- [x] Los modelos Gemini 2.5 y 3.x aparecen en el `ModelCatalogViewer` aunque no haya keys
- [x] La columna "Key" muestra ✓ en verde para providers con key activa, — para los que no
- [x] `npm run build` sin errores TypeScript

---

## Commit message

```
fix(shell): CORE-212 provider gemini en KeyManager y visibilidad de modelos en CatalogViewer
```

---

*Arquitecto IA — 2026-04-28*

---

## Resolución (2026-06-12)
Verificado 2026-06-12: ambos bugs ya corregidos en main — GlobalKeyManager.tsx usa 'gemini' y ModelCatalogViewer.tsx lista siempre el catálogo con columna Key. Hardening adicional: KeyPool::load() normaliza provider ids legacy ('google' → 'gemini').

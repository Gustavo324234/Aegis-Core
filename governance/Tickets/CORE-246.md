# CORE-246 — Tenant: visualización de modelos disponibles por provider en tab Motor

**Tipo:** feat  
**Prioridad:** Alta  
**Estado:✅ Done
**Épica:** UX & Onboarding (EPIC 41)  
**Asignado a:** Shell Engineer  
**Componente:** `shell/ui/src/components/SettingsPanel.tsx` (MotorTab)

---

## Contexto

El tenant actualmente configura su motor de IA con un formulario manual: elige provider, escribe URL y nombre de modelo a mano. No tiene visibilidad de qué modelos están disponibles para él, ni puede ver los providers globales que el admin configuró.

El admin, en cambio, ve una lista rica de providers con sus modelos al usar `ProvidersTab`. El tenant debería tener una experiencia equivalente en su tab Motor.

## Cambios requeridos

### `shell/ui/src/components/SettingsPanel.tsx` — `MotorTab`

Reemplazar el formulario manual de configuración de motor por una vista en dos secciones:

---

**Sección 1 — Providers globales disponibles (read-only)**

Llamar `GET /api/router/keys/global` con las headers del tenant para obtener la lista de providers configurados por el admin. Renderizar cada provider como una card compacta similar a `ProviderCard` de `ProvidersTab`, pero en modo lectura:

```
┌─────────────────────────────────────────┐
│  🌐 Groq                    ● ACTIVO    │
│  llama-3.3-70b-versatile, llama-3.1-8b  │
└─────────────────────────────────────────┘
┌─────────────────────────────────────────┐
│  ⚡ Anthropic               ● ACTIVO    │
│  claude-sonnet-4-6, claude-haiku-4-5    │
└─────────────────────────────────────────┘
```

- Providers con `is_active: false` se muestran atenuados con badge "Inactivo".
- Providers con `rate_limited_until` futuro muestran badge "Rate limited".
- El tenant NO puede editar ni borrar providers globales desde aquí — solo visualizar.
- Mensaje informativo debajo: "Estos providers los gestiona tu administrador."

---

**Sección 2 — Mis keys personales (existente, mejorada)**

Mantener el `TenantKeyManager` actual pero agregando la visualización de modelos. Al agregar una key personal, llamar `POST /api/providers/models` con la key para obtener y mostrar los modelos disponibles — igual que hace el admin en `ProviderModal`. El tenant puede seleccionar cuáles modelos habilitar para sí mismo.

Pasos concretos:
1. Al crear una key de tenant, agregar un paso de verificación igual al admin: botón "Verificar y ver modelos" → modal con `ModelSelector`.
2. Guardar `active_models` en el POST a `/api/router/keys/tenant`.
3. En la tabla de keys existente, mostrar los modelos habilitados junto a la key (truncado a 2 + contador si hay más, igual que `ProviderCard`).

---

**Status del motor activo**

Mantener el banner de estado actual (`engineStatus`) en la parte superior de la tab, pero enriquecerlo: si el router seleccionó un provider global, mostrar de cuál provider/modelo vino la última respuesta (ya está disponible en `lastRoutingInfo` del store).

---

## Estructura sugerida del MotorTab rediseñado

```
MotorTab
├── Banner: motor activo (provider + modelo + estado)
├── Sección: "Providers del sistema" (global, read-only)
│   └── ProviderCard × N (compactas, sin acciones)
├── Divider con texto "o configurá tu propio motor"
└── TenantKeyManager mejorado (con selector de modelos al agregar)
```

El formulario manual (URL + modelo + key) que existe actualmente **se elimina** — queda reemplazado por esta experiencia. La configuración manual era un workaround que ya no tiene sentido con el router multi-provider.

## Criterios de aceptación

- [x] El tab Motor muestra los providers globales configurados por el admin con sus modelos disponibles.
- [x] Providers inactivos o rate-limited se muestran con el badge correcto.
- [x] El tenant NO puede editar providers globales desde esta vista.
- [x] Al agregar una key personal, hay un paso de verificación que obtiene y muestra los modelos disponibles via `POST /api/providers/models`.
- [x] Las keys personales del tenant muestran sus `active_models` en la tabla.
- [x] El formulario manual legacy (URL + modelo + key libre) se elimina.
- [x] El banner de estado del motor sigue funcionando y muestra `lastRoutingInfo` si está disponible.
- [x] TypeScript sin errores de compilación.

## Dependencias

- CORE-245 (no bloqueante — los componentes son independientes)
- Backend: `GET /api/router/keys/global` accesible por tenants (verificar permisos — actualmente puede ser solo-admin)
- Backend: `POST /api/router/keys/tenant` debe aceptar `active_models` en el body (verificar)

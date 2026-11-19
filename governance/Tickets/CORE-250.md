# CORE-250 — Dashboard: FinancialWidget con datos reales (API Cost)

**Tipo:** feat  
**Prioridad:** Media  
**Épica:** EPIC 48 — Shell Observability  
**Asignado a:** Shell Engineer  
**Componente:** `shell/ui/src/components/Dashboard.tsx` — `FinancialWidget`

**Estado:** ✅ Done
---

## Contexto

El `FinancialWidget` muestra "$1,240.50 Monthly Expenses" con dos gastos ficticios (AWS $85.20, OpenAI $120.00). Estos valores están hardcodeados y no representan ningún dato real del tenant.

El endpoint `GET /api/status` ya devuelve datos de telemetría de inferencia. La telemetría incluye información de tokens consumidos y modelos usados que puede transformarse en un widget útil sin requerir un plugin Ledger.

---

## Cambios requeridos

### Reemplazar `FinancialWidget` por `ApiCostWidget`

Renombrar el componente a `ApiCostWidget` y conectarlo a datos reales.

**Fuente de datos:** `GET /api/telemetry` o `GET /api/status` (verificar qué endpoint devuelve datos de inferencia acumulados). El endpoint de telemetría del Admin Dashboard ya existe (`/api/status`) — consultar si incluye tokens consumidos y modelos.

Si el endpoint devuelve datos suficientes, mostrar:

```
┌─────────────────────────────────────────────────────────┐
│  ⚡ Uso de API                      Sesión actual       │
│                                                         │
│  Tokens emitidos: 24,831                                │
│  Velocidad: 12.4 tok/s                                  │
│  Modelo activo: groq / llama-3.3-70b-versatile          │
│                                                         │
│  [Ver telemetría completa →]                            │
└─────────────────────────────────────────────────────────┘
```

Si el endpoint no tiene esos datos agregados por tenant, mostrar estado honesto:

```
┌─────────────────────────────────────────────────────────┐
│  ⚡ Uso de API                                          │
│  Sin datos de consumo disponibles para esta sesión.     │
│                                                         │
│  Activá el Plugin Ledger para seguimiento de gastos.    │
└─────────────────────────────────────────────────────────┘
```

**En cualquier caso:** eliminar los valores ficticios ($1,240.50, AWS, OpenAI) del código.

### Datos a mostrar (si disponibles)

Del endpoint de telemetría, mostrar lo que esté disponible entre:
- Tokens totales emitidos en la sesión actual
- Velocidad promedio (tokens/segundo)
- Proveedor y modelo activo (de `lastRoutingInfo` en el store — ya disponible)
- Latencia promedio

### Fallback graceful

Si la llamada al endpoint falla o devuelve datos vacíos, mostrar el estado vacío honesto. No crashear ni mostrar datos ficticios.

---

## Criterios de aceptación

- [x] `FinancialWidget` no contiene valores ficticios hardcodeados.
- [x] Si hay datos de telemetría disponibles, se muestran (tokens, velocidad, modelo).
- [x] Si no hay datos, se muestra el estado vacío descriptivo.
- [x] El componente no crashea si el endpoint falla.
- [x] TypeScript sin errores de compilación.

## Dependencias

- Verificar qué datos devuelve `GET /api/status` o `GET /api/telemetry` para el tenant
- `lastRoutingInfo` en el store (ya disponible)

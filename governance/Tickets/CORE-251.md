# CORE-251 — Dashboard: Chronos widget honesto (sin eventos ficticios)

**Tipo:** feat  
**Prioridad:** Media  
**Épica:** EPIC 48 — Shell Observability  
**Asignado a:** Shell Engineer  
**Componente:** `shell/ui/src/components/Dashboard.tsx` — widget Chronos/Neural Schedule

**Estado:** ✅ Done
---

## Contexto

El widget "Neural Schedule" muestra dos eventos ficticios hardcodeados:
- "Sync Project Context — IN 15 MIN"
- "Backup Ring 0 Identity — IN 2 HOURS"

Estos no son eventos reales del tenant. Son placeholders que quedaron del diseño inicial y nunca se conectaron a datos reales.

---

## Cambios requeridos

### Opción A — Estado vacío honesto (MVP, sin nuevo endpoint)

Si no existe `GET /api/chronos/events`, reemplazar el contenido falso por un estado vacío descriptivo:

```tsx
<div className="flex flex-col items-center justify-center py-8 gap-3 text-center">
  <Clock className="w-8 h-8 text-white/10" />
  <p className="text-xs font-mono text-white/30">Sin eventos próximos</p>
  <p className="text-[10px] font-mono text-white/20 max-w-[220px] leading-relaxed">
    Pedile a tu agente que agende algo: "Anotá una reunión para mañana a las 10".
  </p>
</div>
```

Mantener el header del widget ("Neural Schedule" + icono Clock + "Chronos Plugin Active").

### Opción B — Conectado a datos reales (si el endpoint existe)

Si `GET /api/chronos/events` existe o se crea en este ticket, consumirlo:

```
GET /api/chronos/events?limit=3&from=now
Auth: x-citadel-tenant + x-citadel-key

Response:
{
  "events": [
    { "title": "Reunión de equipo", "time": "2026-05-04T10:00:00Z", "urgency": "high" }
  ]
}
```

Renderizar hasta 3 eventos próximos con tiempo relativo ("en 15 min", "mañana a las 10").

**Para este ticket, implementar Opción A.** Si el Plugin Chronos tiene endpoint REST disponible, implementar Opción B. El Shell Engineer debe verificar con el Kernel Engineer si `GET /api/chronos/events` existe antes de decidir.

---

## Criterios de aceptación

- [x] Los eventos ficticios ("Sync Project Context", "Backup Ring 0 Identity") eliminados del código.
- [x] Se muestra el estado vacío con el mensaje descriptivo, O eventos reales si el endpoint existe.
- [x] El header del widget ("Neural Schedule") se mantiene.
- [x] TypeScript sin errores de compilación.

## Dependencias

- Verificar disponibilidad de `GET /api/chronos/events` con Kernel Engineer

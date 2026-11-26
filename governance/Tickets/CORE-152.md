# CORE-152 — Feat: Plugins de Dominios (Ledger & Chronos)

**Epic:** 42 — Vision Realignment & Autonomy
**Repo:** Aegis-Core
**Tipo:** feat
**Prioridad:** Media
**Asignado a:** Kernel Engineer
**Estado:** ✅ Completo

---

## Problema

Aegis no tiene herramientas nativas para gestionar aspectos específicos de la vida del usuario como gastos financieros o recordatorios/calendario, limitándose a ser un asistente de chat generalista.

## Solución Propuesta

Desarrollar dos plugins base integrados en el Kernel:

### 1. Ledger (Finanzas)
- **Función:** Registrar entradas/salidas de dinero.
- **Persistencia:** Tabla `expenses` en la base de datos SQLCipher del tenant.
- **Comando:** "Gaste $10000 en el súper" -> `ledger.add_expense(10000, "super")`.

### 2. Chronos (Tiempo)
- **Función:** Gestionar recordatorios y eventos.
- **Persistencia:** Tabla `reminders` en la DB del tenant.
- **Comando:** "Recuérdame una reunión mañana a las 10" -> `chronos.add_reminder("2026-04-24 10:00", "Reunión")`.

---

## Criterios de Aceptación

- [x] Los gastos se guardan y pueden ser consultados (ej. "¿Cuánto gasté este mes?").
- [x] Los recordatorios se guardan correctamente.
- [x] Integración con el sistema de notificaciones (futuro).

---

*Arquitecto IA — 2026-04-23*
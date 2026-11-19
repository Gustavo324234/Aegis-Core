# CORE-321 — App/Android: deshabilitar el file-watcher nativo de Gradle (crashes JVM en Windows)

**Tipo:** chore
**Prioridad:** Alta
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure
**Estado:** ✅ Done
**Asignado a:** DevOps Engineer

---

## Problema

Los volcados `hs_err_pid3296.log` / `hs_err_pid21048.log` (analizados en `governance/aegis_jvm_crash_report.md`) muestran `EXCEPTION_ACCESS_VIOLATION` dentro de `native-platform-file-events.dll` — el watcher nativo de archivos de Gradle — durante compilaciones del cliente Android en Windows. La condición de carrera se dispara bajo carga concurrente (cargo escribiendo `target/`, indexación del IDE y el Kotlin daemon en paralelo) y mata al Gradle Daemon entero.

## Solución propuesta

Aplicar la remediación inmediata (Opción A del reporte): añadir `org.gradle.vfs.watch=false` a `app/android/gradle.properties`. Elimina la ejecución del DLL problemático sin afectar la integridad del build (solo se pierde la invalidación incremental por watcher, irrelevante en CI y builds limpias).

## Criterios de aceptación

- [x] `app/android/gradle.properties` contiene `org.gradle.vfs.watch=false` con comentario referenciando este ticket.
- [x] El resto de la configuración de Gradle queda intacta.

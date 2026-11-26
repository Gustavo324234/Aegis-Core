# CORE-326 — Kernel: handshake SCM oportunista para ImagePath legacy sin --service

**Tipo:** fix
**Prioridad:** Alta
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure
**Estado:** ✅ Done
**Asignado a:** Kernel Engineer

---

## Problema

Desde PR #331, `ank-server` solo entra en modo servicio de Windows cuando recibe el flag `--service` (main.rs). Los servicios registrados por installers anteriores a ese cambio tienen un `ImagePath` *sin* el flag (`"C:\Program Files\Aegis\ank-server.exe"`). Al actualizar el binario en esas máquinas, el SCM lanza el proceso, este arranca en modo consola, nunca completa el handshake, y el SCM lo mata a los 30 segundos con **ERROR 1053** (eventos 7000/7009) — reproducido en producción el 2026-06-12 al actualizar a v0.1.87.

`installer/install.ps1` ya repara el `ImagePath` en su flujo de update (líneas 484-486), pero cualquier reemplazo manual del binario o instalación legacy no migrada queda rota.

## Solución implementada

En `main()` (Windows), cuando NO viene `--service`, se intenta el dispatcher del SCM de forma oportunista (`windows_service_impl::try_dispatch_legacy()`):

- Lanzado por el SCM → el dispatcher conecta, el handshake se completa y el proceso corre como servicio (el dispatcher bloquea hasta el stop).
- Lanzado desde una consola real → `StartServiceCtrlDispatcherW` falla inmediatamente con `ERROR_FAILED_SERVICE_CONTROLLER_CONNECT` y se continúa con el modo consola normal.

Cualquier error del dispatcher se trata como "no estamos bajo el SCM" — el modo de fallo es idéntico al comportamiento anterior, nunca peor.

## Criterios de aceptación

- [x] Un servicio con `ImagePath` legacy (sin `--service`) arranca correctamente con el binario nuevo.
- [x] La ejecución desde consola (incluido `--version`) no cambia de comportamiento.
- [x] `cargo check -p ank-server` y clippy en verde.

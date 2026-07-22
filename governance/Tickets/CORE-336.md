# CORE-336 — B5 — On-ramp: Instalador robusto, releases taggeadas y verificación SHA256

**Tipo:** chore
**Prioridad:** Alta
**Épica:** EPIC 56 — Public MVP / Thesis Validation
**Estado:** ✅ Done
**Asignado a:** DevOps Engineer

---

## Problema

Para una plataforma que promueve la privacidad absoluta y un modelo "Zero-Trust", obligar a los usuarios a pipear un script bash desde la rama main directamente a root (`curl | sudo bash`) sin verificación de integridad ni versiones fijas es una mala práctica que genera desconfianza. Además, el instalador descarga actualmente compilaciones nightly por defecto, lo que incrementa el riesgo de inestabilidad post-instalación.

## Solución propuesta

Optimizar la experiencia de instalación de Aegis Overlay:
1. Modificar el script de instalación (`install.sh` y el equivalente para Windows) para utilizar de forma predeterminada la última tag de release estable publicada (en lugar del build nightly).
2. Generar y publicar archivos `SHA256SUMS` firmados para cada release de binario en GitHub, integrando la comprobación de integridad en el script del instalador.
3. Promover y documentar en primer plano el despliegue mediante Docker local y el flujo alternativo de inspección manual del script (`curl -o install.sh && less install.sh && bash install.sh`).
4. Asegurar que el daemon del servicio corra bajo un usuario de sistema dedicado `aegis` no-root con hardening adecuado.

## Criterios de aceptación

- [x] Script de instalación adaptado para trabajar con releases fijas y verificación SHA256 (`install.sh` e `install.ps1`).
- [x] Documentación del README actualizada con la guía paso a paso del instalador seguro y rootless.
- [x] Docker Compose local documentado y testeado para permitir levantar Aegis en 1 click sin tocar dependencias del sistema operativo host.
- [x] Ejecución del instalador en entornos de prueba locales (Linux y Windows) exitosa y sin solicitar permisos root innecesarios.

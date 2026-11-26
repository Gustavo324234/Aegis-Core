# CORE-314 — Kernel/Shell: Deshabilitación de proveedores en KeyPool/UI no persiste

**Tipo:** fix  
**Prioridad:** Alta  
**Épica:** EPIC 53 — Stabilization: Agent Loop, Observability & Infrastructure  
**Estado:** ✅ Done  
**Asignado a:** Tavo + Kernel Engineer + Shell Engineer  

---

## Problema
El usuario reporta que al intentar deshabilitar/desactivar proveedores o llaves desde la interfaz gráfica (UI), los cambios no se guardan ni persisten en el backend. 

### Diagnóstico Técnico
1. **Diferencia de Ejecutable:** Las pruebas empíricas del usuario en su terminal Powershell de Administrador se ejecutaron usando el binario precompilado antiguo en `C:\Program Files\Aegis\ank-server.exe` en lugar de recompilar y ejecutar el código del espacio de trabajo con los últimos parches aplicados a `router_api.rs` y `key_pool.rs`.
2. **Definición de Campos en backend:** Confirmamos que en el backend (`router_api.rs`), el struct `KeyAddRequest` y los endpoints de actualización de llaves (`update_global_key` y `update_tenant_key`) ya fueron parcheados para aceptar y mapear el campo opcional `is_active: Option<bool>`.
3. **Flujo de UI (Frontend):** Se debe verificar si el componente de UI del frontend (ej. `KeyManager` o similar en `shell/ui/src/`) está enviando correctamente el campo `is_active` con el estado booleano modificado en la carga útil (payload) de la petición HTTP `PUT`.

## Solución propuesta
1. **Instrucciones al usuario:** Asegurarse de ejecutar el servidor local usando cargo para validar los cambios reales del workspace:
   ```powershell
   $env:AEGIS_ENV_FILE="C:\ProgramData\Aegis\aegis.env"; $env:RUST_LOG="info"; cargo run -p ank-server
   ```
2. **Auditoría de Frontend:** Inspeccionar la UI de la Shell para garantizar que al alternar el switch de activación de una llave/proveedor se envíe un payload completo incluyendo `"is_active": false` (o `true`) en lugar de omitir el campo o enviarlo con formato incorrecto.
3. **Cierre de Brecha de persistencia:** Validar que al guardar en `KeyPool::add_tenant_key` o `add_global_key`, el JSON del PCB en base de datos cifrada SQLCipher se actualice de inmediato.

## Criterios de aceptación
- [ ] La UI envía `"is_active": false` al desmarcar una llave.
- [ ] El backend guarda la propiedad de forma persistente en SQLCipher.
- [ ] Al reiniciar el servidor `ank-server` con la llave deshabilitada, esta se carga con `is_active = false` (excluida del CMR router).

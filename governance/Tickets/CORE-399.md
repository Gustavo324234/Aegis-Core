# CORE-399: Integridad `TenantDB` y Symlink Jail (`safe_resolve` en `ReadFile`, `ScribeManager` y `fs.rs`)

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Épica:** EPIC 61
- **Asignado a:** Kernel Engineer

---

## Description

Proteger la base de datos cifrada de cada tenant contra cuarentena accidental y cerrar escapes por enlaces simbólicos en operaciones de archivos:
- Eliminar la cuarentena automática en `TenantDB::open` (`enclave/mod.rs`) ante fallos de descifrado, restringiendo cualquier rotación/respaldo a una ruta explícita tras reset de contraseña, y rechazar `session_key` vacío o `"default"` en `chal/mod.rs`, `syscalls/mod.rs` y `ank-server/src/main.rs`.
- Aplicar `vcm::safe_resolve` en `Syscall::ReadFile` (`syscalls/mod.rs`, bloqueando también archivos sensibles `.env`/`.key`) y en `ScribeManager::write_and_commit` (`scribe/mod.rs`).
- Validar `canonicalize().starts_with(root)` al recorrer directorios y bloquear archivos de credenciales JSON sensibles en `ank-http/src/routes/fs.rs`.

## Acceptance Criteria

- [x] Un PCB sin `session_key` jamás puede renombrar o recrear el `memory.db` de un tenant existente.
- [x] Symlinks dentro del workspace apuntando fuera de la raíz son rechazados por `ReadFile`, `WriteFile` (`ScribeManager`) y `fs_tree`/`fs_file`.
- [x] Tests de regresión cubren ambos escenarios.

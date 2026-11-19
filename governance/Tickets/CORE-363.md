# CORE-363: Harden WASM & JS Maker Sandboxes: StoreLimits, SSRF Redirect/IPv6 Guard, Tainted Enforcement, and Path Traversal Fixes

- **Tipo:** fix
- **Estado:** ✅ Done
- **Prioridad:** Crítica
- **Asignado a:** Kernel Engineer

## Description
1. `ank-server` never invoked `PluginManager::load_all_from_dir` at startup (leaving existing `.wasm` plugins unloaded until modified), `watcher.rs` compiled untrusted `.wasm` modules with Cranelift before verifying Ed25519 signatures, and `PluginSigner` rejected hex-encoded `.wasm.sig` files.
2. `PluginManager::execute_plugin` wrote `TAINTED` status to `TenantDB` on `MemoryOutOfBounds` traps without ever checking it on subsequent executions, lacked `StoreLimits` linear memory caps and wall-clock timeouts, did not sanitize `tenant_id` when constructing WASI preopened paths, and `fetch_url_safe` was vulnerable to IPv4-mapped IPv6 (`::ffff:127.0.0.1`), CGNAT, DNS rebinding, HTTP 302 redirect SSRF, and unbounded response body OOM.
3. `MakerExecutor` (`ank-core/src/syscalls/maker.rs`) ran `boa_engine` synchronously on Tokio worker threads with no `RuntimeLimits`, allowed Windows drive-letter (`C:\...`) and symlink escapes in `read_file`/`write_file`, and unconditionally corrupted `\n`/`\t` escapes inside valid JS string literals.

## Acceptance Criteria
- [x] Load `.wasm` plugins from `{data_dir}/plugins` on `ank-server` boot, verify Ed25519 signatures before Cranelift compilation in `watcher.rs` (supporting raw 64-byte and hex `.wasm.sig`), and enforce `TAINTED` plugin blocking, `StoreLimits`, wall-clock timeouts, and `tenant_id` path sanitization in `PluginManager`.
- [x] Harden `fetch_url_safe` with strict `http`/`https` validation, IPv4-mapped IPv6 & CGNAT blocking, pinned `SocketAddr` resolution, `redirect::Policy::none()`, and a 2 MiB body cap.
- [x] Enforce `boa_engine` `RuntimeLimits` + `spawn_blocking` timeout in `MakerExecutor`, jail `read_file`/`write_file` via component + canonical workspace checks with 2 MiB size caps, and preserve valid JS string escapes.

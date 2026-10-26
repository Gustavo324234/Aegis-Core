use crate::syscalls::SyscallError;
use boa_engine::{Context, JsError, JsValue, Source};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Tamaño máximo permitido por archivo leído/escrito desde el sandbox JS (2 MiB).
const MAX_MAKER_FILE_BYTES: usize = 2 * 1024 * 1024;
/// Límite máximo de iteraciones de bucle en Boa Engine para evitar bloqueo de CPU.
const MAX_JS_LOOP_ITERATIONS: u64 = 200_000;
/// Límite máximo de recursión en funciones JS.
const MAX_JS_RECURSION_DEPTH: usize = 128;
/// Límite de stack de la VM JS.
const MAX_JS_STACK_SIZE: usize = 4096;
/// Timeout total de reloj para la ejecución de un script Maker.
const MAX_MAKER_TIMEOUT: Duration = Duration::from_secs(10);

/// Convierte secuencias de escape literales (`\n`, `\t`, `\r`) únicamente FUERA de
/// literales de string (`'...'`, `"..."`, `` `...` ``), para soportar código JS
/// doblemente escapado por el LLM sin corromper strings válidos como `split("\n")`.
fn unescape_js_outside_strings(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_quote: Option<char> = None;

    while let Some(c) = chars.next() {
        if let Some(q) = in_quote {
            out.push(c);
            if c == '\\' {
                if let Some(escaped) = chars.next() {
                    out.push(escaped);
                }
            } else if c == q {
                in_quote = None;
            }
            continue;
        }

        if c == '\'' || c == '"' || c == '`' {
            in_quote = Some(c);
            out.push(c);
        } else if c == '\\' {
            match chars.peek() {
                Some('n') => {
                    chars.next();
                    out.push('\n');
                }
                Some('t') => {
                    chars.next();
                    out.push('\t');
                }
                Some('r') => {
                    chars.next();
                    out.push('\r');
                }
                Some('\\') => {
                    chars.next();
                    out.push('\\');
                }
                _ => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Resuelve y confina una ruta relativa dentro de `{base_dir}/users/{tenant_id}/workspace`,
/// bloqueando path traversal (`..`), prefijos de unidad Windows (`C:\`), rutas absolutas y symlinks.
fn resolve_jailed_workspace_path(
    base_dir: &str,
    tenant_id: &str,
    rel_path: &str,
    for_write: bool,
) -> Result<PathBuf, String> {
    if tenant_id.is_empty()
        || tenant_id == "."
        || tenant_id == ".."
        || tenant_id.contains("..")
        || tenant_id.contains('/')
        || tenant_id.contains('\\')
        || tenant_id.contains(':')
        || tenant_id.contains('\0')
    {
        return Err("Security Error: Invalid tenant workspace scope".to_string());
    }

    let trimmed = rel_path.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || trimmed.contains(':')
        || trimmed.contains('\0')
    {
        return Err("Security Error: Path traversal blocked".to_string());
    }

    let mut clean_rel = PathBuf::new();
    for part in trimmed.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err("Security Error: Path traversal blocked".to_string());
        }
        clean_rel.push(part);
    }

    if clean_rel.as_os_str().is_empty() {
        return Err("Security Error: Invalid empty file path".to_string());
    }

    let workspace_raw = Path::new(base_dir)
        .join("users")
        .join(tenant_id)
        .join("workspace");
    std::fs::create_dir_all(&workspace_raw)
        .map_err(|e| format!("IO Error: Failed to initialize workspace: {}", e))?;

    let workspace_canon = std::fs::canonicalize(&workspace_raw)
        .map_err(|e| format!("IO Error: Failed to resolve workspace: {}", e))?;

    let candidate = workspace_canon.join(&clean_rel);

    if for_write {
        let parent = candidate
            .parent()
            .ok_or_else(|| "Security Error: Invalid target parent".to_string())?;
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("IO Error: Failed to create parent directory: {}", e))?;
        let parent_canon = std::fs::canonicalize(parent)
            .map_err(|e| format!("IO Error: Failed to resolve target parent: {}", e))?;
        if !parent_canon.starts_with(&workspace_canon) {
            return Err("Security Error: Symlink escape blocked".to_string());
        }

        if let Ok(meta) = std::fs::symlink_metadata(&candidate) {
            if meta.file_type().is_symlink() {
                return Err("Security Error: Writing through symlinks is blocked".to_string());
            }
            let existing_canon = std::fs::canonicalize(&candidate)
                .map_err(|e| format!("IO Error: Failed to resolve target file: {}", e))?;
            if !existing_canon.starts_with(&workspace_canon) {
                return Err("Security Error: Symlink escape blocked".to_string());
            }
        }

        let file_name = candidate
            .file_name()
            .ok_or_else(|| "Security Error: Invalid file name".to_string())?;
        Ok(parent_canon.join(file_name))
    } else {
        let target_canon =
            std::fs::canonicalize(&candidate).map_err(|e| format!("IO Error: {}", e))?;
        if !target_canon.starts_with(&workspace_canon) {
            return Err("Security Error: Symlink escape blocked".to_string());
        }
        Ok(target_canon)
    }
}

fn eval_maker_blocking(
    tenant_id: String,
    code: String,
    params_val: Value,
) -> Result<String, SyscallError> {
    let build_context = |tid: &str, pval: &Value| -> Result<Context, SyscallError> {
        let mut context = Context::default();
        context
            .runtime_limits_mut()
            .set_loop_iteration_limit(MAX_JS_LOOP_ITERATIONS);
        context
            .runtime_limits_mut()
            .set_recursion_limit(MAX_JS_RECURSION_DEPTH);
        context
            .runtime_limits_mut()
            .set_stack_size_limit(MAX_JS_STACK_SIZE);

        let params_js = JsValue::from_json(pval, &mut context).map_err(|e| {
            SyscallError::InternalError(format!("Failed to convert params to JS: {}", e))
        })?;

        let _ = context.register_global_property(
            boa_engine::js_string!("params"),
            params_js,
            boa_engine::property::Attribute::all(),
        );

        let _ = context.register_global_property(
            boa_engine::js_string!("__TENANT_ID__"),
            boa_engine::js_string!(tid),
            boa_engine::property::Attribute::READONLY,
        );

        // read_file(path)
        let _ = context.register_global_builtin_callable(
            boa_engine::js_string!("read_file"),
            1,
            boa_engine::native_function::NativeFunction::from_copy_closure(|_this, args, ctx| {
                let path_str = args
                    .first()
                    .and_then(|v| v.as_string())
                    .map(|s| s.to_std_string().unwrap_or_default())
                    .unwrap_or_default();

                let tid_val = ctx
                    .global_object()
                    .get(boa_engine::js_string!("__TENANT_ID__"), ctx)
                    .map_err(|e| {
                        JsError::from_opaque(JsValue::from(boa_engine::js_string!(e.to_string())))
                    })?;

                let tid = tid_val
                    .as_string()
                    .map(|s| s.to_std_string().unwrap_or_default())
                    .unwrap_or_else(|| "default".to_string());

                let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
                let full_path =
                    match resolve_jailed_workspace_path(&base_dir, &tid, &path_str, false) {
                        Ok(p) => p,
                        Err(msg) => return Ok(JsValue::from(boa_engine::js_string!(msg))),
                    };

                match std::fs::metadata(&full_path) {
                    Ok(meta) if meta.len() as usize > MAX_MAKER_FILE_BYTES => {
                        return Ok(JsValue::from(boa_engine::js_string!(
                            "Security Error: File exceeds 2 MiB sandbox read limit"
                        )));
                    }
                    Err(e) => {
                        return Ok(JsValue::from(boa_engine::js_string!(format!(
                            "IO Error: {}",
                            e
                        ))));
                    }
                    _ => {}
                }

                match std::fs::read_to_string(full_path) {
                    Ok(content) => Ok(JsValue::from(boa_engine::js_string!(content))),
                    Err(e) => Ok(JsValue::from(boa_engine::js_string!(format!(
                        "IO Error: {}",
                        e
                    )))),
                }
            }),
        );

        // write_file(path, content)
        let _ = context.register_global_builtin_callable(
            boa_engine::js_string!("write_file"),
            2,
            boa_engine::native_function::NativeFunction::from_copy_closure(|_this, args, ctx| {
                let path_str = args
                    .first()
                    .and_then(|v| v.as_string())
                    .map(|s| s.to_std_string().unwrap_or_default())
                    .unwrap_or_default();
                let content = args
                    .get(1)
                    .and_then(|v| v.as_string())
                    .map(|s| s.to_std_string().unwrap_or_default())
                    .unwrap_or_default();

                if content.len() > MAX_MAKER_FILE_BYTES {
                    return Ok(JsValue::from(boa_engine::js_string!(
                        "Security Error: Content exceeds 2 MiB sandbox write limit"
                    )));
                }

                let tid_val = ctx
                    .global_object()
                    .get(boa_engine::js_string!("__TENANT_ID__"), ctx)
                    .map_err(|e| {
                        JsError::from_opaque(JsValue::from(boa_engine::js_string!(e.to_string())))
                    })?;

                let tid = tid_val
                    .as_string()
                    .map(|s| s.to_std_string().unwrap_or_default())
                    .unwrap_or_else(|| "default".to_string());

                let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
                let full_path =
                    match resolve_jailed_workspace_path(&base_dir, &tid, &path_str, true) {
                        Ok(p) => p,
                        Err(msg) => return Ok(JsValue::from(boa_engine::js_string!(msg))),
                    };

                match std::fs::write(full_path, content) {
                    Ok(_) => Ok(JsValue::from(boa_engine::js_string!("Success"))),
                    Err(e) => Ok(JsValue::from(boa_engine::js_string!(format!(
                        "IO Error: {}",
                        e
                    )))),
                }
            }),
        );

        // CORE-181: stub require() — lanza excepción JS para que el código del modelo
        // reciba un error claro e inmediato en lugar de continuar con un string inútil.
        let _ = context.register_global_builtin_callable(
            boa_engine::js_string!("require"),
            1,
            boa_engine::native_function::NativeFunction::from_copy_closure(|_this, args, _ctx| {
                let module = args
                    .first()
                    .and_then(|v| v.as_string())
                    .map(|s| s.to_std_string().unwrap_or_default())
                    .unwrap_or_else(|| "unknown".to_string());
                Err(JsError::from_opaque(JsValue::from(boa_engine::js_string!(
                    format!(
                        "require('{}') is not available in Aegis Maker sandbox. Use read_file(path) and write_file(path, content) instead.",
                        module
                    )
                ))))
            }),
        );

        Ok(context)
    };

    // Evaluar primero el código original (preservando `\n` dentro de strings JS válidos).
    // Si falla con SyntaxError y contiene secuencias literales `\n` fuera de comillas,
    // reintentamos des-escapando únicamente fuera de literales de string.
    let mut context = build_context(&tenant_id, &params_val)?;
    let raw_wrapped = format!("(function() {{\n{}\n}})();", code);

    match context.eval(Source::from_bytes(raw_wrapped.as_bytes())) {
        Ok(res) => {
            let js_str = res.to_string(&mut context).map_err(|e| {
                SyscallError::InternalError(format!("Result conversion failed: {}", e))
            })?;
            js_str
                .to_std_string()
                .map_err(|e| SyscallError::InternalError(format!("UTF-8 conversion failed: {}", e)))
        }
        Err(first_err) => {
            let err_text = first_err.to_string();
            if err_text.contains("SyntaxError") && code.contains("\\n") {
                let unescaped = unescape_js_outside_strings(&code);
                if unescaped != code {
                    let mut fallback_ctx = build_context(&tenant_id, &params_val)?;
                    let retry_wrapped = format!("(function() {{\n{}\n}})();", unescaped);
                    let res = fallback_ctx
                        .eval(Source::from_bytes(retry_wrapped.as_bytes()))
                        .map_err(|e| SyscallError::InternalError(format!("JS Error: {}", e)))?;
                    let js_str = res.to_string(&mut fallback_ctx).map_err(|e| {
                        SyscallError::InternalError(format!("Result conversion failed: {}", e))
                    })?;
                    return js_str.to_std_string().map_err(|e| {
                        SyscallError::InternalError(format!("UTF-8 conversion failed: {}", e))
                    });
                }
            }
            Err(SyscallError::InternalError(format!(
                "JS Error: {}",
                err_text
            )))
        }
    }
}

#[allow(
    clippy::get_first,
    clippy::to_string_in_format_args,
    clippy::useless_conversion,
    clippy::empty_line_after_outer_attr
)]
#[allow(clippy::new_without_default)]
/// --- MAKER EXECUTOR (CORE-150 / CORE-363) ---
/// Provee un entorno de ejecución aislado para scripts JavaScript (Boa Engine)
/// con cuotas de bucle/recursión, aislamiento de hilos y jailing canónico.
pub struct MakerExecutor;

impl Default for MakerExecutor {
    fn default() -> Self {
        Self
    }
}
impl MakerExecutor {
    pub fn new() -> Self {
        Self
    }

    /// Ejecuta un script en el sandbox sin bloquear los workers asíncronos de Tokio.
    pub async fn execute(
        &self,
        tenant_id: &str,
        script_type: &str,
        code: &str,
        params_json: &str,
    ) -> Result<String, SyscallError> {
        if script_type != "js" && script_type != "javascript" {
            return Err(SyscallError::InternalError(format!(
                "Unsupported script type: {}. Only 'js' is supported.",
                script_type
            )));
        }

        let params_val: Value = serde_json::from_str(params_json)
            .map_err(|e| SyscallError::InternalError(format!("Invalid params JSON: {}", e)))?;

        let tid = tenant_id.to_string();
        let src = code.to_string();

        let join_res = tokio::time::timeout(
            MAX_MAKER_TIMEOUT,
            tokio::task::spawn_blocking(move || eval_maker_blocking(tid, src, params_val)),
        )
        .await
        .map_err(|_| {
            SyscallError::InternalError("JS Error: Execution timed out (10s limit)".to_string())
        })?;

        join_res.map_err(|e| {
            SyscallError::InternalError(format!("JS sandbox worker panicked: {}", e))
        })?
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_maker_js_execution() {
        let _guard = crate::enclave::acquire_test_lock();
        let maker = MakerExecutor::new();
        let code = "return 1 + 1";
        let res = maker.execute("test", "js", code, "{}").await.unwrap();
        assert_eq!(res, "2");
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_maker_params_injection() {
        let _guard = crate::enclave::acquire_test_lock();
        let maker = MakerExecutor::new();
        let code = "return params.name + ' ' + params.age";
        let params = r#"{"name": "Aegis", "age": 1}"#;
        let res = maker.execute("test", "js", code, params).await.unwrap();
        assert_eq!(res, "Aegis 1");
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_maker_preserves_valid_string_escapes_and_handles_llm_double_escape() {
        let _guard = crate::enclave::acquire_test_lock();
        let maker = MakerExecutor::new();

        // Valid JS containing "\n" inside string literals must NOT fail with SyntaxError
        let valid_code = r#"const s = "line1\nline2\nline3"; return s.split("\n").length;"#;
        let res = maker.execute("test", "js", valid_code, "{}").await.unwrap();
        assert_eq!(res, "3");

        // LLM double-escaped outer newlines while keeping inner string "\n" intact
        let double_escaped = r#"const s = "a\nb";\nreturn s.split("\n").length;"#;
        let res2 = maker
            .execute("test", "js", double_escaped, "{}")
            .await
            .unwrap();
        assert_eq!(res2, "2");
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_maker_aborts_infinite_loop() {
        let _guard = crate::enclave::acquire_test_lock();
        let maker = MakerExecutor::new();
        let err = maker
            .execute("test", "js", "while (true) {}", "{}")
            .await
            .expect_err("Infinite loop must be aborted by RuntimeLimits");
        assert!(err.to_string().contains("JS Error"));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_maker_filesystem_jail() {
        let _guard = crate::enclave::acquire_test_lock();
        let dir = tempdir().unwrap();
        let tenant_id = "jail_test";
        // Simulamos la estructura de directorios que espera el executor
        let workspace = dir.path().join("users").join(tenant_id).join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        // En lugar de set_current_dir, usamos AEGIS_DATA_DIR
        let original_data_dir = std::env::var("AEGIS_DATA_DIR").ok();
        std::env::set_var("AEGIS_DATA_DIR", dir.path());

        let maker = MakerExecutor::new();

        // Escribir y Leer
        let code = "write_file('hello.txt', 'Sandboxed!'); return read_file('hello.txt')";
        let res = maker.execute(tenant_id, "js", code, "{}").await.unwrap();
        assert_eq!(res, "Sandboxed!");

        // Intentar escape con ..
        let code_escape = "return read_file('../../../secret.txt')";
        let res_escape = maker
            .execute(tenant_id, "js", code_escape, "{}")
            .await
            .unwrap();
        assert!(res_escape.contains("Security Error"));

        // Intentar escape con prefijo de unidad Windows (C:/...)
        let drive_escape = "return read_file('C:/Windows/win.ini')";
        let res_drive = maker
            .execute(tenant_id, "js", drive_escape, "{}")
            .await
            .unwrap();
        assert!(res_drive.contains("Security Error"));

        if let Some(old) = original_data_dir {
            std::env::set_var("AEGIS_DATA_DIR", old);
        } else {
            std::env::remove_var("AEGIS_DATA_DIR");
        }
    }
}

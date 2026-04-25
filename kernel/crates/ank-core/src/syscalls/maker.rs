use crate::syscalls::SyscallError;
use boa_engine::{Context, JsError, JsValue, Source};
use serde_json::Value;
use std::path::Path;

#[allow(
    clippy::get_first,
    clippy::to_string_in_format_args,
    clippy::useless_conversion,
    clippy::empty_line_after_outer_attr
)]
#[allow(clippy::new_without_default)]
/// --- MAKER EXECUTOR (CORE-150) ---
/// Provee un entorno de ejecución aislado para scripts JavaScript (Boa Engine).
/// Permite automatizar tareas complejas dentro del workspace del tenant.
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

    /// Ejecuta un script en el sandbox.
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

        let mut context = Context::default();

        // 1. Inyectar parámetros
        let params_val: Value = serde_json::from_str(params_json)
            .map_err(|e| SyscallError::InternalError(format!("Invalid params JSON: {}", e)))?;

        let params_js = JsValue::from_json(&params_val, &mut context).map_err(|e| {
            SyscallError::InternalError(format!("Failed to convert params to JS: {}", e))
        })?;

        let _ = context.register_global_property(
            boa_engine::js_string!("params"),
            params_js,
            boa_engine::property::Attribute::all(),
        );

        let _ = context.register_global_property(
            boa_engine::js_string!("__TENANT_ID__"),
            boa_engine::js_string!(tenant_id),
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

                if path_str.contains("..")
                    || path_str.starts_with("/")
                    || path_str.starts_with("\\")
                {
                    return Ok(JsValue::from(boa_engine::js_string!(
                        "Security Error: Path traversal blocked"
                    )));
                }

                let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
                let full_path = Path::new(&base_dir)
                    .join("users")
                    .join(&tid)
                    .join("workspace")
                    .join(&path_str);
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

                if path_str.contains("..")
                    || path_str.starts_with("/")
                    || path_str.starts_with("\\")
                {
                    return Ok(JsValue::from(boa_engine::js_string!(
                        "Security Error: Path traversal blocked"
                    )));
                }

                let base_dir = std::env::var("AEGIS_DATA_DIR").unwrap_or_else(|_| ".".to_string());
                let full_path = Path::new(&base_dir)
                    .join("users")
                    .join(&tid)
                    .join("workspace")
                    .join(&path_str);
                if let Some(parent) = full_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }

                match std::fs::write(full_path, content) {
                    Ok(_) => Ok(JsValue::from(boa_engine::js_string!("Success"))),
                    Err(e) => Ok(JsValue::from(boa_engine::js_string!(format!(
                        "IO Error: {}",
                        e
                    )))),
                }
            }),
        );

        // CORE-181: stub require() — retorna error descriptivo en lugar de ReferenceError
        let _ = context.register_global_builtin_callable(
            boa_engine::js_string!("require"),
            1,
            boa_engine::native_function::NativeFunction::from_copy_closure(|_this, args, _ctx| {
                let module = args
                    .first()
                    .and_then(|v| v.as_string())
                    .map(|s| s.to_std_string().unwrap_or_default())
                    .unwrap_or_else(|| "unknown".to_string());
                Ok(JsValue::from(boa_engine::js_string!(format!(
                    "Error: require('{}') is not available in Aegis Maker sandbox. Use read_file() and write_file() instead.",
                    module
                ))))
            }),
        );

        // CORE-181: wrap en IIFE para que top-level `return` sea JS válido
        let wrapped = format!("(function() {{\n{}\n}})();", code);

        // 3. Ejecutar
        match context.eval(Source::from_bytes(wrapped.as_bytes())) {
            Ok(res) => {
                let js_str = res.to_string(&mut context).map_err(|e| {
                    SyscallError::InternalError(format!("Result conversion failed: {}", e))
                })?;
                let out = js_str.to_std_string().map_err(|e| {
                    SyscallError::InternalError(format!("UTF-8 conversion failed: {}", e))
                })?;
                Ok(out)
            }
            Err(e) => Err(SyscallError::InternalError(format!("JS Error: {}", e))),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_maker_js_execution() {
        let maker = MakerExecutor::new();
        let code = "return 1 + 1";
        let res = maker.execute("test", "js", code, "{}").await.unwrap();
        assert_eq!(res, "2");
    }

    #[tokio::test]
    async fn test_maker_params_injection() {
        let maker = MakerExecutor::new();
        let code = "return params.name + ' ' + params.age";
        let params = r#"{"name": "Aegis", "age": 1}"#;
        let res = maker.execute("test", "js", code, params).await.unwrap();
        assert_eq!(res, "Aegis 1");
    }

    #[tokio::test]
    async fn test_maker_filesystem_jail() {
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

        // Intentar escape
        let code_escape = "return read_file('../../../secret.txt')";
        let res_escape = maker
            .execute(tenant_id, "js", code_escape, "{}")
            .await
            .unwrap();
        assert!(res_escape.contains("Security Error"));

        if let Some(old) = original_data_dir {
            std::env::set_var("AEGIS_DATA_DIR", old);
        } else {
            std::env::remove_var("AEGIS_DATA_DIR");
        }
    }
}


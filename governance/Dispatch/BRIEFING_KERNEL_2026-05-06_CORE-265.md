# BRIEFING — Kernel Engineer
## CORE-265: ank-server debe leer aegis.env directamente

**Fecha:** 2026-05-06
**Prioridad:** CRITICAL — bloquea instalación en Windows para todos los usuarios
**Rama sugerida:** `fix/core-265-load-env-file`

---

## Contexto

`ank-server` en Windows falla al arrancar como servicio con:

```
FATAL: AEGIS_ROOT_KEY environment variable is missing.
```

El SCM (Service Control Manager) de Windows no garantiza herencia de variables
de entorno escritas en el registro del servicio en el mismo boot sin reinicio.
El installer tiene un workaround parcial, pero la solución correcta es que el
**binario lea el archivo `aegis.env` directamente** antes de buscar vars en el entorno.

Esto es un fix de dos archivos. Nada más.

---

## Cambio 1 — `Cargo.toml` (workspace)

Agregar `dotenvy` a las workspace dependencies:

```toml
dotenvy = "0.15"
```

---

## Cambio 2 — `kernel/crates/ank-server/Cargo.toml`

Agregar la dependencia:

```toml
dotenvy.workspace = true
```

---

## Cambio 3 — `kernel/crates/ank-server/src/main.rs`

### 3a. Agregar la función `load_env_file()` antes de `main()`

```rust
fn load_env_file() {
    let mut candidates: Vec<std::path::PathBuf> = vec![];

    // 1. Override explícito via variable de entorno
    if let Ok(explicit) = std::env::var("AEGIS_ENV_FILE") {
        candidates.push(std::path::PathBuf::from(explicit));
    }

    // 2. Path por defecto según plataforma
    #[cfg(windows)]
    candidates.push(std::path::PathBuf::from(r"C:\ProgramData\Aegis\aegis.env"));

    #[cfg(not(windows))]
    candidates.push(std::path::PathBuf::from("/etc/aegis/aegis.env"));

    // 3. .env en el directorio de trabajo actual (dev convenience)
    candidates.push(std::path::PathBuf::from(".env"));

    for path in &candidates {
        if path.exists() {
            match dotenvy::from_path_override(path) {
                Ok(_) => return, // tracing no está inicializado aún, no logueamos
                Err(e) => eprintln!("[WARN] Failed to load env file {:?}: {}", path, e),
            }
        }
    }
    // Si no se encontró ningún env file, continuar normalmente.
    // Linux/Docker: las vars vienen del entorno del proceso vía systemd EnvironmentFile=
}
```

### 3b. Llamar `load_env_file()` al inicio de `main()`, ANTES del paso 1 (tracing)

La función `main()` actual empieza así (línea ~40):

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--version".to_string()) || args.contains(&"-v".to_string()) {
        println!("Aegis Core v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // 1. Inicializar tracing
    let data_dir = resolve_data_dir();
```

Insertar `load_env_file()` justo antes del paso 1, quedando así:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--version".to_string()) || args.contains(&"-v".to_string()) {
        println!("Aegis Core v{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // 0b. Cargar archivo de entorno antes de leer cualquier variable
    // Permite que ank-server funcione como servicio de Windows sin inyección
    // de env vars via SCM. En Linux/Docker no tiene efecto si el archivo no existe.
    load_env_file();

    // 1. Inicializar tracing
    let data_dir = resolve_data_dir();
```

---

## Verification

```bash
cargo build --release -p ank-server
```

Sin errores. No hay tests nuevos requeridos — los criterios de aceptación
se verifican manualmente en Windows ejecutando el binario sin vars de entorno
seteadas pero con `C:\ProgramData\Aegis\aegis.env` presente.

---

## Notas

- Usar `from_path_override` (no `from_path`) para que el archivo `.env` siempre
  gane sobre variables ya presentes en el entorno del proceso. Esto es correcto
  para un servicio donde el archivo es la fuente de verdad.
- En Linux con systemd, `/etc/aegis/aegis.env` existe pero las vars ya están
  en el entorno via `EnvironmentFile=` en el unit. `from_path_override` las
  sobreescribirá con los mismos valores — sin efecto neto.
- **No modificar `install.ps1`** — ese archivo ya fue actualizado por el Arquitecto.
  El paso 3 del ticket CORE-265 (simplificar el installer) queda para una iteración
  posterior una vez que este fix esté en producción y verificado.

---

## Commit esperado

```
fix(ank-server): CORE-265 load aegis.env on startup for Windows service compatibility
```

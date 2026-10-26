#[allow(unused_imports)]
use anyhow::{anyhow, Context, Result};

#[cfg(windows)]
use std::ffi::OsString;
#[cfg(windows)]
use windows_service::{
    define_windows_service,
    service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_manager::{ServiceManager, ServiceManagerAccess},
};

#[cfg(windows)]
const SERVICE_NAME: &str = "AegisSupervisor";
#[cfg(windows)]
const SERVICE_DISPLAY_NAME: &str = "Aegis OS — Cognitive Neural Kernel";

#[cfg(windows)]
pub fn install_service() -> Result<()> {
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .map_err(|e| {
        anyhow!(
            "Failed to connect to Service Manager: {}. Try running as Administrator.",
            e
        )
    })?;

    let executable_path = std::env::current_exe()?;

    let service_info = ServiceInfo {
        name: SERVICE_NAME.into(),
        display_name: SERVICE_DISPLAY_NAME.into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path,
        launch_arguments: vec![OsString::from("--service")],
        dependencies: Vec::new(),
        account_name: None,
        account_password: None,
    };

    let access = ServiceAccess::QUERY_STATUS
        | ServiceAccess::START
        | ServiceAccess::STOP
        | ServiceAccess::DELETE;

    let _service = manager
        .create_service(&service_info, access)
        .context("Failed to create Windows service.")?;

    println!("Successfully registered Aegis OS as a Windows Service.");
    Ok(())
}

#[cfg(windows)]
pub fn uninstall_service() -> Result<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .map_err(|e| {
            anyhow!(
                "Failed to connect to Service Manager: {}. Access Denied?",
                e
            )
        })?;

    let service = manager.open_service(
        SERVICE_NAME,
        ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE,
    )?;
    service.delete()?;

    println!("Successfully unregistered Aegis OS Windows Service.");
    Ok(())
}

#[cfg(windows)]
define_windows_service!(ffi_service_main, service_main);

#[cfg(windows)]
pub fn run_as_service() -> Result<()> {
    use windows_service::service_dispatcher;

    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
        .context("Failed to start service dispatcher")
}

#[cfg(windows)]
fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = service_run() {
        tracing::error!("AegisSupervisor Windows service failed: {}", e);
    }
}

#[cfg(windows)]
fn service_run() -> Result<()> {
    use crate::supervisor::AegisSupervisor;
    use std::time::Duration;
    use tokio::runtime::Runtime;
    use tokio::sync::watch;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = shutdown_tx.send(true);
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(15),
        process_id: None,
    })?;

    let rt = Runtime::new()?;
    let run_result: Result<()> = rt.block_on(async {
        let mut config = crate::load_config()?;
        config.dev_mode = false;
        let mut supervisor = AegisSupervisor::new(config);
        supervisor.start().await?;

        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::from_secs(0),
            process_id: None,
        })?;

        supervisor.watch_until_shutdown(shutdown_rx).await;
        Ok(())
    });

    let exit_code = if run_result.is_ok() {
        ServiceExitCode::Win32(0)
    } else {
        ServiceExitCode::ServiceSpecific(1)
    };

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code,
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    run_result
}

#[cfg(not(windows))]
pub fn install_service() -> Result<()> {
    Err(anyhow!(
        "Service installation is only supported on Windows."
    ))
}

#[cfg(not(windows))]
pub fn uninstall_service() -> Result<()> {
    Err(anyhow!(
        "Service uninstallation is only supported on Windows."
    ))
}

#[cfg(not(windows))]
pub fn run_as_service() -> Result<()> {
    Err(anyhow!("Service execution is only supported on Windows."))
}

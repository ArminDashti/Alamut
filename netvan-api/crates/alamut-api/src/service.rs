use anyhow::{bail, Result};
use std::ffi::OsString;
use std::process::Command;
use tracing::info;

pub const SERVICE_NAME: &str = "Alamut";
pub const SERVICE_DISPLAY: &str = "Alamut";
pub const DEFAULT_URL: &str = "http://alamut.local";

pub fn install() -> Result<()> {
    if service_exists() {
        println!("Service '{SERVICE_NAME}' is already installed, nothing to do");
        return Ok(());
    }
    let exe = std::env::current_exe()?;
    let bin = exe.display().to_string();
    // NOTE: sc.exe requires the option name (including `=`) and its value
    // to be separate argv tokens, e.g. `start=` + `auto`, not `start= auto`.
    let status = Command::new("sc")
        .args([
            "create",
            SERVICE_NAME,
            "binPath=",
            &format!("\"{bin}\" run"),
            "start=",
            "auto",
            "DisplayName=",
            SERVICE_DISPLAY,
        ])
        .status()?;
    if !status.success() {
        bail!("sc create failed (may need elevation)");
    }
    let _ = Command::new("sc")
        .args([
            "description",
            SERVICE_NAME,
            "Local Alamut collectors + Web UI + HTTP/WebSocket API (http://alamut.local)",
        ])
        .status();
    let _ = Command::new("sc")
        .args([
            "failure",
            SERVICE_NAME,
            "reset=",
            "86400",
            "actions=",
            "restart/5000/restart/5000/restart/5000",
        ])
        .status();
    info!("installed {SERVICE_NAME}");
    println!("Installed Windows service '{SERVICE_NAME}' ({SERVICE_DISPLAY})");
    println!("Listening URL when running: {DEFAULT_URL}");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    if !service_exists() {
        println!("Service '{SERVICE_NAME}' is not installed, nothing to do");
        return Ok(());
    }
    let _ = stop();
    let output = Command::new("sc")
        .args(["delete", SERVICE_NAME])
        .output()?;
    if !output.status.success() {
        // Service may have been removed concurrently; treat "does not exist" as success.
        if !service_exists() || is_not_installed_output(&output) {
            println!("Service '{SERVICE_NAME}' is not installed, nothing to do");
            return Ok(());
        }
        bail!("sc delete failed (may need elevation)");
    }
    info!("uninstalled {SERVICE_NAME}");
    println!("Uninstalled Windows service '{SERVICE_NAME}'");
    Ok(())
}

pub fn start() -> Result<()> {
    let output = Command::new("sc").args(["start", SERVICE_NAME]).status()?;
    if !output.success() {
        bail!("sc start failed (may need elevation)");
    }
    println!("Started '{SERVICE_NAME}' → {DEFAULT_URL}");
    Ok(())
}

pub fn stop() -> Result<()> {
    if !service_exists() {
        println!("Service '{SERVICE_NAME}' is not installed, nothing to stop");
        return Ok(());
    }
    let status = Command::new("sc").args(["stop", SERVICE_NAME]).status()?;
    if !status.success() {
        // ignore if already stopped
    }
    println!("Stopped '{SERVICE_NAME}'");
    Ok(())
}

fn service_exists() -> bool {
    Command::new("sc")
        .args(["query", SERVICE_NAME])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn is_not_installed_output(output: &std::process::Output) -> bool {
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let upper = combined.to_uppercase();
    upper.contains("1060") || upper.contains("DOES NOT EXIST")
}

pub fn status() -> Result<()> {
    let output = Command::new("sc").args(["query", SERVICE_NAME]).output()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let err = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        println!("Service: {SERVICE_NAME}");
        println!("Installed: no");
        println!("State: not installed");
        println!("URL: {DEFAULT_URL} (when running)");
        if !err.trim().is_empty() {
            println!("Detail: {}", err.trim());
        }
        return Ok(());
    }

    let state = text
        .lines()
        .find_map(|line| {
            let t = line.trim();
            if t.starts_with("STATE") {
                // e.g. STATE              : 4  RUNNING
                Some(
                    t.split(':')
                        .nth(1)
                        .unwrap_or("")
                        .split_whitespace()
                        .last()
                        .unwrap_or("unknown")
                        .to_string(),
                )
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".into());

    println!("Service: {SERVICE_NAME}");
    println!("Display: {SERVICE_DISPLAY}");
    println!("Installed: yes");
    println!("State: {state}");
    println!("URL: {DEFAULT_URL}");
    Ok(())
}

#[cfg(windows)]
pub fn run_as_service() -> Result<()> {
    use std::time::Duration;
    use tokio::sync::oneshot;
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::{define_windows_service, service_dispatcher};

    define_windows_service!(ffi_service_main, service_main);

    fn service_main(_args: Vec<OsString>) {
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let shutdown_tx = std::sync::Mutex::new(Some(shutdown_tx));

        let event_handler = move |control| match control {
            ServiceControl::Stop => {
                if let Ok(mut guard) = shutdown_tx.lock() {
                    if let Some(tx) = guard.take() {
                        let _ = tx.send(());
                    }
                }
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        };

        let status_handle = match service_control_handler::register(SERVICE_NAME, event_handler) {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("failed to register service control handler: {e}");
                return;
            }
        };

        let pending = ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::StartPending,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 1,
            wait_hint: Duration::from_secs(30),
            process_id: None,
        };
        let _ = status_handle.set_service_status(pending);

        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("failed to create tokio runtime: {e}");
                let _ = status_handle.set_service_status(ServiceStatus {
                    service_type: ServiceType::OWN_PROCESS,
                    current_state: ServiceState::Stopped,
                    controls_accepted: ServiceControlAccept::empty(),
                    exit_code: ServiceExitCode::ServiceSpecific(1),
                    checkpoint: 0,
                    wait_hint: Duration::default(),
                    process_id: None,
                });
                return;
            }
        };

        let running = ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        };
        let _ = status_handle.set_service_status(running);

        let shutdown = async {
            let _ = shutdown_rx.await;
        };
        let result = rt.block_on(crate::http_server::run(shutdown));

        if let Err(e) = result {
            tracing::error!("http server exited with error: {e}");
        }

        let stopping = ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::StopPending,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 1,
            wait_hint: Duration::from_secs(10),
            process_id: None,
        };
        let _ = status_handle.set_service_status(stopping);

        let stopped = ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        };
        let _ = status_handle.set_service_status(stopped);
    }

    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

#[cfg(not(windows))]
pub fn run_as_service() -> Result<()> {
    bail!("Windows service mode is only supported on Windows");
}

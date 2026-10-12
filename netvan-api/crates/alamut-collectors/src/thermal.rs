//! Live temperature sensors via the LibreHardwareMonitor helper (`alamut-hwmon`).

use alamut_core::types::ThermalSnapshot;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::{Duration, Instant};
use tracing::warn;

const CACHE_TTL: Duration = Duration::from_millis(1500);

struct Helper {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

struct State {
    helper: Option<Helper>,
    snapshot: ThermalSnapshot,
    at: Option<Instant>,
    spawn_failed_at: Option<Instant>,
}

/// How long to wait before retrying a failed `alamut-hwmon` spawn.
/// Without this the service would never pick up the helper if it was
/// deployed after the service started (empty sensors forever).
const SPAWN_RETRY: Duration = Duration::from_secs(30);

static STATE: Lazy<Mutex<State>> = Lazy::new(|| {
    Mutex::new(State {
        helper: None,
        snapshot: ThermalSnapshot {
            sensors: Vec::new(),
        },
        at: None,
        spawn_failed_at: None,
    })
});

pub fn snapshot() -> ThermalSnapshot {
    #[cfg(not(windows))]
    {
        ThermalSnapshot {
            sensors: Vec::new(),
        }
    }
    #[cfg(windows)]
    {
        snapshot_windows()
    }
}

#[cfg(windows)]
fn snapshot_windows() -> ThermalSnapshot {
    let mut st = STATE.lock();
    if let Some(at) = st.at {
        if at.elapsed() < CACHE_TTL {
            return st.snapshot.clone();
        }
    }

    return match poll_helper(&mut st) {
        Ok(mut snap) => {
            if snap.sensors.is_empty() {
                match native_wmi_snapshot() {
                    Ok(native) => snap = native,
                    Err(e) => warn!("native Windows thermal query failed: {e:#}"),
                }
            }
            st.snapshot = snap.clone();
            st.at = Some(Instant::now());
            snap
        }

        Err(e) => {
            warn!("thermal helper: {e:#}");
            drop_helper(&mut st);
            st.snapshot = ThermalSnapshot {
                sensors: Vec::new(),
            };
            st.at = Some(Instant::now());
            st.snapshot.clone()
        }
    };

    #[cfg(windows)]
    fn native_wmi_snapshot() -> anyhow::Result<ThermalSnapshot> {
        use anyhow::Context;
        use serde::Deserialize;
        use wmi::{COMLibrary, WMIConnection};

        #[derive(Deserialize)]
        struct ThermalZone {
            #[serde(rename = "CurrentTemperature")]
            current_temperature: Option<u32>,
            #[serde(rename = "InstanceName")]
            instance_name: Option<String>,
        }

        let com = COMLibrary::new().context("initialize WMI COM")?;
        let wmi = WMIConnection::new(com).context("connect to WMI")?;
        let rows: Vec<ThermalZone> = wmi.raw_query(
            "SELECT CurrentTemperature, InstanceName FROM MSAcpi_ThermalZoneTemperature",
        )?;

        let sensors = rows
            .into_iter()
            .filter_map(|row| {
                let raw = row.current_temperature?;
                if raw == 0 {
                    return None;
                }
                let celsius = raw as f64 / 10.0 - 273.15;
                if !(-50.0..=150.0).contains(&celsius) {
                    return None;
                }
                let name = row
                    .instance_name
                    .as_deref()
                    .and_then(|v| v.rsplit('\\').next())
                    .filter(|v| !v.is_empty())
                    .unwrap_or("ACPI thermal zone")
                    .to_string();
                Some(alamut_core::types::ThermalSensor {
                    id: format!("wmi:{name}"),
                    hardware_kind: alamut_core::types::HardwareKind::Motherboard,
                    hardware_name: "Windows ACPI".into(),
                    sensor_name: name,
                    celsius: Some(celsius),
                })
            })
            .collect();

        Ok(ThermalSnapshot { sensors })
    }
}

#[cfg(windows)]
fn poll_helper(st: &mut State) -> anyhow::Result<ThermalSnapshot> {
    if st.helper.is_none() {
        if let Some(t) = st.spawn_failed_at {
            if t.elapsed() < SPAWN_RETRY {
                return Ok(ThermalSnapshot {
                    sensors: Vec::new(),
                });
            }
        }
        match spawn_helper() {
            Ok(h) => {
                st.helper = Some(h);
                st.spawn_failed_at = None;
            }
            Err(e) => {
                st.spawn_failed_at = Some(Instant::now());
                warn!("alamut-hwmon spawn failed: {e:#}");
                return Ok(ThermalSnapshot {
                    sensors: Vec::new(),
                });
            }
        }
    }

    let helper = st.helper.as_mut().expect("helper set");
    helper.stdin.write_all(b"snapshot\n")?;
    helper.stdin.flush()?;

    let mut line = String::new();
    helper.stdout.read_line(&mut line)?;
    if line.is_empty() {
        anyhow::bail!("helper closed stdout");
    }

    let snap: ThermalSnapshot = serde_json::from_str(line.trim())?;
    Ok(snap)
}

#[cfg(windows)]
fn drop_helper(st: &mut State) {
    if let Some(mut h) = st.helper.take() {
        let _ = h.stdin.write_all(b"quit\n");
        let _ = h.child.kill();
        let _ = h.child.wait();
    }
    st.spawn_failed_at = None;
}

#[cfg(windows)]
fn spawn_helper() -> anyhow::Result<Helper> {
    let path = helper_path().ok_or_else(|| anyhow::anyhow!("alamut-hwmon.exe not found next to alamut-api"))?;
    let mut cmd = Command::new(&path);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    crate::win_cmd::hide_console(&mut cmd);
    let mut child = cmd.spawn()?;
    let stdin = child.stdin.take().ok_or_else(|| anyhow::anyhow!("no stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("no stdout"))?;
    Ok(Helper {
        child,
        stdin,
        stdout: BufReader::new(stdout),
    })
}

#[cfg(windows)]
fn helper_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("ALAMUT_HWMON_PATH") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [
        dir.join("alamut-hwmon.exe"),
        dir.join("alamut-hwmon").join("alamut-hwmon.exe"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

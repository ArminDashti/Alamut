//! CPU, memory, and disk snapshots via sysinfo.

use alamut_core::history::TimeRange;
use alamut_core::types::{
    CpuSnapshot, DiskKind, DiskSnapshot, MemorySnapshot, PowerEvent, PowerEventKind,
    PowerEventsReport, ProcessCpuUsage,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Local, TimeZone, Utc};
use parking_lot::Mutex;
use serde::Deserialize;
use std::process::Command;
use sysinfo::{CpuRefreshKind, DiskKind as SysDiskKind, Disks, MemoryRefreshKind, RefreshKind, System};

use crate::win_cmd::hide_console;

pub struct SystemMetricsCollector {
    system: Mutex<System>,
    disks: Mutex<Disks>,
    warmed_up: Mutex<bool>,
}

impl SystemMetricsCollector {
    pub fn new() -> Self {
        let refresh = RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything());
        let mut system = System::new_with_specifics(refresh);
        // Warm-up so the first cpu_usage is not a false zero.
        system.refresh_cpu_specifics(CpuRefreshKind::everything());
        system.refresh_memory_specifics(MemoryRefreshKind::everything());
        let disks = Disks::new_with_refreshed_list();
        Self {
            system: Mutex::new(system),
            disks: Mutex::new(disks),
            warmed_up: Mutex::new(false),
        }
    }

    pub fn refresh_live(&self) {
        let mut system = self.system.lock();
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        system.refresh_cpu_specifics(CpuRefreshKind::everything());
        system.refresh_memory_specifics(MemoryRefreshKind::everything());
        let mut disks = self.disks.lock();
        disks.refresh(true);
        let mut warmed = self.warmed_up.lock();
        if !*warmed {
            // Second refresh after a short delay is handled by the next tick;
            // mark warmed so we still expose data after the first real interval.
            *warmed = true;
        }
    }

    pub fn cpu_snapshot(&self) -> CpuSnapshot {
        let system = self.system.lock();
        let cpus = system.cpus();
        let first = cpus.first();
        let per_core: Vec<f64> = cpus.iter().map(|c| c.cpu_usage() as f64).collect();
        CpuSnapshot {
            brand: first.map(|c| c.brand().to_string()).unwrap_or_default(),
            vendor_id: first.map(|c| c.vendor_id().to_string()).unwrap_or_default(),
            physical_cores: system.physical_core_count().map(|n| n as u32),
            logical_cores: cpus.len() as u32,
            frequency_mhz: first.map(|c| c.frequency()).filter(|&f| f > 0),
            // Overall = mean of all per-core usages.
            utilization: if per_core.is_empty() {
                0.0
            } else {
                per_core.iter().sum::<f64>() / per_core.len() as f64
            },
            per_core,
            // Task-Manager-style: aggregate CPU by app name, high to low, top 20.
            top_processes: {
                let mut by_name: std::collections::HashMap<String, f32> =
                    std::collections::HashMap::new();
                for (_, p) in system.processes() {
                    *by_name
                        .entry(p.name().to_string_lossy().to_string())
                        .or_default() += p.cpu_usage();
                }
                let mut rows: Vec<ProcessCpuUsage> = by_name
                    .into_iter()
                    .map(|(process_name, cpu)| ProcessCpuUsage {
                        process_name,
                        cpu_percent: cpu as f64,
                    })
                    .collect();
                rows.sort_by(|a, b| {
                    b.cpu_percent
                        .partial_cmp(&a.cpu_percent)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                rows.truncate(20);
                rows
            },
        }
    }

    pub fn memory_snapshot(&self) -> MemorySnapshot {
        let system = self.system.lock();
        let total = system.total_memory();
        let used = system.used_memory();
        let available = system.available_memory();
        let utilization = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        MemorySnapshot {
            total_bytes: total,
            used_bytes: used,
            available_bytes: available,
            utilization,
        }
    }

    pub fn disk_snapshots(&self, kind_filter: Option<DiskKind>) -> Vec<DiskSnapshot> {
        let disks = self.disks.lock();
        let mut out = Vec::new();
        for disk in disks.list() {
            if disk.is_removable() {
                continue;
            }
            let kind = match disk.kind() {
                SysDiskKind::SSD => DiskKind::Ssd,
                SysDiskKind::HDD => DiskKind::Hdd,
                _ => continue, // skip unknown / unclassified
            };
            if let Some(filter) = kind_filter {
                if kind != filter {
                    continue;
                }
            }
            let total = disk.total_space();
            if total == 0 {
                continue;
            }
            let available = disk.available_space();
            let used = total.saturating_sub(available);
            let utilization = (used as f64 / total as f64) * 100.0;
            let mount = disk.mount_point().to_string_lossy().to_string();
            let name = disk.name().to_string_lossy().to_string();
            let id = if mount.is_empty() {
                name.clone()
            } else {
                mount.clone()
            };
            out.push(DiskSnapshot {
                id,
                name: if name.is_empty() {
                    mount.clone()
                } else {
                    name
                },
                mount_point: mount,
                file_system: disk.file_system().to_string_lossy().to_string(),
                kind,
                total_bytes: total,
                used_bytes: used,
                available_bytes: available,
                utilization,
            });
        }
        out.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
        out
    }
}

impl Default for SystemMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct PsPowerEvent {
    #[serde(rename = "TimeCreated")]
    time_created: String,
    #[serde(rename = "Id")]
    id: u32,
    #[serde(rename = "ProviderName")]
    provider_name: String,
    #[serde(rename = "Message")]
    message: String,
}

fn classify_power_event(id: u32, message: &str) -> PowerEventKind {
    match id {
        41 | 6008 => PowerEventKind::Crash,
        6006 => PowerEventKind::Shutdown,
        6005 | 12 => PowerEventKind::Boot,
        1074 => {
            let m = message.to_lowercase();
            if m.contains("restart") || m.contains("reboot") {
                PowerEventKind::Restart
            } else {
                PowerEventKind::Shutdown
            }
        }
        _ => PowerEventKind::Shutdown,
    }
}

fn parse_ps_time(raw: &str) -> Option<i64> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Some(dt.timestamp());
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(raw, "%m/%d/%Y %H:%M:%S %p") {
        return Local
            .from_local_datetime(&dt)
            .single()
            .map(|d| d.timestamp());
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S") {
        return Local
            .from_local_datetime(&dt)
            .single()
            .map(|d| d.timestamp());
    }
    None
}

fn power_event_message_line(s: &str) -> String {
    s.lines()
        .next()
        .unwrap_or(s)
        .trim()
        .chars()
        .take(240)
        .collect()
}

pub fn collect_power_events(tr: &TimeRange) -> Result<PowerEventsReport> {
    Ok(PowerEventsReport {
        uptime_secs: System::uptime(),
        events: query_power_events(tr)?,
    })
}

#[cfg(windows)]
fn query_power_events(tr: &TimeRange) -> Result<Vec<PowerEvent>> {
    let start_ts = tr.start_ts.unwrap_or_else(|| Utc::now().timestamp() - 30 * 86_400);
    let end_ts = tr.end_ts.unwrap_or_else(|| Utc::now().timestamp());

    let ps = format!(
        "$s=[DateTimeOffset]::FromUnixTimeSeconds({start_ts}).LocalDateTime; \
         $e=Get-WinEvent -FilterHashtable @{{LogName='System';Id=41,12,6005,6006,6008,1074;StartTime=$s}} \
         -MaxEvents 500 -ErrorAction SilentlyContinue; \
         if ($null -eq $e) {{ '[]' }} else {{ $e | Select-Object TimeCreated,Id,ProviderName,Message | ConvertTo-Json -Compress }}"
    );

    let output = hide_console(
        Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &ps]),
    )
    .output()
    .context("spawn powershell for power events")?;

    if !output.status.success() && output.stdout.is_empty() {
        let err = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("power events query failed: {err}");
    }

    let stdout_raw = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout_raw.trim();
    if stdout.is_empty() || stdout == "[]" {
        return Ok(Vec::new());
    }

    let parsed: Vec<PsPowerEvent> = if stdout.starts_with('[') {
        serde_json::from_str(stdout).context("parse power events json array")?
    } else {
        vec![serde_json::from_str(stdout).context("parse power events json object")?]
    };

    let mut events: Vec<PowerEvent> = Vec::new();
    for ev in parsed {
        let ts = parse_ps_time(&ev.time_created).unwrap_or(0);
        if ts < start_ts || ts > end_ts {
            continue;
        }
        events.push(PowerEvent {
            ts,
            kind: classify_power_event(ev.id, &ev.message),
            source: ev.provider_name,
            event_id: ev.id,
            message: power_event_message_line(&ev.message),
        });
    }

    events.sort_by(|a, b| b.ts.cmp(&a.ts));
    Ok(events)
}

#[cfg(not(windows))]
fn query_power_events(_tr: &TimeRange) -> Result<Vec<PowerEvent>> {
    Ok(Vec::new())
}

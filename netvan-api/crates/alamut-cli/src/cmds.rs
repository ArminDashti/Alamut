//! Headless one-shot + live CLI commands (work without the GUI / TUI).
//!
//! Primary source is the running Alamut API (`POST /api/rpc`). For
//! `cpu` / `memory` / `disk` / `os` we fall back to local `sysinfo`
//! collection so the CLI stays useful when the service is stopped.
//!
//! Style: modern geek-lovely — 24-bit color, nerd-glyph icons, block
//! meters, dim table chrome. Honors `NO_COLOR` / `TERM=dumb` / `--plain`.

use anyhow::Result;
use alamut_core::ipc::{RpcRequest, RpcResponse};
use alamut_core::types::DiskKind;
use serde_json::json;

use crate::api::ApiClient;

pub fn resolve_base(explicit: Option<String>) -> String {
    if let Some(b) = explicit {
        if !b.trim().is_empty() {
            return b;
        }
    }
    if let Ok(v) = std::env::var("ALAMUT_API_URL") {
        if !v.trim().is_empty() {
            return v;
        }
    }
    if let Ok(v) = std::env::var("ALAMUT_API_BASE") {
        if !v.trim().is_empty() {
            return v;
        }
    }
    match std::env::var("ALAMUT_API_BIND") {
        Ok(bind) => format!("http://{bind}"),
        Err(_) => "http://127.0.0.1:28437".to_string(),
    }
}

// ------------------------------------------------------------------- style

static PLAIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_plain(plain: bool) {
    PLAIN.store(plain, std::sync::atomic::Ordering::Relaxed);
}

fn color_on() -> bool {
    if PLAIN.load(std::sync::atomic::Ordering::Relaxed) {
        return false;
    }
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if matches!(std::env::var("TERM").as_deref(), Ok("dumb")) {
        return false;
    }
    true
}

fn paint(code: &str, s: &str) -> String {
    if color_on() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn bold(s: &str) -> String {
    paint("1", s)
}
fn dim(s: &str) -> String {
    paint("90", s)
}
fn cyan(s: &str) -> String {
    paint("36", s)
}
fn magenta(s: &str) -> String {
    paint("35", s)
}
fn yellow(s: &str) -> String {
    paint("33", s)
}
fn green(s: &str) -> String {
    paint("32", s)
}
#[allow(dead_code)]
fn red(s: &str) -> String {
    paint("31;1", s)
}
fn sky(s: &str) -> String {
    paint("38;2;56;189;248", s)
}
fn lime(s: &str) -> String {
    paint("38;2;163;230;53", s)
}
fn rose(s: &str) -> String {
    paint("38;2;251;113;133", s)
}

/// Gradient-ish wordmark: one vivid color per character.
fn wordmark(text: &str) -> String {
    if !color_on() {
        return text.to_string();
    }
    const C: [&str; 6] = [
        "38;2;34;211;238",
        "38;2;56;189;248",
        "38;2;232;121;249",
        "38;2;167;139;250",
        "38;2;251;191;36",
        "38;2;163;230;53",
    ];
    text.chars()
        .enumerate()
        .map(|(i, ch)| format!("\x1b[1;{}m{ch}\x1b[0m", C[i % C.len()]))
        .collect()
}

fn header_line(title: &str, glyph: &str) -> String {
    format!("{} {} {}", cyan(glyph), wordmark("ALAMUT"), dim(&format!("◆ {title} · {}", ts_now())))
}

fn rule() -> String {
    dim(&"─".repeat(56))
}

/// Color the meter by severity: lime < 60, amber < 85, rose above.
fn meter(pct: f64, width: usize) -> String {
    let pct = pct.clamp(0.0, 100.0);
    let fill = ((pct / 100.0) * width as f64).round() as usize;
    let body = format!("{}{}", "█".repeat(fill), "░".repeat(width.saturating_sub(fill)));
    let colored = if !color_on() {
        body
    } else if pct < 60.0 {
        paint("38;2;74;222;128", &body)
    } else if pct < 85.0 {
        paint("38;2;251;191;36", &body)
    } else {
        paint("38;2;251;113;133", &body)
    };
    format!("[{colored}] {:5.1}%", pct)
}

fn dot_live(ok: bool) -> String {
    if ok {
        green("●")
    } else {
        rose("○")
    }
}

// ---------------------------------------------------------------- formatting

pub fn fmt_bytes(b: u64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const KB: f64 = 1024.0;
    let f = b as f64;
    if f >= GB {
        format!("{:.1} GiB", f / GB)
    } else if f >= MB {
        format!("{:.1} MiB", f / MB)
    } else if f >= KB {
        format!("{:.1} KiB", f / KB)
    } else {
        format!("{b} B")
    }
}

pub fn fmt_bps(bps: f64) -> String {
    if bps >= 1_000_000_000.0 {
        format!("{:.2} Gbps", bps / 1_000_000_000.0)
    } else if bps >= 1_000_000.0 {
        format!("{:.1} Mbps", bps / 1_000_000.0)
    } else if bps >= 1_000.0 {
        format!("{:.1} Kbps", bps / 1_000.0)
    } else {
        format!("{bps:.0} bps")
    }
}

fn clear_screen() {
    print!("\x1B[2J\x1B[H");
}

fn ts_now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

// ------------------------------------------------------------ local fallback

struct LocalSys {
    cpu_util: f64,
    cpu_brand: String,
    logical: u32,
    physical: Option<u32>,
    freq_mhz: Option<u64>,
    mem_total: u64,
    mem_used: u64,
    mem_avail: u64,
}

fn local_sys_snapshot() -> LocalSys {
    use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
    let refresh = RefreshKind::nothing()
        .with_cpu(CpuRefreshKind::everything())
        .with_memory(MemoryRefreshKind::everything());
    let mut sys = System::new_with_specifics(refresh);
    sys.refresh_cpu_specifics(CpuRefreshKind::everything());
    sys.refresh_memory_specifics(MemoryRefreshKind::everything());
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu_specifics(CpuRefreshKind::everything());
    sys.refresh_memory_specifics(MemoryRefreshKind::everything());
    let cpus = sys.cpus();
    let first = cpus.first();
    let total = sys.total_memory();
    let used = sys.used_memory();
    let avail = sys.available_memory();
    // Overall = mean of all per-core usages.
    let cpu_util = if cpus.is_empty() {
        0.0
    } else {
        cpus.iter().map(|c| c.cpu_usage() as f64).sum::<f64>() / cpus.len() as f64
    };
    LocalSys {
        cpu_util,
        cpu_brand: first.map(|c| c.brand().to_string()).unwrap_or_default(),
        logical: cpus.len() as u32,
        physical: sys.physical_core_count().map(|n| n as u32),
        freq_mhz: first.map(|c| c.frequency()).filter(|&f| f > 0),
        mem_total: total,
        mem_used: used,
        mem_avail: avail,
    }
}

fn local_disks() -> Vec<(String, String, u64, u64)> {
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();
    let mut out = Vec::new();
    for d in disks.list() {
        if d.is_removable() || d.total_space() == 0 {
            continue;
        }
        let mount = d.mount_point().to_string_lossy().to_string();
        let total = d.total_space();
        let avail = d.available_space();
        out.push((mount.clone(), mount, total, avail));
    }
    out.sort();
    out
}

// ------------------------------------------------------------------ commands

pub async fn cmd_overview(api: &ApiClient, as_json: bool) -> Result<()> {
    let (status, cpu, mem, nics) = tokio::join!(
        api.call(RpcRequest::GetStatus),
        api.call(RpcRequest::GetCpuSnapshot),
        api.call(RpcRequest::GetMemorySnapshot),
        api.call(RpcRequest::ListNics),
    );
    let (ssd, hdd) = tokio::join!(
        api.call(RpcRequest::GetDisks { kind: DiskKind::Ssd }),
        api.call(RpcRequest::GetDisks { kind: DiskKind::Hdd }),
    );

    if as_json {
        let mut disks = Vec::new();
        if let Ok(RpcResponse::Disks(v)) = ssd {
            disks.extend(v);
        }
        if let Ok(RpcResponse::Disks(v)) = hdd {
            disks.extend(v);
        }
        let v = json!({
            "status": status.ok(),
            "cpu": cpu.ok(),
            "memory": mem.ok(),
            "nics": nics.ok(),
            "disks": disks,
        });
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }

    println!("{}", header_line("overview", "◈"));
    println!("{}", rule());
    match status {
        Ok(RpcResponse::Status(s)) => println!(
            "{} svc    {} {} {}",
            dot_live(s.running),
            if s.running { green("running") } else { yellow("stopped") },
            dim(&format!("(capture: {})", s.capture_mode)),
            cyan("◈")
        ),
        Err(e) => println!("{} svc    {} {}", dot_live(false), rose("unreachable"), dim(&e.to_string())),
        _ => {}
    }
    match cpu {
        Ok(RpcResponse::CpuSnapshot(c)) => {
            let label = if c.brand.is_empty() {
                format!("({} cores)", c.logical_cores)
            } else {
                format!("{} ({} cores)", c.brand, c.logical_cores)
            };
            println!("{} cpu    {} {}", cyan("󰻠"), meter(c.utilization, 20), dim(&label));
        }
        Err(e) => {
            let l = local_sys_snapshot();
            println!(
                "{} cpu    {} {}",
                cyan("󰻠"),
                meter(l.cpu_util, 20),
                dim(&format!("(local fallback · API: {e})"))
            );
        }
        _ => {}
    }
    match mem {
        Ok(RpcResponse::MemorySnapshot(m)) => println!(
            "{} mem    {} {}",
            magenta("󰍛"),
            meter(m.utilization, 20),
            bold(&format!("{}/{}", fmt_bytes(m.used_bytes), fmt_bytes(m.total_bytes)))
        ),
        Err(e) => {
            let l = local_sys_snapshot();
            let pct = if l.mem_total > 0 {
                l.mem_used as f64 / l.mem_total as f64 * 100.0
            } else {
                0.0
            };
            println!(
                "{} mem    {} {} {}",
                magenta("󰍛"),
                meter(pct, 20),
                bold(&format!("{}/{}", fmt_bytes(l.mem_used), fmt_bytes(l.mem_total))),
                dim(&format!("(local · API: {e})"))
            );
        }
        _ => {}
    }
    let mut disks = Vec::new();
    if let Ok(RpcResponse::Disks(v)) = ssd {
        disks.extend(v);
    }
    if let Ok(RpcResponse::Disks(v)) = hdd {
        disks.extend(v);
    }
    if disks.is_empty() {
        for (id, _, total, avail) in local_disks() {
            let used = total.saturating_sub(avail);
            let pct = if total > 0 {
                used as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            println!(
                "{} disk   {} {} {}",
                yellow("󰋊"),
                meter(pct, 20),
                bold(&format!("{id} {}/{}", fmt_bytes(used), fmt_bytes(total))),
                dim("(local)")
            );
        }
    } else {
        for d in &disks {
            println!(
                "{} disk   {} {}",
                yellow("󰋊"),
                meter(d.utilization, 20),
                bold(&format!("{} {}/{}", d.mount_point, fmt_bytes(d.used_bytes), fmt_bytes(d.total_bytes)))
            );
        }
    }
    match nics {
        Ok(RpcResponse::Nics(list)) => {
            let up: Vec<_> = list.iter().filter(|n| n.oper_status == "Up").collect();
            println!(
                "{} net    {} {}",
                sky("󰈀"),
                bold(&format!("{}/{} up", up.len(), list.len())),
                dim("adapters")
            );
            for n in up.iter().take(8) {
                println!(
                    "         {} {} {} {} {} {}",
                    green("▲"),
                    bold(&truncate(&n.name, 26)),
                    dim(&n.media_type),
                    cyan(n.ipv4_addresses.first().map(|s| s.as_str()).unwrap_or("—")),
                    dim("▼"),
                    fmt_bps(n.rx_bps),
                );
            }
        }
        Err(e) => println!("{} net    {} {}", sky("󰈀"), rose("unreachable"), dim(&e.to_string())),
        _ => {}
    }
    println!("{}", rule());
    Ok(())
}

pub async fn cmd_cpu(api: &ApiClient, as_json: bool) -> Result<()> {
    match api.call(RpcRequest::GetCpuSnapshot).await {
        Ok(RpcResponse::CpuSnapshot(c)) => {
            if as_json {
                println!("{}", serde_json::to_string_pretty(&c)?);
                return Ok(());
            }
            println!("{}", header_line("cpu", "󰻠"));
            println!("{}", rule());
            let label = if c.brand.is_empty() { "—".to_string() } else { c.brand.clone() };
            println!("{} {}  {}", cyan("󰻠"), bold(&label), meter(c.utilization, 26));
            println!(
                "  {} cores logical={} physical={} freq={}",
                dim("◎"),
                lime(&c.logical_cores.to_string()),
                lime(&c.physical_cores.map(|v| v.to_string()).unwrap_or_else(|| "?".into())),
                lime(&c.frequency_mhz.map(|f| format!("{f} MHz")).unwrap_or_else(|| "n/a".into()))
            );
            for (i, u) in c.per_core.iter().enumerate() {
                println!("  {} core {i:2} {}", dim("│"), meter(*u, 26));
            }
            println!("{}", rule());
            Ok(())
        }
        Ok(other) => anyhow::bail!("unexpected reply: {other:?}"),
        Err(e) => {
            let l = local_sys_snapshot();
            if as_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "brand": l.cpu_brand, "utilization": l.cpu_util,
                        "logical_cores": l.logical, "physical_cores": l.physical,
                        "frequency_mhz": l.freq_mhz, "source": "local-fallback",
                    }))?
                );
                return Ok(());
            }
            eprintln!("{} {}", yellow("!"), dim(&format!("API unreachable ({e}) — local fallback")));
            println!("{}", header_line("cpu · local", "󰻠"));
            println!("{} {}  {}", cyan("󰻠"), bold(&l.cpu_brand), meter(l.cpu_util, 26));
            println!("  {} cores logical={}", dim("◎"), lime(&l.logical.to_string()));
            println!("{}", rule());
            Ok(())
        }
    }
}

pub async fn cmd_memory(api: &ApiClient, as_json: bool) -> Result<()> {
    match api.call(RpcRequest::GetMemorySnapshot).await {
        Ok(RpcResponse::MemorySnapshot(m)) => {
            if as_json {
                println!("{}", serde_json::to_string_pretty(&m)?);
                return Ok(());
            }
            println!("{}", header_line("memory", "󰍛"));
            println!("{}", rule());
            println!(
                "{} {} {}",
                magenta("󰍛"),
                meter(m.utilization, 26),
                bold(&format!("{}/{}", fmt_bytes(m.used_bytes), fmt_bytes(m.total_bytes)))
            );
            println!("  {} avail {}", dim("◦"), lime(&fmt_bytes(m.available_bytes)));
            println!("{}", rule());
            Ok(())
        }
        Ok(other) => anyhow::bail!("unexpected reply: {other:?}"),
        Err(e) => {
            let l = local_sys_snapshot();
            let pct = if l.mem_total > 0 {
                l.mem_used as f64 / l.mem_total as f64 * 100.0
            } else {
                0.0
            };
            if as_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "total_bytes": l.mem_total, "used_bytes": l.mem_used,
                        "available_bytes": l.mem_avail, "utilization": pct,
                        "source": "local-fallback",
                    }))?
                );
                return Ok(());
            }
            eprintln!("{} {}", yellow("!"), dim(&format!("API unreachable ({e}) — local fallback")));
            println!("{}", header_line("memory · local", "󰍛"));
            println!(
                "{} {} {}",
                magenta("󰍛"),
                meter(pct, 26),
                bold(&format!("{}/{}", fmt_bytes(l.mem_used), fmt_bytes(l.mem_total)))
            );
            println!("{}", rule());
            Ok(())
        }
    }
}

pub async fn cmd_disk(api: &ApiClient, as_json: bool) -> Result<()> {
    let (ssd, hdd) = tokio::join!(
        api.call(RpcRequest::GetDisks { kind: DiskKind::Ssd }),
        api.call(RpcRequest::GetDisks { kind: DiskKind::Hdd }),
    );
    let mut disks = Vec::new();
    let mut api_ok = false;
    if let Ok(RpcResponse::Disks(v)) = ssd {
        api_ok = true;
        disks.extend(v);
    }
    if let Ok(RpcResponse::Disks(v)) = hdd {
        api_ok = true;
        disks.extend(v);
    }
    if !api_ok {
        eprintln!("{} {}", yellow("!"), dim("API unreachable — local fallback"));
        let local = local_disks();
        if as_json {
            let v: Vec<_> = local
                .iter()
                .map(|(id, mount, total, avail)| {
                    let used = total.saturating_sub(*avail);
                    json!({"id": id, "mount_point": mount, "total_bytes": total,
                           "used_bytes": used, "available_bytes": avail, "source": "local-fallback"})
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&v)?);
            return Ok(());
        }
        println!("{}", header_line("disk · local", "󰋊"));
        println!("{}", rule());
        for (id, _, total, avail) in &local {
            let used = total.saturating_sub(*avail);
            let pct = if *total > 0 { used as f64 / *total as f64 * 100.0 } else { 0.0 };
            println!(
                "{} {:8} {} {}",
                yellow("󰋊"),
                bold(id),
                meter(pct, 24),
                dim(&format!("{}/{}", fmt_bytes(used), fmt_bytes(*total)))
            );
        }
        println!("{}", rule());
        return Ok(());
    }
    if as_json {
        println!("{}", serde_json::to_string_pretty(&disks)?);
        return Ok(());
    }
    println!("{}", header_line("disk", "󰋊"));
    println!("{}", rule());
    if disks.is_empty() {
        println!("  {} {}", dim("○"), dim("no disks reported"));
    }
    for d in &disks {
        let kind = match d.kind {
            DiskKind::Ssd => cyan("ssd"),
            DiskKind::Hdd => magenta("hdd"),
        };
        println!(
            "{} {:8} {} {} {} {}",
            yellow("󰋊"),
            bold(&d.mount_point),
            meter(d.utilization, 24),
            dim(&format!("{}/{}", fmt_bytes(d.used_bytes), fmt_bytes(d.total_bytes))),
            kind,
            dim(&d.file_system)
        );
    }
    println!("{}", rule());
    Ok(())
}

pub async fn cmd_network(api: &ApiClient, as_json: bool) -> Result<()> {
    let res = api.call(RpcRequest::ListNics).await?;
    let list = match res {
        RpcResponse::Nics(v) => v,
        other => anyhow::bail!("unexpected reply: {other:?}"),
    };
    if as_json {
        println!("{}", serde_json::to_string_pretty(&list)?);
        return Ok(());
    }
    println!("{}", header_line("network", "󰈀"));
    println!("{}", rule());
    println!(
        "  {:24} {:8} {:6} {:15} {:>12} {:>12}",
        dim("NAME"),
        dim("TYPE"),
        dim("STATE"),
        dim("IPv4"),
        dim("▼ DOWN"),
        dim("▲ UP")
    );
    for n in &list {
        let state = if n.oper_status == "Up" {
            green("Up")
        } else {
            dim("Down")
        };
        println!(
            "  {:24} {:8} {:6} {:15} {:>12} {:>12}",
            bold(&truncate(&n.name, 24)),
            dim(&n.media_type),
            state,
            cyan(n.ipv4_addresses.first().map(|s| s.as_str()).unwrap_or("—")),
            fmt_bps(n.rx_bps),
            fmt_bps(n.tx_bps),
        );
    }
    println!("{}", rule());
    Ok(())
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n.saturating_sub(1)).collect::<String>() + "…"
    }
}

pub async fn cmd_os(api: &ApiClient, as_json: bool) -> Result<()> {
    let name = sysinfo::System::name().unwrap_or_default();
    let ver = sysinfo::System::os_version().unwrap_or_default();
    let kernel = sysinfo::System::kernel_version().unwrap_or_default();
    let host = sysinfo::System::host_name().unwrap_or_else(|| "?".into());
    let arch = std::env::consts::ARCH;
    let uptime = sysinfo::System::uptime();
    let online = api.call(RpcRequest::Ping).await.is_ok();
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "os_name": name, "os_version": ver, "kernel": kernel,
                "hostname": host, "arch": arch, "uptime_secs": uptime,
                "api": if online { "online" } else { "offline" },
            }))?
        );
        return Ok(());
    }
    println!("{}", header_line("os", ""));
    println!("{}", rule());
    println!("  {} host   {}", sky("󰟀"), bold(&host));
    println!("  {} os     {} {} {}", sky(""), bold(&name), lime(&ver), dim(&format!("({arch})")));
    println!("  {} kernel {}", dim("◦"), kernel);
    println!("  {} uptime {} {}", dim("◦"), lime(&fmt_uptime(uptime)), dim(&format!("({uptime}s)")));
    println!("  {} api     {} {}", dot_live(online), if online { green("online") } else { rose("offline") }, cyan("◈"));
    println!("{}", rule());
    Ok(())
}

fn fmt_uptime(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

pub async fn cmd_machine(api: &ApiClient, as_json: bool) -> Result<()> {
    let res = api.call(RpcRequest::GetHardwareInventory).await?;
    let inv = match res {
        RpcResponse::HardwareInventory(v) => v,
        other => anyhow::bail!("unexpected reply: {other:?}"),
    };
    if as_json {
        println!("{}", serde_json::to_string_pretty(&inv)?);
        return Ok(());
    }
    println!("{}", header_line("machine info", "󰌢"));
    println!("{}", rule());
    println!("  {} cpu   {} {}", cyan("󰻠"), bold(&inv.cpu.brand), bold(&inv.cpu.model));
    println!(
        "           {} cores={} threads={} base={}",
        dim("└"),
        lime(&inv.cpu.physical_cores.map(|v| v.to_string()).unwrap_or_else(|| "?".into())),
        lime(&inv.cpu.logical_processors.to_string()),
        lime(&inv.cpu.base_speed_mhz.map(|f| format!("{f} MHz")).unwrap_or_else(|| "n/a".into()))
    );
    println!(
        "  {} mem   {} {} {}",
        magenta("󰍛"),
        bold(&inv.memory.brand),
        bold(&inv.memory.model),
        lime(&fmt_bytes(inv.memory.size_bytes)),
    );
    println!(
        "           {} x{} {}{}",
        dim("└"),
        inv.memory.modules,
        inv.memory.memory_type,
        if inv.memory.form_factor.is_empty() {
            String::new()
        } else {
            format!(" · {}", inv.memory.form_factor)
        }
    );
    println!("  {} board {} {}", yellow("󰌢"), bold(&inv.motherboard.brand), inv.motherboard.model);
    for d in &inv.disks {
        println!(
            "  {} disk  {} {} {} {}",
            yellow("󰋊"),
            bold(&d.brand),
            d.model,
            lime(&fmt_bytes(d.capacity_bytes)),
            dim(match d.kind {
                DiskKind::Ssd => "ssd",
                DiskKind::Hdd => "hdd",
            })
        );
    }
    for g in &inv.gpus {
        println!("  {} gpu   {} {}", sky("󰢮"), bold(&g.brand), g.model);
    }
    println!("{}", rule());
    Ok(())
}

// ------------------------------------------------------------------ rebuild

/// ponytail: shells out to the existing reinstall script (build → uninstall → install → start),
/// then relaunches alamut-cli. Paths are baked at compile time from the workspace layout;
/// a relocated binary needs them passed in instead.
pub async fn cmd_rebuild() -> Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let script = root.join("scripts").join("reinstall-windows-service.ps1");

    println!("{} {}", cyan("◈"), dim("closing running alamut-api / alamut-cli instances"));
    let me = std::process::id().to_string();
    // taskkill exits non-zero when nothing matched; that is fine.
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "alamut-cli.exe", "/FI", &format!("PID ne {me}")])
        .status();
    let _ = std::process::Command::new("taskkill").args(["/F", "/IM", "alamut-api.exe"]).status();

    println!("{} {}", cyan("◈"), dim(&format!("rebuild via {}", script.display())));
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .status()?;
    anyhow::ensure!(status.success(), "rebuild failed ({status}) — run from an elevated shell");

    println!("{} {}", cyan("◈"), dim("rebuilding and relaunching alamut-cli"));
    let built = std::process::Command::new("cargo")
        .args(["build", "-p", "alamut-cli", "--release"])
        .current_dir(&root)
        .status()?;
    anyhow::ensure!(built.success(), "alamut-cli build failed ({built})");
    let exe = root.join("target").join("release").join("alamut-cli.exe");
    std::process::Command::new("cmd")
        .args(["/C", "start", "", &exe.to_string_lossy()])
        .spawn()?;
    Ok(())
}

// ---------------------------------------------------------------- live loops

async fn live_loop(title: &str, glyph: &str, interval_ms: u64, mut draw: impl FnMut() -> Result<()>) -> Result<()> {
    println!(
        "{} {} {} {}",
        cyan(glyph),
        wordmark("ALAMUT"),
        dim(&format!("◆ {title} · live · every {interval_ms} ms · Ctrl+C to stop")),
        dot_live(true)
    );
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\n{} {}", dim("■"), dim("stopped."));
                return Ok(());
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms.max(250))) => {
                clear_screen();
                println!(
                    "{} {} {}",
                    cyan(glyph),
                    wordmark("ALAMUT"),
                    dim(&format!("◆ {title} · {} · Ctrl+C stops", ts_now()))
                );
                println!("{}", rule());
                if let Err(e) = draw() {
                    eprintln!("  {} {}", rose("✖"), e);
                }
                println!("{}", rule());
            }
        }
    }
}

pub async fn cmd_cpu_live(api: ApiClient, interval_ms: u64) -> Result<()> {
    live_loop("cpu", "󰻠", interval_ms, || {
        let snap = tokio::runtime::Handle::try_current()
            .ok()
            .and_then(|h| h.block_on(api.call(RpcRequest::GetCpuSnapshot)).ok());
        match snap {
            Some(RpcResponse::CpuSnapshot(c)) => {
                println!("  {} {}  {}", cyan("󰻠"), bold(&c.brand), meter(c.utilization, 28));
                for (i, u) in c.per_core.iter().enumerate().take(16) {
                    println!("    {} core {i:2} {}", dim("│"), meter(*u, 28));
                }
                if c.per_core.len() > 16 {
                    println!("    {} … ({} cores total)", dim("└"), c.per_core.len());
                }
                Ok(())
            }
            _ => {
                let l = local_sys_snapshot();
                println!("  {} {}  {} {}", cyan("󰻠"), bold(&l.cpu_brand), meter(l.cpu_util, 28), dim("(local)"));
                Ok(())
            }
        }
    })
    .await
}

pub async fn cmd_memory_live(api: ApiClient, interval_ms: u64) -> Result<()> {
    live_loop("memory", "󰍛", interval_ms, || {
        let snap = tokio::runtime::Handle::try_current()
            .ok()
            .and_then(|h| h.block_on(api.call(RpcRequest::GetMemorySnapshot)).ok());
        match snap {
            Some(RpcResponse::MemorySnapshot(m)) => {
                println!(
                    "  {} {} {}",
                    magenta("󰍛"),
                    meter(m.utilization, 28),
                    bold(&format!("{}/{}", fmt_bytes(m.used_bytes), fmt_bytes(m.total_bytes)))
                );
                Ok(())
            }
            _ => {
                let l = local_sys_snapshot();
                let pct = if l.mem_total > 0 {
                    l.mem_used as f64 / l.mem_total as f64 * 100.0
                } else {
                    0.0
                };
                println!(
                    "  {} {} {} {}",
                    magenta("󰍛"),
                    meter(pct, 28),
                    bold(&format!("{}/{}", fmt_bytes(l.mem_used), fmt_bytes(l.mem_total))),
                    dim("(local)")
                );
                Ok(())
            }
        }
    })
    .await
}

pub async fn cmd_disk_live(api: ApiClient, interval_ms: u64) -> Result<()> {
    live_loop("disk", "󰋊", interval_ms, || {
        let disks: Vec<alamut_core::types::DiskSnapshot> = tokio::runtime::Handle::try_current()
            .ok()
            .map(|h| {
                let (ssd, hdd) = (
                    h.block_on(api.call(RpcRequest::GetDisks { kind: DiskKind::Ssd })),
                    h.block_on(api.call(RpcRequest::GetDisks { kind: DiskKind::Hdd })),
                );
                let mut out = Vec::new();
                if let Ok(RpcResponse::Disks(v)) = ssd {
                    out.extend(v);
                }
                if let Ok(RpcResponse::Disks(v)) = hdd {
                    out.extend(v);
                }
                out
            })
            .unwrap_or_default();
        if !disks.is_empty() {
            for d in &disks {
                println!(
                    "  {} {:8} {} {}",
                    yellow("󰋊"),
                    bold(&d.mount_point),
                    meter(d.utilization, 28),
                    dim(&format!("{}/{}", fmt_bytes(d.used_bytes), fmt_bytes(d.total_bytes)))
                );
            }
            return Ok(());
        }
        for (id, _, total, avail) in local_disks() {
            let used = total.saturating_sub(avail);
            let pct = if total > 0 { used as f64 / total as f64 * 100.0 } else { 0.0 };
            println!(
                "  {} {:8} {} {} {}",
                yellow("󰋊"),
                bold(&id),
                meter(pct, 28),
                dim(&format!("{}/{}", fmt_bytes(used), fmt_bytes(total))),
                dim("(local)")
            );
        }
        Ok(())
    })
    .await
}

pub async fn cmd_network_live(api: ApiClient, interval_ms: u64) -> Result<()> {
    live_loop("network", "󰈀", interval_ms, || {
        let list = tokio::runtime::Handle::try_current()
            .ok()
            .and_then(|h| h.block_on(api.call(RpcRequest::ListNics)).ok());
        let list = match list {
            Some(RpcResponse::Nics(v)) => v,
            _ => anyhow::bail!("API unreachable — start it with `alamut run`"),
        };
        println!(
            "    {:24} {:6} {:>12} {:>12}",
            dim("NAME"),
            dim("STATE"),
            dim("▼ DOWN"),
            dim("▲ UP")
        );
        for n in &list {
            if n.media_type == "Loopback" {
                continue;
            }
            let state = if n.oper_status == "Up" { green("Up") } else { dim("Down") };
            println!(
                "    {:24} {:6} {:>12} {:>12}",
                bold(&truncate(&n.name, 24)),
                state,
                cyan(&fmt_bps(n.rx_bps)),
                magenta(&fmt_bps(n.tx_bps)),
            );
        }
        Ok(())
    })
    .await
}

pub async fn cmd_live(api: ApiClient, interval_ms: u64) -> Result<()> {
    live_loop("live", "◈", interval_ms, || {
        let h = tokio::runtime::Handle::try_current().map_err(|_| anyhow::anyhow!("no runtime"))?;
        let cpu = h.block_on(api.call(RpcRequest::GetCpuSnapshot));
        let mem = h.block_on(api.call(RpcRequest::GetMemorySnapshot));
        let nics = h.block_on(api.call(RpcRequest::ListNics));
        if let Ok(RpcResponse::CpuSnapshot(c)) = cpu {
            println!("  {} cpu    {} {}", cyan("󰻠"), meter(c.utilization, 26), dim(&c.brand));
        } else {
            let l = local_sys_snapshot();
            println!("  {} cpu    {} {}", cyan("󰻠"), meter(l.cpu_util, 26), dim("(local)"));
        }
        if let Ok(RpcResponse::MemorySnapshot(m)) = mem {
            println!(
                "  {} memory {} {}",
                magenta("󰍛"),
                meter(m.utilization, 26),
                dim(&format!("{}/{}", fmt_bytes(m.used_bytes), fmt_bytes(m.total_bytes)))
            );
        }
        if let Ok(RpcResponse::Nics(list)) = nics {
            for n in list.iter().filter(|n| n.oper_status == "Up").take(6) {
                println!(
                    "  {} {:10} {} {} {} {}",
                    sky("󰈀"),
                    bold(&truncate(&n.name, 10)),
                    dim("▼"),
                    cyan(&fmt_bps(n.rx_bps)),
                    dim("▲"),
                    magenta(&fmt_bps(n.tx_bps))
                );
            }
        }
        Ok(())
    })
    .await
}

//! Shared state model: views, snapshot data, the polling loop, and tool runs.

use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::Local;
use alamut_core::history::HistoryRange;
use alamut_core::ipc::{RpcRequest, RpcResponse};
use alamut_core::types::*;
use serde::Serialize;
use tokio::sync::mpsc;

use crate::api::{grab, ApiClient};

// ---------------------------------------------------------------- tabs / views

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Overview,
    Nics,
    Bandwidth,
    Latency,
    System,
    Apps,
    Tools,
}

impl View {
    pub const ALL: [View; 7] = [
        View::Overview,
        View::Nics,
        View::Bandwidth,
        View::Latency,
        View::System,
        View::Apps,
        View::Tools,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            View::Overview => "Overview",
            View::Nics => "NICs",
            View::Bandwidth => "Bandwidth",
            View::Latency => "Latency",
            View::System => "System",
            View::Apps => "Apps",
            View::Tools => "Tools",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            View::Overview => "◈",
            View::Nics => "⌗",
            View::Bandwidth => "⇈",
            View::Latency => "≈",
            View::System => "◎",
            View::Apps => "⊞",
            View::Tools => "⚙",
        }
    }

    pub fn index(&self) -> usize {
        View::ALL.iter().position(|v| v == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> View {
        View::ALL[i % View::ALL.len()]
    }
}

// --------------------------------------------------------------------- tools

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Ping,
    HttpLatency,
    Traceroute,
    Nslookup,
    Speedtest,
}

impl ToolKind {
    pub const ALL: [ToolKind; 5] = [
        ToolKind::Ping,
        ToolKind::HttpLatency,
        ToolKind::Traceroute,
        ToolKind::Nslookup,
        ToolKind::Speedtest,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            ToolKind::Ping => "Ping",
            ToolKind::HttpLatency => "HTTP latency",
            ToolKind::Traceroute => "Traceroute",
            ToolKind::Nslookup => "nslookup",
            ToolKind::Speedtest => "Speedtest",
        }
    }

    /// Prefill shown when the input is empty.
    pub fn placeholder(&self) -> &'static str {
        match self {
            ToolKind::Ping => "1.1.1.1",
            ToolKind::HttpLatency => "https://example.com",
            ToolKind::Traceroute => "1.1.1.1",
            ToolKind::Nslookup => "google.com",
            ToolKind::Speedtest => "(no target needed)",
        }
    }

    pub fn hint(&self) -> &'static str {
        match self {
            ToolKind::Ping => "ICMP echo, 5 packets",
            ToolKind::HttpLatency => "DNS / connect / TLS / TTFB breakdown",
            ToolKind::Traceroute => "hop-by-hop path to the target",
            ToolKind::Nslookup => "DNS record lookup",
            ToolKind::Speedtest => "Ookla download / upload / ping",
        }
    }

    pub fn needs_input(&self) -> bool {
        !matches!(self, ToolKind::Speedtest)
    }

    pub fn index(&self) -> usize {
        ToolKind::ALL.iter().position(|t| t == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> ToolKind {
        ToolKind::ALL[i % ToolKind::ALL.len()]
    }
}

// ------------------------------------------------------------------ snapshot

#[derive(Debug, Clone, Default, Serialize)]
pub struct Snapshot {
    pub connected: bool,
    pub error: Option<String>,
    pub updated: Option<chrono::DateTime<Local>>,
    pub status: Option<ServiceStatus>,
    pub nics: Vec<NicInfo>,
    /// (ts, rx_bytes, tx_bytes) summed across NICs for the selected range.
    pub bandwidth: Vec<UsagePoint>,
    pub ping: Vec<PingSample>,
    pub http: Vec<HttpLatencySample>,
    pub link_events: Vec<LinkEvent>,
    pub cpu: Option<CpuSnapshot>,
    pub memory: Option<MemorySnapshot>,
    pub disks: Vec<DiskSnapshot>,
    pub cpu_history: Option<SystemMetricHistory>,
    pub mem_history: Option<SystemMetricHistory>,
    pub usage: Vec<AppUsageRow>,
    pub thermal: Option<ThermalSnapshot>,
    pub speedtests: Vec<SpeedtestResult>,
    pub eula_accepted: Option<bool>,
}

impl Snapshot {
    /// Down (rx) / up (tx) rates in bits-per-second derived from byte-counter deltas.
    pub fn bandwidth_rates(&self) -> Vec<(i64, f64, f64)> {
        let mut out = Vec::with_capacity(self.bandwidth.len());
        let mut prev: Option<&UsagePoint> = None;
        for p in &self.bandwidth {
            if let Some(w) = prev {
                let dt = (p.ts - w.ts) as f64;
                if dt > 0.0 {
                    let rx = ((p.rx_bytes.saturating_sub(w.rx_bytes)) as f64 * 8.0) / dt;
                    let tx = ((p.tx_bytes.saturating_sub(w.tx_bytes)) as f64 * 8.0) / dt;
                    out.push((p.ts, rx.max(0.0), tx.max(0.0)));
                }
            }
            prev = Some(p);
        }
        out
    }
}

// --------------------------------------------------------------- commands / events

#[derive(Debug, Clone)]
pub enum Cmd {
    /// Fetch immediately for the given view (also used to force a refresh).
    SetView(View),
    /// Restrict history queries to one NIC (None = all NICs).
    SelectNic(Option<String>),
    Refresh,
    RunTool {
        kind: ToolKind,
        target: String,
        accept_eula: bool,
    },
    AcceptEula,
    Quit,
}

#[derive(Debug, Clone)]
pub enum ToolEvent {
    Status(String),
    Line(String),
    Finished,
    Failed(String),
}

#[derive(Debug)]
pub enum Ev {
    Snapshot(Box<Snapshot>),
    Tool(ToolEvent),
}

// ------------------------------------------------------------------ poll loop

pub async fn poll_loop(
    api: ApiClient,
    mut rx: mpsc::Receiver<Cmd>,
    tx: mpsc::Sender<Ev>,
    tick: Duration,
) {
    let mut view = View::Overview;
    let mut nic: Option<String> = None;
    let mut interval = tokio::time::interval(tick);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // First frame immediately instead of waiting a full tick.
    interval.tick().await;

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let snap = fetch(&api, view, nic.clone()).await;
                if tx.send(Ev::Snapshot(Box::new(snap))).await.is_err() { break; }
            }
            cmd = rx.recv() => match cmd {
                None | Some(Cmd::Quit) => break,
                Some(Cmd::SetView(v)) => {
                    view = v;
                    let snap = fetch(&api, view, nic.clone()).await;
                    if tx.send(Ev::Snapshot(Box::new(snap))).await.is_err() { break; }
                }
                Some(Cmd::SelectNic(id)) => {
                    nic = id;
                    let snap = fetch(&api, view, nic.clone()).await;
                    if tx.send(Ev::Snapshot(Box::new(snap))).await.is_err() { break; }
                }
                Some(Cmd::Refresh) => {
                    let snap = fetch(&api, view, nic.clone()).await;
                    if tx.send(Ev::Snapshot(Box::new(snap))).await.is_err() { break; }
                }
                Some(Cmd::AcceptEula) => {
                    spawn_eula(api.clone(), tx.clone());
                }
                Some(Cmd::RunTool { kind, target, accept_eula }) => {
                    spawn_tool(api.clone(), tx.clone(), kind, target, nic.clone(), accept_eula);
                }
            }
        }
    }
}

/// One-shot overview fetch (used by `--dump` for headless diagnostics).
pub async fn fetch_overview(api: &ApiClient) -> Snapshot {
    fetch(api, View::Overview, None).await
}

async fn fetch(api: &ApiClient, view: View, nic: Option<String>) -> Snapshot {
    let mut snap = Snapshot::default();
    let mut errors: Vec<String> = Vec::new();
    let mut any_ok = false;
    let range = HistoryRange::Today;

    // Baseline: always refreshed so the header/gauges never go stale.
    let (status, cpu, mem) = tokio::join!(
        api.call(RpcRequest::GetStatus),
        api.call(RpcRequest::GetCpuSnapshot),
        api.call(RpcRequest::GetMemorySnapshot),
    );
    if let Some(v) = grab!(status => Status, errors) {
        any_ok = true;
        snap.status = Some(v);
    }
    if let Some(v) = grab!(cpu => CpuSnapshot, errors) {
        any_ok = true;
        snap.cpu = Some(v);
    }
    if let Some(v) = grab!(mem => MemorySnapshot, errors) {
        any_ok = true;
        snap.memory = Some(v);
    }

    match view {
        View::Overview => {
            let (nics, bw, ssd, hdd) = tokio::join!(
                api.call(RpcRequest::ListNics),
                bandwidth_req(api, nic.clone()),
                api.call(RpcRequest::GetDisks { kind: DiskKind::Ssd }),
                api.call(RpcRequest::GetDisks { kind: DiskKind::Hdd }),
            );
            if let Some(v) = grab!(nics => Nics, errors) {
                any_ok = true;
                snap.nics = v;
            }
            if let Some(v) = bw {
                any_ok = true;
                snap.bandwidth = v;
            }
            let mut disks: Vec<DiskSnapshot> = Vec::new();
            if let Some(v) = grab!(ssd => Disks, errors) {
                any_ok = true;
                disks.extend(v);
            }
            if let Some(v) = grab!(hdd => Disks, errors) {
                any_ok = true;
                disks.extend(v);
            }
            snap.disks = disks;
        }
        View::Nics => {
            let nics = api.call(RpcRequest::ListNics).await;
            if let Some(v) = grab!(nics => Nics, errors) {
                any_ok = true;
                snap.nics = v;
            }
        }
        View::Bandwidth => {
            let (nics, bw) = tokio::join!(
                api.call(RpcRequest::ListNics),
                bandwidth_req(api, nic.clone()),
            );
            if let Some(v) = grab!(nics => Nics, errors) {
                any_ok = true;
                snap.nics = v;
            }
            if let Some(v) = bw {
                any_ok = true;
                snap.bandwidth = v;
            }
        }
        View::Latency => {
            let (ping, http, events) = tokio::join!(
                api.call(RpcRequest::GetPingHistory {
                    nic_id: nic.clone(),
                    range,
                    start_ts: None,
                    end_ts: None,
                }),
                api.call(RpcRequest::GetHttpLatencyHistory {
                    nic_id: nic.clone(),
                    range,
                    start_ts: None,
                    end_ts: None,
                }),
                api.call(RpcRequest::GetLinkEvents {
                    nic_id: nic.clone(),
                    range,
                    start_ts: None,
                    end_ts: None,
                }),
            );
            if let Some(v) = grab!(ping => PingHistory, errors) {
                any_ok = true;
                snap.ping = v;
            }
            if let Some(v) = grab!(http => HttpLatencyHistory, errors) {
                any_ok = true;
                snap.http = v;
            }
            if let Some(v) = grab!(events => LinkEvents, errors) {
                any_ok = true;
                snap.link_events = v;
            }
        }
        View::System => {
            let (ssd, hdd, cpu_h, mem_h, thermal) = tokio::join!(
                api.call(RpcRequest::GetDisks { kind: DiskKind::Ssd }),
                api.call(RpcRequest::GetDisks { kind: DiskKind::Hdd }),
                api.call(RpcRequest::GetCpuHistory {
                    range,
                    start_ts: None,
                    end_ts: None,
                }),
                api.call(RpcRequest::GetMemoryHistory {
                    range,
                    start_ts: None,
                    end_ts: None,
                }),
                api.call(RpcRequest::GetThermalSnapshot),
            );
            let mut disks: Vec<DiskSnapshot> = Vec::new();
            if let Some(v) = grab!(ssd => Disks, errors) {
                any_ok = true;
                disks.extend(v);
            }
            if let Some(v) = grab!(hdd => Disks, errors) {
                any_ok = true;
                disks.extend(v);
            }
            snap.disks = disks;
            if let Some(v) = grab!(cpu_h => CpuHistory, errors) {
                any_ok = true;
                snap.cpu_history = Some(v);
            }
            if let Some(v) = grab!(mem_h => MemoryHistory, errors) {
                any_ok = true;
                snap.mem_history = Some(v);
            }
            if let Some(v) = grab!(thermal => ThermalSnapshot, errors) {
                any_ok = true;
                snap.thermal = Some(v);
            }
        }
        View::Apps => {
            let usage = api
                .call(RpcRequest::GetAppUsage {
                    range,
                    start_ts: None,
                    end_ts: None,
                    group_by: "process".to_string(),
                    nic_id: nic.clone(),
                })
                .await;
            if let Some(v) = grab!(usage => AppUsage, errors) {
                any_ok = true;
                snap.usage = v;
            }
        }
        View::Tools => {
            let (speed, settings) = tokio::join!(
                api.call(RpcRequest::GetSpeedtestHistory {
                    range,
                    start_ts: None,
                    end_ts: None,
                }),
                api.call(RpcRequest::GetSettings),
            );
            if let Some(v) = grab!(speed => SpeedtestHistory, errors) {
                any_ok = true;
                snap.speedtests = v;
            }
            if let Some(v) = grab!(settings => Settings, errors) {
                any_ok = true;
                snap.eula_accepted = Some(v.speedtest_eula_accepted);
            }
        }
    }

    snap.connected = any_ok;
    snap.error = if any_ok {
        None
    } else {
        Some(
            errors
                .into_iter()
                .next()
                .unwrap_or_else(|| "no response from API".to_string()),
        )
    };
    snap.updated = Some(Local::now());
    snap
}

/// Fetch bandwidth history and collapse duplicate timestamps (one row per NIC).
async fn bandwidth_req(api: &ApiClient, nic: Option<String>) -> Option<Vec<UsagePoint>> {
    let res = api
        .call(RpcRequest::GetBandwidthHistory {
            nic_id: nic,
            range: HistoryRange::Today,
            start_ts: None,
            end_ts: None,
        })
        .await;
    let mut errors = Vec::new();
    let mut points = grab!(res => BandwidthHistory, errors)?;
    points.sort_by_key(|p| p.ts);
    let mut merged: Vec<UsagePoint> = Vec::with_capacity(points.len());
    for p in points {
        match merged.last_mut() {
            Some(last) if last.ts == p.ts => {
                last.rx_bytes = last.rx_bytes.saturating_add(p.rx_bytes);
                last.tx_bytes = last.tx_bytes.saturating_add(p.tx_bytes);
            }
            _ => merged.push(p),
        }
    }
    Some(merged)
}

// ------------------------------------------------------------------ tool runs

fn spawn_eula(api: ApiClient, tx: mpsc::Sender<Ev>) {
    tokio::spawn(async move {
        let _ = api.call(RpcRequest::AcceptSpeedtestEula).await;
        let _ = tx
            .send(Ev::Tool(ToolEvent::Status(
                "Speedtest EULA accepted — ready to run".into(),
            )))
            .await;
    });
}

fn spawn_tool(
    api: ApiClient,
    tx: mpsc::Sender<Ev>,
    kind: ToolKind,
    target: String,
    nic: Option<String>,
    accept_eula: bool,
) {
    tokio::spawn(async move {
        let label = kind.title();
        let _ = tx
            .send(Ev::Tool(ToolEvent::Status(format!("running {label}…"))))
            .await;
        let res = tool_run(&api, kind, target, nic, accept_eula).await;
        emit_result(res, &tx).await;
    });
}

async fn tool_run(
    api: &ApiClient,
    kind: ToolKind,
    target: String,
    nic: Option<String>,
    accept_eula: bool,
) -> Result<Vec<String>> {
    let res: Result<Vec<String>> = match kind {
            ToolKind::Ping => {
                let r = api
                    .call(RpcRequest::RunPing {
                        target,
                        nic_id: nic,
                        count: Some(5),
                        packet_size: None,
                    })
                    .await?;
                let s = match r {
                    RpcResponse::PingResult(s) => s,
                    other => return Err(anyhow!("unexpected reply: {other:?}")),
                };
                Ok(vec![
                    format!("target      {}", s.target),
                    format!(
                        "result      {}",
                        if s.success { "OK" } else { "FAILED" }
                    ),
                    format!(
                        "rtt         {}",
                        s.rtt_ms
                            .map(|v| format!("{v:.1} ms"))
                            .unwrap_or_else(|| "n/a".into())
                    ),
                    format!(
                        "time        {}",
                        chrono::DateTime::from_timestamp(s.ts, 0)
                            .map(|d| d.with_timezone(&Local).to_rfc3339())
                            .unwrap_or_default()
                    ),
                    match s.error {
                        Some(e) => format!("error       {e}"),
                        None => String::new(),
                    },
                ])
            }
            ToolKind::HttpLatency => {
                let r = api
                    .call(RpcRequest::RunHttpLatency { url: target, nic_id: nic })
                    .await?;
                let s = match r {
                    RpcResponse::HttpLatencyResult(s) => s,
                    other => return Err(anyhow!("unexpected reply: {other:?}")),
                };
                let ms = |v: Option<f64>| {
                    v.map(|v| format!("{v:>8.1} ms"))
                        .unwrap_or_else(|| "     n/a".into())
                };
                Ok(vec![
                    format!("url         {}", s.url),
                    format!(
                        "status      {}",
                        s.status_code
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| "n/a".into())
                    ),
                    format!("dns         {}", ms(s.dns_ms)),
                    format!("connect     {}", ms(s.connect_ms)),
                    format!("tls         {}", ms(s.tls_ms)),
                    format!("ttfb        {}", ms(s.ttfb_ms)),
                    format!("total       {}", ms(s.total_ms)),
                    match s.error {
                        Some(e) => format!("error       {e}"),
                        None => String::new(),
                    },
                ])
            }
            ToolKind::Traceroute => {
                let r = api
                    .call(RpcRequest::RunTraceroute {
                        target,
                        nic_id: nic,
                        max_hops: None,
                    })
                    .await?;
                let res = match r {
                    RpcResponse::Traceroute(res) => res,
                    other => return Err(anyhow!("unexpected reply: {other:?}")),
                };
                let mut lines = vec![format!("path to {}", res.target)];
                for h in res.hops {
                    let host = h
                        .hostname
                        .as_deref()
                        .or(h.address.as_deref())
                        .unwrap_or("*")
                        .to_string();
                    lines.push(format!(
                        "  {:>2}. {:<40} {}",
                        h.hop,
                        host,
                        h.rtt_ms
                            .map(|v| format!("{v:.1} ms"))
                            .unwrap_or_else(|| "*".into())
                    ));
                }
                Ok(lines)
            }
            ToolKind::Nslookup => {
                let r = api.call(RpcRequest::RunNslookup { query: target }).await?;
                let res = match r {
                    RpcResponse::Nslookup(res) => res,
                    other => return Err(anyhow!("unexpected reply: {other:?}")),
                };
                let mut lines = vec![format!("query: {}", res.query)];
                if res.records.is_empty() {
                    lines.push("  (no records)".into());
                }
                for rec in res.records {
                    lines.push(format!("  {rec}"));
                }
                Ok(lines)
            }
            ToolKind::Speedtest => {
                let r = api
                    .call(RpcRequest::RunSpeedtest {
                        nic_id: nic,
                        server_id: None,
                        accept_eula,
                    })
                    .await?;
                let s = match r {
                    RpcResponse::Speedtest(s) => s,
                    other => return Err(anyhow!("unexpected reply: {other:?}")),
                };
                Ok(vec![
                    format!("server      {}", s.server_name.unwrap_or_else(|| "auto".into())),
                    format!("download    {:.1} Mbps", s.download_mbps),
                    format!("upload      {:.1} Mbps", s.upload_mbps),
                    format!("ping        {:.1} ms", s.ping_ms),
                    format!(
                        "jitter      {}",
                        s.jitter_ms
                            .map(|v| format!("{v:.1} ms"))
                            .unwrap_or_else(|| "n/a".into())
                    ),
                    format!(
                        "loss        {}",
                        s.packet_loss
                            .map(|v| format!("{:.1} %", v * 100.0))
                            .unwrap_or_else(|| "n/a".into())
                    ),
                ])
            }
    };

    res
}

async fn emit_result(res: Result<Vec<String>>, tx: &mpsc::Sender<Ev>) {
    match res {
        Ok(lines) => {
            for l in lines {
                if tx.send(Ev::Tool(ToolEvent::Line(l))).await.is_err() {
                    return;
                }
            }
            let _ = tx.send(Ev::Tool(ToolEvent::Finished)).await;
        }
        Err(e) => {
            let _ = tx.send(Ev::Tool(ToolEvent::Failed(e.to_string()))).await;
        }
    }
}

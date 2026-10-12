//! Overview: live gauges, NIC summary, and a bandwidth sparkline.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Sparkline;
use ratatui::Frame;

use crate::theme;
use crate::ui::widgets;
use crate::ui::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let accent = theme::accent(app.view.index());
    let rows = Layout::vertical([Constraint::Min(8), Constraint::Length(7)]).split(area);
    let top = Layout::horizontal([
        Constraint::Percentage(42),
        Constraint::Percentage(58),
    ])
    .split(rows[0]);

    gauges(f, app, top[0], accent);
    nic_summary(f, app, top[1], accent);
    bandwidth_spark(f, app, rows[1], accent);
}

fn gauges(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(3),
    ])
    .split(area);

    let cpu = app.snap.cpu.as_ref();
    let cpu_pct = cpu.map(|c| c.utilization).unwrap_or(0.0);
    let cpu_label = cpu
        .map(|c| format!("{:.0}%  {}", c.utilization, short_brand(&c.brand)))
        .unwrap_or_else(|| "n/a".into());
    f.render_widget(
        widgets::gauge(cpu_pct, theme::CYAN, cpu_label),
        rows[0],
    );

    let mem = app.snap.memory.as_ref();
    let mem_pct = mem.map(|m| m.utilization * 100.0).unwrap_or(0.0);
    let mem_label = mem
        .map(|m| {
            format!(
                "{:.0}%  {} / {}",
                m.utilization * 100.0,
                widgets::human_bytes(m.used_bytes as f64),
                widgets::human_bytes(m.total_bytes as f64)
            )
        })
        .unwrap_or_else(|| "n/a".into());
    f.render_widget(
        widgets::gauge(mem_pct, theme::MAGENTA, mem_label),
        rows[1],
    );

    let (disk_pct, disk_label) = if app.snap.disks.is_empty() {
        (0.0, "no disks reported".to_string())
    } else {
        let avg: f64 =
            app.snap.disks.iter().map(|d| d.utilization * 100.0).sum::<f64>()
                / app.snap.disks.len() as f64;
        let biggest = app
            .snap
            .disks
            .iter()
            .max_by(|a, b| {
                a.utilization
                    .partial_cmp(&b.utilization)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|d| d.name.clone())
            .unwrap_or_default();
        (avg, format!("{avg:.0}%  {biggest}"))
    };
    f.render_widget(
        widgets::gauge(disk_pct, theme::AMBER, disk_label),
        rows[2],
    );

    // Service status strip.
    let status_line = match &app.snap.status {
        Some(s) => Line::from(vec![
            Span::styled("service ", Style::default().fg(theme::SLATE)),
            Span::styled(
                if s.running { "running" } else { "stopped" },
                Style::default().fg(if s.running { theme::GREEN } else { theme::ROSE })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ·  capture ", Style::default().fg(theme::SLATE)),
            Span::styled(s.capture_mode.clone(), Style::default().fg(theme::VIOLET)),
            Span::styled("  ·  pipe ", Style::default().fg(theme::SLATE)),
            Span::styled(
                if s.pipe_connected { "linked" } else { "open" },
                Style::default().fg(if s.pipe_connected { theme::SKY } else { theme::AMBER }),
            ),
        ]),
        None => Line::from(Span::styled(
            "service status unavailable",
            Style::default().fg(theme::DIM),
        )),
    };
    f.render_widget(
        ratatui::widgets::Paragraph::new(status_line)
            .block(widgets::panel_sub("Service", "live", accent)),
        rows[3],
    );
}

fn short_brand(brand: &str) -> String {
    brand.split_whitespace().take(3).collect::<Vec<_>>().join(" ")
}

fn nic_summary(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let mut lines: Vec<Line> = Vec::new();
    if app.snap.nics.is_empty() {
        lines.push(Line::from(Span::styled(
            "  no adapters reported",
            Style::default().fg(theme::DIM),
        )));
    }
    for nic in &app.snap.nics {
        let up = nic.oper_status.eq_ignore_ascii_case("up")
            || nic.oper_status.eq_ignore_ascii_case("connected");
        lines.push(Line::from(vec![
            Span::styled("  ● ", Style::default().fg(if up { theme::GREEN } else { theme::DIM })),
            Span::styled(
                truncate(&nic.name, 22),
                Style::default().fg(theme::FG).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}", nic.ipv4_addresses.first().map(|s| s.as_str()).unwrap_or("—")),
                Style::default().fg(theme::SKY),
            ),
            Span::styled(
                format!("   ↓{}", widgets::human_rate(nic.rx_bps)),
                Style::default().fg(theme::CYAN),
            ),
            Span::styled(
                format!("   ↑{}", widgets::human_rate(nic.tx_bps)),
                Style::default().fg(theme::MAGENTA),
            ),
        ]));
    }

    let total_rx: f64 = app.snap.nics.iter().map(|n| n.rx_bps).sum();
    let total_tx: f64 = app.snap.nics.iter().map(|n| n.tx_bps).sum();
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  Σ ", Style::default().fg(theme::AMBER)),
        Span::styled(
            format!("↓ {}", widgets::human_rate(total_rx)),
            Style::default().fg(theme::CYAN).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("    ↑ {}", widgets::human_rate(total_tx)),
            Style::default().fg(theme::MAGENTA).add_modifier(Modifier::BOLD),
        ),
    ]));

    f.render_widget(
        ratatui::widgets::Paragraph::new(lines)
            .block(widgets::panel_sub(
                "Network adapters",
                format!("{} up", app.snap.nics.len()),
                accent,
            )),
        area,
    );
}

fn bandwidth_spark(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let rates = app.snap.bandwidth_rates();
    let (rx, tx): (Vec<f64>, Vec<f64>) = rates.iter().map(|(_, r, t)| (*r, *t)).unzip();
    let rx_now = rx.last().copied().unwrap_or(0.0);
    let tx_now = tx.last().copied().unwrap_or(0.0);

    // Sparkline wants integers: plot kilobits/s.
    let rx_kb: Vec<u64> = rx.iter().map(|v| (*v / 1000.0).round() as u64).collect();
    let tx_kb: Vec<u64> = tx.iter().map(|v| (*v / 1000.0).round() as u64).collect();
    let max = rx_kb
        .iter()
        .chain(tx_kb.iter())
        .copied()
        .max()
        .unwrap_or(1)
        .max(1);

    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .split(area);

    let legend = Line::from(vec![
        Span::styled("  ↓ rx ", Style::default().fg(theme::SLATE)),
        Span::styled(
            widgets::human_rate(rx_now),
            Style::default().fg(theme::CYAN).add_modifier(Modifier::BOLD),
        ),
        Span::styled("     ↑ tx ", Style::default().fg(theme::SLATE)),
        Span::styled(
            widgets::human_rate(tx_now),
            Style::default().fg(theme::MAGENTA).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("     {} samples · today", rates.len()),
            Style::default().fg(theme::DIM),
        ),
    ]);
    f.render_widget(
        ratatui::widgets::Paragraph::new(legend),
        rows[0],
    );

    if rx_kb.is_empty() {
        f.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                "  no bandwidth samples yet today",
                Style::default().fg(theme::DIM),
            )),
            rows[1],
        );
        return;
    }

    let panel = widgets::panel_sub("Throughput", "Kbps · today", accent);
    let inner = panel.inner(rows[1]);
    f.render_widget(panel, rows[1]);

    let block = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(inner);

    f.render_widget(
        ratatui::widgets::Paragraph::new(Line::from(vec![
            Span::styled("  ↓ ", Style::default().fg(theme::CYAN)),
            Span::styled("rx", Style::default().fg(theme::DIM)),
        ])),
        block[0],
    );
    f.render_widget(
        Sparkline::default()
            .data(&rx_kb)
            .max(max)
            .style(Style::default().fg(theme::CYAN)),
        block[1],
    );
    f.render_widget(
        Sparkline::default()
            .data(&tx_kb)
            .max(max)
            .style(Style::default().fg(theme::MAGENTA)),
        block[2],
    );
}

fn truncate(s: &str, len: usize) -> String {
    if s.chars().count() <= len {
        s.to_string()
    } else {
        let cut: String = s.chars().take(len.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

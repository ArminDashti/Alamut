//! System view: CPU, memory, disks, and thermal sensors.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Row, Table};
use ratatui::Frame;

use crate::theme;
use crate::ui::widgets;
use crate::ui::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let accent = theme::accent(app.view.index());
    let rows = Layout::vertical([
        Constraint::Length(9),
        Constraint::Min(6),
    ])
    .split(area);
    let top = Layout::horizontal([
        Constraint::Percentage(52),
        Constraint::Percentage(48),
    ])
    .split(rows[0]);
    let bottom = Layout::horizontal([
        Constraint::Percentage(58),
        Constraint::Percentage(42),
    ])
    .split(rows[1]);

    cpu_panel(f, app, top[0], accent);
    mem_panel(f, app, top[1], accent);
    disk_panel(f, app, bottom[0], accent);
    thermal_panel(f, app, bottom[1], accent);
}

fn cpu_panel(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let block = widgets::panel_sub("CPU", "live", accent);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(inner);

    let Some(cpu) = app.snap.cpu.clone() else {
        f.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                "  no CPU snapshot",
                Style::default().fg(theme::DIM),
            )),
            rows[0],
        );
        return;
    };

    f.render_widget(
        widgets::gauge(
            cpu.utilization * 100.0,
            theme::CYAN,
            format!(
                "{:.0}%  {:.0} MHz",
                cpu.utilization * 100.0,
                cpu.frequency_mhz.unwrap_or(0)
            ),
        ),
        rows[0],
    );

    let info = Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            cpu.brand.trim().to_string(),
            Style::default().fg(theme::FG),
        ),
        Span::styled(
            format!(
                "  ·  {}C / {}T",
                cpu.physical_cores.unwrap_or(cpu.logical_cores),
                cpu.logical_cores
            ),
            Style::default().fg(theme::SLATE),
        ),
    ]);
    f.render_widget(ratatui::widgets::Paragraph::new(info), rows[1]);

    let mut lines: Vec<Line> = Vec::new();
    for (i, core) in cpu.per_core.iter().enumerate() {
        let pct = (core * 100.0).clamp(0.0, 100.0);
        let color = if pct >= 85.0 {
            theme::ROSE
        } else if pct >= 60.0 {
            theme::AMBER
        } else {
            theme::GREEN
        };
        let filled = (pct / 5.0) as usize;
        let bar: String = "█".repeat(filled) + &"░".repeat(20usize.saturating_sub(filled));
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:>2} ", i),
                Style::default().fg(theme::DIM),
            ),
            Span::styled(bar, Style::default().fg(color)),
            Span::styled(
                format!(" {:>3.0}%", pct),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    f.render_widget(ratatui::widgets::Paragraph::new(lines), rows[2]);
}

fn mem_panel(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let block = widgets::panel_sub("Memory", "live", accent);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .split(inner);

    let Some(mem) = app.snap.memory.clone() else {
        f.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                "  no memory snapshot",
                Style::default().fg(theme::DIM),
            )),
            rows[0],
        );
        return;
    };

    let pct = mem.utilization * 100.0;
    let color = if pct >= 85.0 {
        theme::ROSE
    } else if pct >= 60.0 {
        theme::AMBER
    } else {
        theme::MAGENTA
    };
    f.render_widget(
        widgets::gauge(
            pct,
            color,
            format!(
                "{pct:.0}%  {} / {}",
                widgets::human_bytes(mem.used_bytes as f64),
                widgets::human_bytes(mem.total_bytes as f64)
            ),
        ),
        rows[0],
    );

    let lines = vec![
        line_kv("total", widgets::human_bytes(mem.total_bytes as f64), theme::FG),
        line_kv("used", widgets::human_bytes(mem.used_bytes as f64), theme::MAGENTA),
        line_kv(
            "available",
            widgets::human_bytes(mem.available_bytes as f64),
            theme::LIME,
        ),
        line_kv(
            "free",
            widgets::human_bytes((mem.total_bytes - mem.used_bytes) as f64),
            theme::SKY,
        ),
    ];
    f.render_widget(ratatui::widgets::Paragraph::new(lines), rows[1]);
}

fn line_kv(k: &str, v: String, color: ratatui::style::Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {k:<12}"), Style::default().fg(theme::SLATE)),
        Span::styled(v, Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ])
}

fn disk_panel(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let header = Row::new(vec!["mount", "name", "fs", "kind", "size", "used", "%"])
        .style(Style::default().fg(accent).add_modifier(Modifier::BOLD));

    let mut rows: Vec<Row> = Vec::new();
    for d in &app.snap.disks {
        let pct = (d.utilization * 100.0).clamp(0.0, 100.0);
        let color = if pct >= 90.0 {
            theme::ROSE
        } else if pct >= 75.0 {
            theme::AMBER
        } else {
            theme::LIME
        };
        rows.push(Row::new(vec![
            Cell::from(d.mount_point.clone()).style(Style::default().fg(theme::SKY)),
            Cell::from(d.name.clone()).style(Style::default().fg(theme::FG)),
            Cell::from(d.file_system.clone()).style(Style::default().fg(theme::SLATE)),
            Cell::from(d.kind.as_str().to_uppercase()).style(Style::default().fg(theme::VIOLET)),
            Cell::from(widgets::human_bytes(d.total_bytes as f64)),
            Cell::from(widgets::human_bytes(d.used_bytes as f64)),
            Cell::from(format!("{pct:.0}")).style(
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ]));
    }
    if rows.is_empty() {
        rows.push(Row::new(vec![Cell::from("  no disks reported").style(
            Style::default().fg(theme::DIM),
        )]));
    }

    let widths = [
        Constraint::Length(10),
        Constraint::Percentage(24),
        Constraint::Length(7),
        Constraint::Length(6),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(5),
    ];
    f.render_widget(
        Table::new(rows, widths)
            .header(header)
            .block(widgets::panel_sub(
                "Disks",
                format!("{} volume(s)", app.snap.disks.len()),
                accent,
            )),
        area,
    );
}

fn thermal_panel(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let mut lines: Vec<Line> = Vec::new();
    let sensors = app.snap.thermal.as_ref().map(|t| &t.sensors);
    match sensors {
        None => lines.push(Line::from(Span::styled(
            "  thermal data unavailable",
            Style::default().fg(theme::DIM),
        ))),
        Some(list) if list.is_empty() => lines.push(Line::from(Span::styled(
            "  no thermal sensors reported",
            Style::default().fg(theme::DIM),
        ))),
        Some(list) => {
            for s in list.iter().take(20) {
                let (temp, color) = match s.celsius {
                    Some(c) => {
                        let color = if c >= 85.0 {
                            theme::ROSE
                        } else if c >= 70.0 {
                            theme::AMBER
                        } else {
                            theme::GREEN
                        };
                        (format!("{c:>5.1} °C"), color)
                    }
                    None => ("   n/a".to_string(), theme::DIM),
                };
                lines.push(Line::from(vec![
                    Span::styled(temp, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        truncate(&s.hardware_name, 14),
                        Style::default().fg(theme::SLATE),
                    ),
                    Span::styled(" · ", Style::default().fg(theme::DIM)),
                    Span::styled(s.sensor_name.clone(), Style::default().fg(theme::FG)),
                ]));
            }
        }
    }

    f.render_widget(
        ratatui::widgets::Paragraph::new(lines).block(widgets::panel_sub(
            "Thermals",
            app.snap
                .thermal
                .as_ref()
                .map(|t| format!("{} sensor(s)", t.sensors.len()))
                .unwrap_or_else(|| "n/a".into()),
            accent,
        )),
        area,
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

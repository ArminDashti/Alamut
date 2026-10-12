//! Latency view: ping RTT chart, HTTP latency chart, and link events.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::symbols::Marker;
use ratatui::widgets::{Axis, Chart, Dataset, GraphType, Row, Table};
use ratatui::Frame;

use crate::theme;
use crate::ui::widgets;
use crate::ui::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let accent = theme::accent(app.view.index());
    let rows = Layout::vertical([
        Constraint::Percentage(55),
        Constraint::Percentage(45),
    ])
    .split(area);
    let cols = Layout::horizontal([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(rows[0]);

    ping_chart(f, app, cols[0], accent);
    http_chart(f, app, cols[1], accent);
    link_events(f, app, rows[1], accent);
}

fn latency_chart(
    f: &mut Frame,
    area: Rect,
    title: &'static str,
    color: ratatui::style::Color,
    accent: ratatui::style::Color,
    points: Vec<(i64, f64)>,
    unit: &str,
    stats: Line<'static>,
) {
    let block_title = format!(
        "{title} · {} pts",
        points.len()
    );

    if points.len() < 2 {
        f.render_widget(
            ratatui::widgets::Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  no {unit} samples yet today"),
                    Style::default().fg(theme::DIM),
                )),
            ])
            .block(widgets::panel_sub(title, "today", accent)),
            area,
        );
        return;
    }

    let x0 = points[0].0 as f64;
    let x1 = points[points.len() - 1].0 as f64;
    let x1 = if x1 > x0 { x1 } else { x0 + 1.0 };
    let peak = points
        .iter()
        .map(|(_, v)| *v)
        .fold(1.0_f64, f64::max)
        * 1.2;
    let data: Vec<(f64, f64)> = points.iter().map(|(t, v)| (*t as f64, *v)).collect();

    let chart = Chart::new(vec![Dataset::default()
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(color))
        .data(&data)])
    .block(widgets::panel_sub(block_title, "today", accent))
    .x_axis(
        Axis::default()
            .style(Style::default().fg(theme::DIM))
            .labels(vec![
                Span::styled(widgets::hhmm(points[0].0), Style::default().fg(theme::SLATE)),
                Span::styled(
                    widgets::hhmm(points[points.len() - 1].0),
                    Style::default().fg(theme::SLATE),
                ),
            ])
            .bounds([x0, x1]),
    )
    .y_axis(
        Axis::default()
            .style(Style::default().fg(theme::DIM))
            .labels(vec![
                Span::styled("0", Style::default().fg(theme::SLATE)),
                Span::styled(
                    format!("{:.0} {unit}", peak / 2.0),
                    Style::default().fg(theme::SLATE),
                ),
                Span::styled(
                    format!("{peak:.0} {unit}"),
                    Style::default().fg(theme::SLATE),
                ),
            ])
            .bounds([0.0, peak]),
    );
    f.render_widget(chart, area);
    let _ = stats;
}

fn ping_chart(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let mut points: Vec<(i64, f64)> = app
        .snap
        .ping
        .iter()
        .filter(|p| p.success)
        .filter_map(|p| p.rtt_ms.map(|v| (p.ts, v)))
        .collect();
    points.sort_by_key(|(t, _)| *t);

    let ok = app.snap.ping.iter().filter(|p| p.success).count();
    let fail = app.snap.ping.len() - ok;
    let avg = if points.is_empty() {
        0.0
    } else {
        points.iter().map(|(_, v)| *v).sum::<f64>() / points.len() as f64
    };
    let stats = Line::from(vec![
        Span::styled("avg ", Style::default().fg(theme::SLATE)),
        Span::styled(
            format!("{avg:.1} ms"),
            Style::default().fg(theme::LIME).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  ·  {fail} failed"),
            Style::default().fg(if fail > 0 { theme::ROSE } else { theme::DIM }),
        ),
    ]);
    latency_chart(f, area, "Ping RTT", theme::LIME, accent, points, "ms", stats);
}

fn http_chart(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let mut points: Vec<(i64, f64)> = app
        .snap
        .http
        .iter()
        .filter(|h| h.success)
        .filter_map(|h| h.total_ms.map(|v| (h.ts, v)))
        .collect();
    points.sort_by_key(|(t, _)| *t);

    let fails = app.snap.http.iter().filter(|h| !h.success).count();
    let avg = if points.is_empty() {
        0.0
    } else {
        points.iter().map(|(_, v)| *v).sum::<f64>() / points.len() as f64
    };
    let stats = Line::from(vec![
        Span::styled("avg ", Style::default().fg(theme::SLATE)),
        Span::styled(
            format!("{avg:.1} ms"),
            Style::default().fg(theme::AMBER).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  ·  {fails} failed"),
            Style::default().fg(if fails > 0 { theme::ROSE } else { theme::DIM }),
        ),
    ]);
    latency_chart(
        f,
        area,
        "HTTP total",
        theme::AMBER,
        accent,
        points,
        "ms",
        stats,
    );
}

fn link_events(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let header = Row::new(vec!["time", "event", "adapter", "detail"])
        .style(Style::default().fg(accent).add_modifier(Modifier::BOLD));

    let mut rows: Vec<Row> = Vec::new();
    for ev in app.snap.link_events.iter().rev().take(50) {
        let color = match ev.event.to_lowercase().as_str() {
            e if e.contains("up") || e.contains("connect") => theme::GREEN,
            e if e.contains("down") || e.contains("disconnect") => theme::ROSE,
            _ => theme::AMBER,
        };
        rows.push(Row::new(vec![
            Span::styled(widgets::hhmmss(ev.ts), Style::default().fg(theme::SLATE)),
            Span::styled(ev.event.clone(), Style::default().fg(color)),
            Span::styled(ev.nic_id.clone(), Style::default().fg(theme::DIM)),
            Span::styled(
                ev.detail.clone().unwrap_or_else(|| "—".into()),
                Style::default().fg(theme::FG),
            ),
        ]));
    }
    if rows.is_empty() {
        rows.push(Row::new(vec![Span::styled(
            "  no link events today",
            Style::default().fg(theme::DIM),
        )]));
    }

    let widths = [
        Constraint::Length(9),
        Constraint::Length(18),
        Constraint::Length(14),
        Constraint::Min(20),
    ];
    f.render_widget(
        Table::new(rows, widths)
            .header(header)
            .block(widgets::panel_sub(
                "Link events",
                format!("{} today", app.snap.link_events.len()),
                accent,
            )),
        area,
    );
}

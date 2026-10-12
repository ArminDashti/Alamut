//! Small rendering helpers shared by all views.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Gauge};

use crate::theme;

/// Split `area` into a centered sub-rectangle.
pub fn centered(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vert = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vert[1])[1]
}

/// Rounded, accent-colored panel with a title.
pub fn panel(title: impl Into<String>, color: ratatui::style::Color) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .title(Span::styled(
            format!(" {} ", title.into()),
            Style::default()
                .fg(color)
                .add_modifier(Modifier::BOLD),
        ))
}

/// Panel whose title carries a leading symbol and a dimmed subtitle.
pub fn panel_sub(
    title: impl Into<String>,
    sub: impl Into<String>,
    color: ratatui::style::Color,
) -> Block<'static> {
    panel(title, color).title_bottom(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(sub.into(), Style::default().fg(theme::DIM)),
    ]))
}

/// Horizontal progress bar with an accent color and a formatted label.
pub fn gauge(percent: f64, color: ratatui::style::Color, label: String) -> Gauge<'static> {
    Gauge::default()
        .gauge_style(Style::default().fg(color).bg(theme::HEADER_BG))
        .percent(percent.clamp(0.0, 100.0) as u16)
        .label(Span::styled(
            label,
            Style::default().fg(theme::FG).add_modifier(Modifier::BOLD),
        ))
}

/// `1.23 GB` style byte formatting.
pub fn human_bytes(bytes: f64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut v = bytes.max(0.0);
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{v:.0} {}", UNITS[i])
    } else {
        format!("{v:.2} {}", UNITS[i])
    }
}

/// `12.4 Mbps` / `812 Kbps` rate formatting (input is bits per second).
pub fn human_rate(bps: f64) -> String {
    if bps >= 1_000_000_000.0 {
        format!("{:.2} Gbps", bps / 1_000_000_000.0)
    } else if bps >= 1_000_000.0 {
        format!("{:.1} Mbps", bps / 1_000_000.0)
    } else if bps >= 1_000.0 {
        format!("{:.0} Kbps", bps / 1_000.0)
    } else {
        format!("{:.0} bps", bps)
    }
}

/// Local `HH:MM` label for a unix timestamp.
pub fn hhmm(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_default()
}

/// Local `HH:MM:SS` label for a unix timestamp.
pub fn hhmmss(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|d| d.with_timezone(&chrono::Local).format("%H:%M:%S").to_string())
        .unwrap_or_default()
}

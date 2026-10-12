//! Apps view: top processes by traffic today.

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
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Min(6)]).split(area);

    let mut entries: Vec<(String, u64, u64)> = app
        .snap
        .usage
        .iter()
        .map(|u| (u.process_name.clone(), u.bytes_in, u.bytes_out))
        .collect();
    entries.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)));

    let total_in: u64 = entries.iter().map(|e| e.1).sum();
    let total_out: u64 = entries.iter().map(|e| e.2).sum();
    let total = (total_in + total_out) as f64;

    let summary = Line::from(vec![
        Span::styled("  today  ", Style::default().fg(theme::SLATE)),
        Span::styled(
            format!("↓ {}", widgets::human_bytes(total_in as f64)),
            Style::default().fg(theme::CYAN).add_modifier(Modifier::BOLD),
        ),
        Span::styled("     ", Style::default()),
        Span::styled(
            format!("↑ {}", widgets::human_bytes(total_out as f64)),
            Style::default().fg(theme::MAGENTA).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("     {} process(es)", entries.len()),
            Style::default().fg(theme::DIM),
        ),
    ]);
    f.render_widget(ratatui::widgets::Paragraph::new(summary), rows[0]);

    let header = Row::new(vec!["process", "share", "↓ in", "↑ out", "total"])
        .style(Style::default().fg(accent).add_modifier(Modifier::BOLD));

    let mut table_rows: Vec<Row> = Vec::new();
    for (name, bytes_in, bytes_out) in entries.iter().take(40) {
        let sum = (bytes_in + bytes_out) as f64;
        let share = if total > 0.0 { sum / total } else { 0.0 };
        let filled = (share * 20.0).round() as usize;
        let bar: String = "━".repeat(filled) + &"·".repeat(20usize.saturating_sub(filled));
        let color = if share >= 0.4 {
            theme::ROSE
        } else if share >= 0.15 {
            theme::AMBER
        } else {
            theme::SKY
        };
        table_rows.push(Row::new(vec![
            Cell::from(name.clone()).style(
                Style::default().fg(theme::FG).add_modifier(Modifier::BOLD),
            ),
            Cell::from(format!("{bar} {:>4.1}%", share * 100.0))
                .style(Style::default().fg(color)),
            Cell::from(widgets::human_bytes(*bytes_in as f64))
                .style(Style::default().fg(theme::CYAN)),
            Cell::from(widgets::human_bytes(*bytes_out as f64))
                .style(Style::default().fg(theme::MAGENTA)),
            Cell::from(widgets::human_bytes(sum)).style(Style::default().fg(theme::FG)),
        ]));
    }
    if table_rows.is_empty() {
        table_rows.push(Row::new(vec![Cell::from(
            "  no per-process usage recorded today",
        )
        .style(Style::default().fg(theme::DIM))]));
    }

    let widths = [
        Constraint::Percentage(30),
        Constraint::Length(26),
        Constraint::Length(11),
        Constraint::Length(11),
        Constraint::Length(11),
    ];
    f.render_widget(
        Table::new(table_rows, widths)
            .header(header)
            .block(widgets::panel_sub(
                "Traffic by process",
                "grouped by process · today",
                accent,
            )),
        rows[1],
    );
}

//! Help overlay rendered on top of any view.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Clear;
use ratatui::Frame;

use crate::state::View;
use crate::theme;
use crate::ui::widgets;

pub fn overlay(f: &mut Frame, area: Rect) {
    let pop = widgets::centered(area, 70, 78);
    f.render_widget(Clear, pop);

    let accent = theme::MAGENTA;
    let key = |k: &str, label: &str| {
        Line::from(vec![
            Span::styled(
                format!("  {k:<10}"),
                Style::default()
                    .fg(Color::Black)
                    .bg(theme::SLATE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {label}"), Style::default().fg(theme::FG)),
        ])
    };
    let head = |label: &str, color: Color| {
        Line::from(Span::styled(
            format!("  {label}"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ))
    };

    let mut lines: Vec<Line> = vec![
        Line::from(""),
        head("Navigation", accent),
        key("1 … 7", "jump straight to a tab"),
        key("Tab", "next tab"),
        key("Shift+Tab", "previous tab"),
        Line::from(""),
        head("Global", theme::CYAN),
        key("r", "refresh the current view now"),
        key("?", "show / hide this help"),
        key("q", "quit (also Esc, or Ctrl+C)"),
        Line::from(""),
        head("NICs / Bandwidth / Latency", theme::LIME),
        key("↑ / ↓", "move the NIC cursor (row 1 = all NICs)"),
        key("Enter", "apply the NIC filter to history queries"),
        Line::from(""),
        head("Tools", theme::AMBER),
        key("← / →", "switch tool (ping, http, trace, dns, speed)"),
        key("i / Enter", "focus the target input"),
        key("r", "run the tool"),
        key("a", "accept the Ookla speedtest EULA"),
        key("c", "clear the output"),
        key("↑ / ↓", "scroll the output"),
        Line::from(""),
    ];

    let mut tab_spans = vec![Span::styled(
        "  Tabs:  ",
        Style::default().fg(theme::DIM),
    )];
    for v in View::ALL {
        tab_spans.push(Span::styled(
            format!("{} ", v.index() + 1),
            Style::default().fg(theme::DIM),
        ));
        tab_spans.push(Span::styled(
            v.title().to_string(),
            Style::default().fg(theme::accent(v.index())),
        ));
        tab_spans.push(Span::styled("    ", Style::default()));
    }
    lines.push(Line::from(tab_spans));
    lines.push(Line::from(""));

    let block = widgets::panel_sub(
        "Help",
        format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
        accent,
    );

    f.render_widget(
        ratatui::widgets::Paragraph::new(lines)
            .block(block)
            .style(Style::default().bg(theme::HEADER_BG)),
        pop,
    );
}

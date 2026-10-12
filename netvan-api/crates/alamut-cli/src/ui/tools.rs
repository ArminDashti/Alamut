//! Tools view: pick a network tool, type a target, run it, read the output.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, List, ListItem, Row, Table};
use ratatui::Frame;

use crate::state::ToolKind;
use crate::theme;
use crate::ui::widgets;
use crate::ui::App;

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let accent = theme::accent(app.view.index());
    let cols = Layout::horizontal([
        Constraint::Percentage(36),
        Constraint::Percentage(64),
    ])
    .split(area);

    let rows = Layout::vertical([
        Constraint::Min(8),
        Constraint::Length(3),
        Constraint::Length(7),
    ])
    .split(cols[0]);

    tool_picker(f, app, rows[0], accent);
    target_input(f, app, rows[1], accent);
    hints(f, app, rows[2], accent);

    if matches!(app.tool, ToolKind::Speedtest) {
        let right = Layout::vertical([Constraint::Min(5), Constraint::Length(7)]).split(cols[1]);
        output(f, app, right[0], accent);
        speedtest_history(f, app, right[1], accent);
    } else {
        output(f, app, cols[1], accent);
    }
}

fn tool_picker(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let items: Vec<ListItem> = ToolKind::ALL
        .iter()
        .map(|t| {
            let selected = *t == app.tool;
            ListItem::new(Line::from(vec![
                Span::styled(
                    if selected { "  ◉ " } else { "  ○ " },
                    Style::default().fg(if selected { accent } else { theme::DIM }),
                ),
                Span::styled(
                    t.title().to_string(),
                    Style::default()
                        .fg(if selected { theme::FG } else { theme::SLATE })
                        .add_modifier(if selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::styled(
                    format!("  — {}", t.hint()),
                    Style::default().fg(theme::DIM),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(widgets::panel_sub("Tool", "←/→ switch", accent))
        .highlight_style(
            Style::default()
                .bg(theme::SEL_BG)
                .fg(accent)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(list, area);
}

fn target_input(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let mut spans: Vec<Span> = Vec::new();

    if !app.tool.needs_input() {
        spans.push(Span::styled(
            "  not needed for this tool",
            Style::default().fg(theme::DIM),
        ));
    } else if app.input.is_empty() && !app.input_active {
        spans.push(Span::styled(
            format!("  {} ", app.tool.placeholder()),
            Style::default().fg(theme::DIM),
        ));
    } else {
        let (before, at, after) = split_at(&app.input, app.cursor);
        spans.push(Span::styled(
            format!("  {before}"),
            Style::default().fg(theme::FG),
        ));
        spans.push(Span::styled(
            at.unwrap_or(' ').to_string(),
            Style::default()
                .fg(Color::Black)
                .bg(if app.input_active { accent } else { theme::SLATE })
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(after.to_string(), Style::default().fg(theme::FG)));
    }

    let title = if app.input_active {
        "Target  ·  Enter run · Esc cancel"
    } else {
        "Target  ·  i or Enter to type"
    };
    let block = widgets::panel(title, accent).border_style(Style::default().fg(if app
        .input_active
    {
        accent
    } else {
        theme::HEADER_BG
    }));

    f.render_widget(
        ratatui::widgets::Paragraph::new(Line::from(spans)).block(block),
        area,
    );
}

fn split_at(s: &str, idx: usize) -> (&str, Option<char>, &str) {
    let mut start = idx.min(s.len());
    while start > 0 && !s.is_char_boundary(start) {
        start -= 1;
    }
    let before = &s[..start];
    let rest = &s[start..];
    match rest.chars().next() {
        Some(c) => (before, Some(c), &rest[c.len_utf8()..]),
        None => (before, None, ""),
    }
}

fn hints(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let mut lines: Vec<Line> = vec![Line::from(Span::styled(
        format!("  {}", app.tool.hint()),
        Style::default().fg(theme::SLATE),
    ))];

    let eula_ok = app.snap.eula_accepted.unwrap_or(false) || app.eula_ack;
    if matches!(app.tool, ToolKind::Speedtest) {
        if eula_ok {
            lines.push(Line::from(Span::styled(
                "  Ookla EULA: accepted ✓",
                Style::default().fg(theme::LIME),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "  Ookla EULA: not accepted — press [a]",
                Style::default()
                    .fg(theme::AMBER)
                    .add_modifier(Modifier::BOLD),
            )));
        }
    }

    lines.push(Line::from(Span::styled(
        "  runs on the API host; results stream back here",
        Style::default().fg(theme::DIM),
    )));
    lines.push(Line::from(Span::styled(
        "  the NIC filter chosen on other tabs applies here too",
        Style::default().fg(theme::DIM),
    )));

    f.render_widget(
        ratatui::widgets::Paragraph::new(lines)
            .block(widgets::panel_sub("Notes", "", accent)),
        area,
    );
}

fn output(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let spin = SPINNER[app.frame as usize % SPINNER.len()];
    let status = app
        .tool_status
        .clone()
        .unwrap_or_else(|| "idle — press i, type a target, press Enter".into());

    let status_spans = if app.tool_running {
        vec![
            Span::styled(format!("{spin} "), Style::default().fg(accent)),
            Span::styled(status, Style::default().fg(accent)),
        ]
    } else {
        vec![
            Span::styled("● ", Style::default().fg(theme::GREEN)),
            Span::styled(status, Style::default().fg(theme::SLATE)),
        ]
    };

    let block = widgets::panel_sub("Output", "", accent).title_bottom(Line::from({
        let mut spans = vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("{} lines", app.tool_lines.len()),
                Style::default().fg(theme::DIM),
            ),
            Span::styled("   ", Style::default()),
        ];
        spans.extend(status_spans);
        spans
    }));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let visible = inner.height as usize;
    let total = app.tool_lines.len();
    let scroll = app.tool_scroll.min(total.saturating_sub(1));
    let start = total.saturating_sub(visible + scroll);
    let end = (start + visible).min(total);

    if total == 0 {
        f.render_widget(
            ratatui::widgets::Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  No output yet.",
                    Style::default().fg(theme::SLATE),
                )),
                Line::from(Span::styled(
                    "  Pick a tool on the left, type a target, press Enter to run.",
                    Style::default().fg(theme::DIM),
                )),
            ]),
            inner,
        );
        return;
    }

    let lines: Vec<Line> = app.tool_lines[start..end]
        .iter()
        .map(|l| {
            let color = if l.starts_with('✖') {
                theme::ROSE
            } else if l.starts_with('▶') {
                accent
            } else if l.starts_with('—') {
                theme::DIM
            } else {
                theme::FG
            };
            Line::from(Span::styled(format!("  {l}"), Style::default().fg(color)))
        })
        .collect();

    f.render_widget(ratatui::widgets::Paragraph::new(lines), inner);
}

fn speedtest_history(f: &mut Frame, app: &App, area: Rect, accent: ratatui::style::Color) {
    let header = Row::new(vec!["time", "server", "↓ down", "↑ up", "ping", "jitter"])
        .style(Style::default().fg(accent).add_modifier(Modifier::BOLD));

    let mut rows: Vec<Row> = Vec::new();
    for s in app.snap.speedtests.iter().rev().take(10) {
        rows.push(Row::new(vec![
            Cell::from(widgets::hhmm(s.ts)).style(Style::default().fg(theme::SLATE)),
            Cell::from(s.server_name.clone().unwrap_or_else(|| "auto".into()))
                .style(Style::default().fg(theme::FG)),
            Cell::from(format!("{:.1} Mbps", s.download_mbps))
                .style(Style::default().fg(theme::CYAN)),
            Cell::from(format!("{:.1} Mbps", s.upload_mbps))
                .style(Style::default().fg(theme::MAGENTA)),
            Cell::from(format!("{:.1} ms", s.ping_ms)).style(Style::default().fg(theme::LIME)),
            Cell::from(
                s.jitter_ms
                    .map(|v| format!("{v:.1} ms"))
                    .unwrap_or_else(|| "—".into()),
            )
            .style(Style::default().fg(theme::SLATE)),
        ]));
    }
    if rows.is_empty() {
        rows.push(Row::new(vec![Cell::from("  no speedtests yet").style(
            Style::default().fg(theme::DIM),
        )]));
    }

    let widths = [
        Constraint::Length(7),
        Constraint::Percentage(30),
        Constraint::Length(13),
        Constraint::Length(13),
        Constraint::Length(10),
        Constraint::Length(10),
    ];
    f.render_widget(
        Table::new(rows, widths)
            .header(header)
            .block(widgets::panel_sub(
                "Speedtest history",
                format!("{} today", app.snap.speedtests.len()),
                accent,
            )),
        area,
    );
}

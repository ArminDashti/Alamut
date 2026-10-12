//! NIC list: adapters table with link details and per-NIC rates.

use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Row, Table, TableState};
use ratatui::Frame;

use crate::theme;
use crate::ui::widgets;
use crate::ui::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let accent = theme::accent(app.view.index());

    let header = Row::new(vec![
        "status", "adapter", "ipv4", "link", "wi-fi", "rssi", "↓ rx", "↑ tx", "mac",
    ])
    .style(Style::default().fg(accent).add_modifier(Modifier::BOLD));

    let mut rows: Vec<Row> = Vec::new();

    // Row 0 — pseudo "all adapters" entry (matches nic_sel == 0).
    rows.push(
        Row::new(vec![
            Cell::from(Line::from(vec![
                Span::styled("● ", Style::default().fg(theme::AMBER)),
                Span::styled("all", Style::default().fg(theme::AMBER)),
            ])),
            Cell::from("All adapters").style(
                Style::default().fg(theme::FG).add_modifier(Modifier::BOLD),
            ),
            Cell::from("aggregate").style(Style::default().fg(theme::DIM)),
            Cell::from("—"),
            Cell::from("—"),
            Cell::from("—"),
            Cell::from(widgets::human_rate(
                app.snap.nics.iter().map(|n| n.rx_bps).sum(),
            ))
            .style(Style::default().fg(theme::CYAN)),
            Cell::from(widgets::human_rate(
                app.snap.nics.iter().map(|n| n.tx_bps).sum(),
            ))
            .style(Style::default().fg(theme::MAGENTA)),
            Cell::from("—"),
        ])
        .style(Style::default().bg(theme::HEADER_BG)),
    );

    for nic in &app.snap.nics {
        let up = nic.oper_status.eq_ignore_ascii_case("up")
            || nic.oper_status.eq_ignore_ascii_case("connected");
        let status = Cell::from(Line::from(vec![
            Span::styled(
                "● ",
                Style::default().fg(if up { theme::GREEN } else { theme::ROSE }),
            ),
            Span::styled(
                if up { "up" } else { "down" },
                Style::default().fg(if up { theme::GREEN } else { theme::ROSE }),
            ),
        ]));
        let link = nic
            .link_speed_bps
            .map(|s| widgets::human_rate(s as f64))
            .unwrap_or_else(|| "—".into());
        let wifi = nic
            .wifi_ssid
            .as_deref()
            .map(|s| truncate(s, 14))
            .unwrap_or_else(|| "—".into());
        let rssi = nic
            .wifi_signal
            .map(|dbm| format!("{dbm} dBm"))
            .unwrap_or_else(|| "—".into());
        let rssi_color = match nic.wifi_signal {
            Some(v) if v >= -55 => theme::GREEN,
            Some(v) if v >= -70 => theme::AMBER,
            Some(_) => theme::ROSE,
            None => theme::DIM,
        };

        rows.push(Row::new(vec![
            status,
            Cell::from(nic.name.clone())
                .style(Style::default().fg(theme::FG).add_modifier(Modifier::BOLD)),
            Cell::from(
                nic.ipv4_addresses
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "—".into()),
            )
            .style(Style::default().fg(theme::SKY)),
            Cell::from(link),
            Cell::from(wifi).style(Style::default().fg(theme::VIOLET)),
            Cell::from(rssi).style(Style::default().fg(rssi_color)),
            Cell::from(widgets::human_rate(nic.rx_bps)).style(Style::default().fg(theme::CYAN)),
            Cell::from(widgets::human_rate(nic.tx_bps)).style(Style::default().fg(theme::MAGENTA)),
            Cell::from(nic.mac.clone().unwrap_or_else(|| "—".into()))
                .style(Style::default().fg(theme::DIM)),
        ]));
    }

    let widths = [
        Constraint::Length(8),
        Constraint::Percentage(22),
        Constraint::Percentage(14),
        Constraint::Length(11),
        Constraint::Percentage(13),
        Constraint::Length(10),
        Constraint::Length(11),
        Constraint::Length(11),
        Constraint::Length(18),
    ];

    let filter_note = Line::from(vec![
        Span::styled("  filter ", Style::default().fg(theme::SLATE)),
        Span::styled(
            selected_label(app),
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  ·  ↑/↓ pick · Enter apply · r refresh",
            Style::default().fg(theme::DIM),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(
            format!(" Network adapters  {} ", app.snap.nics.len()),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(filter_note);

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(
            Style::default()
                .bg(theme::SEL_BG)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ")
        .block(block);

    let mut state = TableState::default().with_selected(Some(app.nic_sel));
    f.render_stateful_widget(table, area, &mut state);
}

fn selected_label(app: &App) -> String {
    if app.nic_sel == 0 {
        "all NICs".into()
    } else {
        app.snap
            .nics
            .get(app.nic_sel - 1)
            .map(|n| n.name.clone())
            .unwrap_or_else(|| "all NICs".into())
    }
}

fn truncate(s: &str, len: usize) -> String {
    if s.chars().count() <= len {
        s.to_string()
    } else {
        let cut: String = s.chars().take(len.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

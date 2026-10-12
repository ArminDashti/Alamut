//! Bandwidth view: rx/tx line chart over today's samples.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::symbols::Marker;
use ratatui::widgets::{Axis, Chart, Dataset, GraphType};
use ratatui::Frame;

use crate::theme;
use crate::ui::widgets;
use crate::ui::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let accent = theme::accent(app.view.index());
    let rows = Layout::vertical([Constraint::Length(4), Constraint::Min(6)]).split(area);

    let rates = app.snap.bandwidth_rates();
    summary(f, app, rows[0], &rates, accent);

    if rates.len() < 2 {
        f.render_widget(
            ratatui::widgets::Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Not enough samples yet for a chart.",
                    Style::default().fg(theme::SLATE),
                )),
                Line::from(Span::styled(
                    "  The collector writes bandwidth samples every few seconds — this fills in shortly.",
                    Style::default().fg(theme::DIM),
                )),
            ])
            .block(widgets::panel_sub("Throughput", "today", accent)),
            rows[1],
        );
        return;
    }

    let x0 = rates[0].0 as f64;
    let x1 = rates[rates.len() - 1].0 as f64;
    let x1 = if x1 > x0 { x1 } else { x0 + 1.0 };

    let rx_pts: Vec<(f64, f64)> = rates.iter().map(|(t, r, _)| (*t as f64, *r)).collect();
    let tx_pts: Vec<(f64, f64)> = rates.iter().map(|(t, _, v)| (*t as f64, *v)).collect();
    let peak = rx_pts
        .iter()
        .chain(tx_pts.iter())
        .map(|(_, v)| *v)
        .fold(1.0_f64, f64::max)
        * 1.15;

    let datasets = vec![
        Dataset::default()
            .name("rx")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme::CYAN))
            .data(&rx_pts),
        Dataset::default()
            .name("tx")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme::MAGENTA))
            .data(&tx_pts),
    ];

    let chart = Chart::new(datasets)
        .block(widgets::panel_sub(
            "Throughput",
            format!("{} pts · today · {}", rx_pts.len(), app_selected(app)),
            accent,
        ))
        .x_axis(
            Axis::default()
                .style(Style::default().fg(theme::DIM))
                .labels(vec![
                    Span::styled(widgets::hhmm(rates[0].0), Style::default().fg(theme::SLATE)),
                    Span::styled(
                        widgets::hhmm(rates[rates.len() / 2].0),
                        Style::default().fg(theme::SLATE),
                    ),
                    Span::styled(
                        widgets::hhmm(rates[rates.len() - 1].0),
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
                        widgets::human_rate(peak / 2.0),
                        Style::default().fg(theme::SLATE),
                    ),
                    Span::styled(widgets::human_rate(peak), Style::default().fg(theme::SLATE)),
                ])
                .bounds([0.0, peak]),
        );
    f.render_widget(chart, rows[1]);
}

fn summary(
    f: &mut Frame,
    app: &App,
    area: Rect,
    rates: &[(i64, f64, f64)],
    accent: ratatui::style::Color,
) {
    let rx_now = rates.last().map(|(_, r, _)| *r).unwrap_or(0.0);
    let tx_now = rates.last().map(|(_, _, t)| *t).unwrap_or(0.0);
    let rx_peak = rates.iter().map(|(_, r, _)| *r).fold(0.0, f64::max);
    let tx_peak = rates.iter().map(|(_, _, t)| *t).fold(0.0, f64::max);
    let rx_avg = avg(rates.iter().map(|(_, r, _)| *r), rates.len());
    let tx_avg = avg(rates.iter().map(|(_, _, t)| *t), rates.len());

    let cell = |label: &str, value: String, color: ratatui::style::Color| {
        Line::from(vec![
            Span::styled(format!("  {label}  "), Style::default().fg(theme::SLATE)),
            Span::styled(
                value,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
        ])
    };

    let lines = vec![
        cell("now ↓", widgets::human_rate(rx_now), theme::CYAN),
        cell("now ↑", widgets::human_rate(tx_now), theme::MAGENTA),
        cell("avg ↓", widgets::human_rate(rx_avg), theme::SKY),
        cell("avg ↑", widgets::human_rate(tx_avg), theme::VIOLET),
        cell("peak ↓", widgets::human_rate(rx_peak), theme::LIME),
        cell("peak ↑", widgets::human_rate(tx_peak), theme::AMBER),
    ];

    f.render_widget(
        ratatui::widgets::Paragraph::new(lines)
            .block(widgets::panel_sub("Rates", app_selected(app), accent)),
        area,
    );
}

fn avg(vals: impl Iterator<Item = f64>, n: usize) -> f64 {
    if n == 0 {
        return 0.0;
    }
    vals.sum::<f64>() / n as f64
}

fn app_selected(app: &App) -> String {
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

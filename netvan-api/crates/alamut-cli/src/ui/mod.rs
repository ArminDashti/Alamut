//! Terminal app: setup, event loop, key handling, and the shared chrome
//! (header, tab bar, footer) plus dispatch into the per-view renderers.

pub mod apps;
pub mod bandwidth;
pub mod help;
pub mod latency;
pub mod nics;
pub mod overview;
pub mod system;
pub mod tools;
pub mod widgets;

use std::time::Duration;

use anyhow::Result;
use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Tabs};
use ratatui::Frame;
use tokio::sync::mpsc;

use crate::api::ApiClient;
use crate::state::{Cmd, Ev, Snapshot, ToolEvent, ToolKind, View};
use crate::theme;

pub struct App {
    pub view: View,
    /// 0 = all NICs, otherwise index into `snap.nics`.
    pub nic_sel: usize,
    /// NIC id the poller is currently filtered to.
    pub nic_filter: Option<String>,
    pub snap: Snapshot,
    pub tool: ToolKind,
    pub input: String,
    pub cursor: usize,
    pub input_active: bool,
    pub tool_lines: Vec<String>,
    pub tool_status: Option<String>,
    pub tool_running: bool,
    pub tool_scroll: usize,
    /// Local confirmation that the speedtest EULA was accepted this session.
    pub eula_ack: bool,
    pub show_help: bool,
    pub quit: bool,
    frame: u64,
}

impl App {
    fn new() -> Self {
        Self {
            view: View::Overview,
            nic_sel: 0,
            nic_filter: None,
            snap: Snapshot::default(),
            tool: ToolKind::Ping,
            input: String::new(),
            cursor: 0,
            input_active: false,
            tool_lines: Vec::new(),
            tool_status: None,
            tool_running: false,
            tool_scroll: 0,
            eula_ack: false,
            show_help: false,
            quit: false,
            frame: 0,
        }
    }

    fn selected_nic_id(&self) -> Option<String> {
        if self.nic_sel == 0 {
            None
        } else {
            self.snap
                .nics
                .get(self.nic_sel - 1)
                .map(|n| n.id.clone())
        }
    }

    fn selected_nic_label(&self) -> String {
        if self.nic_sel == 0 {
            "all NICs".to_string()
        } else {
            self.snap
                .nics
                .get(self.nic_sel - 1)
                .map(|n| n.name.clone())
                .unwrap_or_else(|| "all NICs".to_string())
        }
    }

    /// Effective tool target: typed text, else the placeholder default.
    fn tool_target(&self) -> String {
        if self.input.trim().is_empty() {
            self.tool.placeholder().to_string()
        } else {
            self.input.trim().to_string()
        }
    }

    fn push_line(&mut self, line: String) {
        self.tool_lines.push(line);
        self.tool_scroll = 0;
    }
}

/// Run the whole TUI until the user quits.
pub async fn run(base: String, poll_ms: u64) -> Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, base, poll_ms).await;
    ratatui::restore();
    result
}

async fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    base: String,
    poll_ms: u64,
) -> Result<()> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>(64);
    let (ev_tx, mut ev_rx) = mpsc::channel::<Ev>(256);

    let api = ApiClient::new(base);
    let poller = tokio::spawn(crate::state::poll_loop(
        api,
        cmd_rx,
        ev_tx,
        Duration::from_millis(poll_ms.max(250)),
    ));
    let _ = cmd_tx.send(Cmd::SetView(View::Overview)).await;

    let mut app = App::new();

    loop {
        while let Ok(ev) = ev_rx.try_recv() {
            apply_event(&mut app, ev);
        }

        terminal.draw(|f| render(f, &mut app))?;
        app.frame = app.frame.wrapping_add(1);

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    handle_key(&mut app, key, &cmd_tx).await?;
                }
                _ => {}
            }
        }

        if app.quit {
            break;
        }
    }

    let _ = cmd_tx.send(Cmd::Quit).await;
    poller.abort();
    Ok(())
}

fn apply_event(app: &mut App, ev: Ev) {
    match ev {
        Ev::Snapshot(s) => app.snap = *s,
        Ev::Tool(te) => match te {
            ToolEvent::Status(s) => {
                app.tool_running = !s.contains("accepted");
                app.tool_status = Some(s);
            }
            ToolEvent::Line(l) => app.push_line(l),
            ToolEvent::Finished => {
                app.tool_running = false;
                app.tool_status = Some("finished".into());
                app.push_line("── finished ──".into());
            }
            ToolEvent::Failed(e) => {
                app.tool_running = false;
                app.tool_status = Some(format!("error: {e}"));
                app.push_line(format!("✖ {e}"));
            }
        },
    }
}

// ---------------------------------------------------------------- key handling

async fn handle_key(app: &mut App, key: KeyEvent, cmd: &mpsc::Sender<Cmd>) -> Result<()> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.quit = true;
        return Ok(());
    }

    if app.show_help {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
            app.show_help = false;
        } else if key.code == KeyCode::Char('q') {
            app.quit = true;
        }
        return Ok(());
    }

    if app.input_active {
        return handle_input_key(app, key, cmd).await;
    }

    match key.code {
        KeyCode::Char('q') => {
            app.quit = true;
            return Ok(());
        }
        KeyCode::Char('?') => {
            app.show_help = true;
            return Ok(());
        }
        KeyCode::Tab => {
            set_view(app, cmd, View::from_index(app.view.index() + 1)).await;
            return Ok(());
        }
        KeyCode::BackTab => {
            set_view(app, cmd, View::from_index(app.view.index() + View::ALL.len() - 1)).await;
            return Ok(());
        }
        KeyCode::Char(c @ '1'..='7') => {
            let idx = c as usize - '1' as usize;
            set_view(app, cmd, View::from_index(idx)).await;
            return Ok(());
        }
        _ => {}
    }

    match app.view {
        View::Nics | View::Bandwidth | View::Latency => match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                app.nic_sel = app.nic_sel.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = app.snap.nics.len();
                if app.nic_sel < max {
                    app.nic_sel += 1;
                }
            }
            KeyCode::Enter => {
                let id = app.selected_nic_id();
                app.nic_filter = id.clone();
                let _ = cmd.send(Cmd::SelectNic(id)).await;
            }
            _ => {}
        },
        View::Tools => handle_tools_key(app, key, cmd).await?,
        _ => {
            if key.code == KeyCode::Char('r') {
                let _ = cmd.send(Cmd::Refresh).await;
            }
        }
    }
    Ok(())
}

async fn set_view(app: &mut App, cmd: &mpsc::Sender<Cmd>, view: View) {
    if app.view != view {
        app.view = view;
        let _ = cmd.send(Cmd::SetView(view)).await;
    }
}

async fn handle_input_key(app: &mut App, key: KeyEvent, cmd: &mpsc::Sender<Cmd>) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.input_active = false;
        }
        KeyCode::Enter => {
            app.input_active = false;
            submit_tool(app, cmd).await?;
        }
        KeyCode::Backspace => {
            if app.cursor > 0 {
                app.cursor -= 1;
                app.input.remove(app.cursor);
            }
        }
        KeyCode::Delete => {
            if app.cursor < app.input.len() {
                app.input.remove(app.cursor);
            }
        }
        KeyCode::Left => app.cursor = app.cursor.saturating_sub(1),
        KeyCode::Right => app.cursor = (app.cursor + 1).min(app.input.len()),
        KeyCode::Home => app.cursor = 0,
        KeyCode::End => app.cursor = app.input.len(),
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.input.len() < 200 {
                app.input.insert(app.cursor, c);
                app.cursor += 1;
            }
        }
        _ => {}
    }
    Ok(())
}

async fn handle_tools_key(app: &mut App, key: KeyEvent, cmd: &mpsc::Sender<Cmd>) -> Result<()> {
    match key.code {
        KeyCode::Char('i') | KeyCode::Enter => {
            if app.tool_running {
                app.tool_status = Some("already running…".into());
            } else {
                app.input_active = true;
                app.cursor = app.input.len();
            }
        }
        KeyCode::Char('r') => submit_tool(app, cmd).await?,
        KeyCode::Char('c') => {
            app.tool_lines.clear();
            app.tool_status = None;
            app.tool_scroll = 0;
        }
        KeyCode::Char('a') => {
            app.eula_ack = true;
            let _ = cmd.send(Cmd::AcceptEula).await;
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.tool = ToolKind::from_index(app.tool.index() + ToolKind::ALL.len() - 1);
            app.input.clear();
            app.cursor = 0;
        }
        KeyCode::Right | KeyCode::Char('l') => {
            app.tool = ToolKind::from_index(app.tool.index() + 1);
            app.input.clear();
            app.cursor = 0;
        }
        KeyCode::Up => app.tool_scroll = app.tool_scroll.saturating_add(1),
        KeyCode::Down => app.tool_scroll = app.tool_scroll.saturating_sub(1),
        KeyCode::PageUp => app.tool_scroll = app.tool_scroll.saturating_add(10),
        KeyCode::PageDown => app.tool_scroll = app.tool_scroll.saturating_sub(10),
        _ => {}
    }
    Ok(())
}

async fn submit_tool(app: &mut App, cmd: &mpsc::Sender<Cmd>) -> Result<()> {
    if app.tool_running {
        app.tool_status = Some("already running…".into());
        return Ok(());
    }
    let accept_eula = app.eula_ack || app.snap.eula_accepted.unwrap_or(false);
    if matches!(app.tool, ToolKind::Speedtest) && !accept_eula {
        app.tool_status = Some("press [a] to accept the Ookla EULA first".into());
        return Ok(());
    }
    let kind = app.tool;
    let target = app.tool_target();
    app.push_line(format!("▶ {} — {target}", kind.title()));
    app.tool_running = true;
    app.tool_status = Some(format!("running {}…", kind.title()));
    let _ = cmd
        .send(Cmd::RunTool {
            kind,
            target,
            accept_eula,
        })
        .await;
    Ok(())
}

// --------------------------------------------------------------------- render

fn render(f: &mut Frame, app: &mut App) {
    let area = f.area();
    if area.width < 64 || area.height < 20 {
        f.render_widget(
            Paragraph::new("Terminal too small — resize to at least 64×20.")
                .style(Style::default().fg(theme::ROSE)),
            area,
        );
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(1),
    ])
    .split(area);

    header(f, app, chunks[0]);
    tabs(f, app, chunks[1]);
    body(f, app, chunks[2]);
    footer(f, app, chunks[3]);

    if app.show_help {
        help::overlay(f, area);
    }
}

fn header(f: &mut Frame, app: &App, area: Rect) {
    let accent = theme::accent(app.view.index());
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(theme::HEADER_BG));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let clock = Local::now().format("%H:%M:%S").to_string();
    let (dot, state, state_color) = if app.snap.connected {
        ("●", "ONLINE", theme::GREEN)
    } else if app.snap.updated.is_some() {
        ("●", "OFFLINE", theme::ROSE)
    } else {
        ("◐", "CONNECTING", theme::AMBER)
    };
    let updated = app
        .snap
        .updated
        .map(|t| t.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "—".into());

    let right = format!(" {state} · api · {updated} · {clock} ");
    let right_width = right.chars().count() + dot.chars().count() + 1;

    let mut spans: Vec<Span> = vec![
        Span::styled("◆ ", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
    ];
    for (i, ch) in "ALAMUT".chars().enumerate() {
        spans.push(Span::styled(
            ch.to_string(),
            theme::wordmark("ALAMUT")[i],
        ));
    }
    spans.push(Span::styled(
        " TUI",
        Style::default().fg(theme::SLATE).add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(
        "  │  ",
        Style::default().fg(theme::DIM),
    ));
    spans.push(Span::styled(
        app.view.symbol().to_string(),
        Style::default().fg(theme::accent(app.view.index() + 1)),
    ));
    spans.push(Span::styled(
        format!(" {} ", app.view.title()),
        Style::default().fg(theme::FG).add_modifier(Modifier::BOLD),
    ));
    if app.nic_filter.is_some() {
        spans.push(Span::styled(
            format!(" ⚲ {}", app.selected_nic_label()),
            Style::default().fg(theme::AMBER),
        ));
    }

    let filler = (inner.width as usize).saturating_sub(text_width(&spans) + right_width);
    spans.push(Span::styled(
        " ".repeat(filler),
        Style::default().bg(theme::HEADER_BG),
    ));
    spans.push(Span::styled(
        format!("{dot} {state}"),
        Style::default().fg(state_color).add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(
        format!(" · {updated} · {clock}  "),
        Style::default().fg(theme::SLATE),
    ));

    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(theme::HEADER_BG)),
        inner,
    );
}

fn text_width(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.content.chars().count()).sum()
}

fn tabs(f: &mut Frame, app: &App, area: Rect) {
    let accent = theme::accent(app.view.index());
    let titles: Vec<Line> = View::ALL
        .iter()
        .map(|v| {
            Line::from(vec![
                Span::styled(
                    format!("{} ", v.index() + 1),
                    Style::default().fg(theme::DIM),
                ),
                Span::styled(v.symbol().to_string(), Style::default().fg(theme::accent(v.index()))),
                Span::styled(
                    format!(" {}", v.title()),
                    Style::default().fg(if *v == app.view {
                        theme::FG
                    } else {
                        theme::SLATE
                    }),
                ),
            ])
        })
        .collect();

    let widget = Tabs::new(titles)
        .select(app.view.index())
        .divider(Span::styled("│", Style::default().fg(theme::DIM)))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(accent)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default())
        .block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .border_style(Style::default().fg(theme::HEADER_BG)),
        );
    f.render_widget(widget, area);
}

fn body(f: &mut Frame, app: &mut App, area: Rect) {
    if !app.snap.connected {
        if let Some(err) = app.snap.error.clone() {
            let msg = match app.snap.updated {
                Some(_) => format!("API unreachable: {err}"),
                None => "Connecting to the Alamut API…".to_string(),
            };
            let p = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  {msg}"),
                    Style::default().fg(theme::ROSE),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Start it with:  alamut run   (or alamut service start)",
                    Style::default().fg(theme::SLATE),
                )),
                Line::from(Span::styled(
                    "  Override the endpoint with:  alamut-cli --api http://127.0.0.1:8000",
                    Style::default().fg(theme::DIM),
                )),
            ])
            .block(widgets::panel(" API ", theme::ROSE));
            f.render_widget(p, area);
            return;
        }
    }

    match app.view {
        View::Overview => overview::render(f, app, area),
        View::Nics => nics::render(f, app, area),
        View::Bandwidth => bandwidth::render(f, app, area),
        View::Latency => latency::render(f, app, area),
        View::System => system::render(f, app, area),
        View::Apps => apps::render(f, app, area),
        View::Tools => tools::render(f, app, area),
    }
}

fn footer(f: &mut Frame, app: &App, area: Rect) {
    let key = |k: &str| Span::styled(format!(" {k} "), Style::default().fg(Color::Black).bg(theme::SLATE).add_modifier(Modifier::BOLD));
    let dim = |t: &str| Span::styled(t.to_string(), Style::default().fg(theme::DIM));

    let mut spans = match app.view {
        View::Tools if app.input_active => vec![
            key("Enter"),
            dim(" run  "),
            key("Esc"),
            dim(" cancel  "),
            dim("type the target, then press Enter"),
        ],
        View::Tools => vec![
            key("i"),
            dim(" input  "),
            key("←/→"),
            dim(" tool  "),
            key("r"),
            dim(" run  "),
            key("a"),
            dim(" accept EULA  "),
            key("c"),
            dim(" clear  "),
            key("↑/↓"),
            dim(" scroll  "),
        ],
        View::Nics | View::Bandwidth | View::Latency => vec![
            key("↑/↓"),
            dim(" pick NIC  "),
            key("Enter"),
            dim(" apply filter  "),
            key("r"),
            dim(" refresh  "),
        ],
        _ => vec![key("r"), dim(" refresh  ")],
    };
    spans.extend([
        dim("  │  "),
        key("?"),
        dim(" help  "),
        key("Tab"),
        dim(" next  "),
        key("q"),
        dim(" quit"),
    ]);

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn sample_app(view: View) -> App {
        let mut app = App::new();
        app.view = view;
        app.snap.connected = true;
        app.snap.updated = Some(Local::now());
        app.snap.status = Some(alamut_core::types::ServiceStatus {
            running: true,
            pipe_connected: true,
            capture_mode: "driver".into(),
            message: "ok".into(),
        });
        app.snap.cpu = Some(alamut_core::types::CpuSnapshot {
            brand: "Test CPU".into(),
            vendor_id: "test".into(),
            physical_cores: Some(4),
            logical_cores: 8,
            frequency_mhz: Some(3600),
            utilization: 0.42,
            per_core: vec![0.1, 0.4, 0.9, 0.2],
            top_processes: Vec::new(),
        });
        app.snap.memory = Some(alamut_core::types::MemorySnapshot {
            total_bytes: 16 * 1024 * 1024 * 1024,
            used_bytes: 6 * 1024 * 1024 * 1024,
            available_bytes: 10 * 1024 * 1024 * 1024,
            utilization: 0.375,
        });
        app.snap.disks = vec![alamut_core::types::DiskSnapshot {
            id: "c".into(),
            name: "System".into(),
            mount_point: "C:\\".into(),
            file_system: "NTFS".into(),
            kind: alamut_core::types::DiskKind::Ssd,
            total_bytes: 512 * 1024 * 1024 * 1024,
            used_bytes: 200 * 1024 * 1024 * 1024,
            available_bytes: 312 * 1024 * 1024 * 1024,
            utilization: 0.39,
        }];
        app.snap.bandwidth = (0..30)
            .map(|i| alamut_core::types::UsagePoint {
                ts: 1_700_000_000 + i * 30,
                rx_bytes: (i as u64) * 1_000_000,
                tx_bytes: (i as u64) * 250_000,
            })
            .collect();
        app.snap.ping = (0..10)
            .map(|i| alamut_core::types::PingSample {
                nic_id: None,
                target: "1.1.1.1".into(),
                ts: 1_700_000_000 + i * 60,
                rtt_ms: Some(10.0 + (i as f64)),
                success: true,
                error: None,
                raw_output: None,
            })
            .collect();
        app
    }

    #[test]
    fn every_view_renders() {
        for view in View::ALL {
            let mut app = sample_app(view);
            let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
            terminal
                .draw(|f| render(f, &mut app))
                .unwrap_or_else(|e| panic!("view {:?} failed to render: {e}", view.title()));
        }
    }

    #[test]
    fn every_view_renders_when_offline() {
        let mut app = App::new();
        app.snap.connected = false;
        app.snap.error = Some("connection refused".into());
        app.snap.updated = Some(Local::now());
        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        terminal.draw(|f| render(f, &mut app)).unwrap();
    }

    #[test]
    fn help_overlay_renders() {
        let mut app = sample_app(View::Overview);
        app.show_help = true;
        let mut terminal = Terminal::new(TestBackend::new(100, 32)).unwrap();
        terminal.draw(|f| render(f, &mut app)).unwrap();
    }

    #[test]
    fn tiny_terminal_is_handled() {
        let mut app = sample_app(View::Overview);
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        terminal.draw(|f| render(f, &mut app)).unwrap();
    }
}

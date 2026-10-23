mod.rs 714L cognitive
// C:/Users/armin/GitHub/Alamut/netvan-api/crates/alamut-cli/src/ui/mod.rs
§ function event_loop (L120-L164)
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
// ... 26 lines omitted
§ function handle_key(app (L191-L261)
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
// ... 8 lines omitted
§ function handle_input_key(app (L270-L303)
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
// ... 1 lines omitted
§ function handle_tools_key(app (L305-L342)
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
// ... 1 lines omitted
§ function submit_tool(app (L344-L367)
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
// ... 32 lines omitted
§ function header(f (L400-L477)
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
// ... 47 lines omitted
§ function body(f (L525-L563)
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
                    "  Override the endpoint with:  alamut-cli --api http://127.0.0.1:28437",
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
7/32 chunks shown (2746 tokens)
[lean-ctx] full source: read "C:/Users/armin/GitHub/Alamut/netvan-api/crates/alamut-cli/src/ui/mod.rs" directly (no MCP)  ·  or ctx_read("C:/Users/armin/GitHub/Alamut/netvan-api/crates/alamut-cli/src/ui/mod.rs", mode="full")

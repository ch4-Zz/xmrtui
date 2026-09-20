use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap};

use crate::app::{App, Mode, SendField, View};
use crate::rpc::format_xmr;

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_tabs(frame, app, chunks[0]);
    match app.view {
        View::Dashboard => draw_dashboard(frame, app, chunks[1]),
        View::Transfers => draw_transfers(frame, app, chunks[1]),
        View::Send => draw_send(frame, app, chunks[1]),
        View::Help => draw_help(frame, chunks[1]),
    }
    draw_status(frame, app, chunks[2]);
    draw_cmdline(frame, app, chunks[3]);

    if app.mode == Mode::Password {
        draw_password_modal(frame, app);
    } else if app.mode == Mode::Confirm {
        draw_confirm_modal(frame, app);
    }
}

fn draw_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let titles = ["dashboard", "transfers", "send", "help"];
    let selected = match app.view {
        View::Dashboard => 0,
        View::Transfers => 1,
        View::Send => 2,
        View::Help => 3,
    };
    let sync = app
        .snapshot
        .as_ref()
        .map(|s| {
            if s.daemon_ok {
                format!("height {}", s.height)
            } else {
                format!("height {}  OUT OF SYNC", s.height)
            }
        })
        .unwrap_or_else(|| "locked".into());
    let title = format!(" xmrtui · {} · {sync} ", app.wallet_name);
    let tabs = Tabs::new(titles.iter().copied().map(Line::from))
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .select(selected);
    frame.render_widget(tabs, area);
}

fn draw_dashboard(frame: &mut Frame, app: &App, area: Rect) {
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(3)])
        .split(area);

    let (balance, unlocked, address, height, daemon_state, daemon_style) = match &app.snapshot {
        Some(s) => {
            let (state, style) = if s.daemon_ok {
                (
                    format!("{}  ok", app.daemon),
                    Style::default().fg(Color::Green),
                )
            } else {
                (
                    format!("{}  UNREACHABLE", app.daemon),
                    Style::default().fg(Color::Red),
                )
            };
            (
                format_xmr(s.balance),
                format_xmr(s.unlocked),
                s.address.clone(),
                s.height.to_string(),
                state,
                style,
            )
        }
        None => (
            "—".into(),
            "—".into(),
            "—".into(),
            "—".into(),
            app.daemon.clone(),
            Style::default(),
        ),
    };

    let mut balance_spans = vec![
        Span::styled("balance   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{balance} XMR"),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(s) = &app.snapshot
        && let Some(usd) = app.usd_for_pico(s.balance.as_pico())
    {
        balance_spans.push(Span::styled(
            format!("  ({usd})"),
            Style::default().fg(Color::DarkGray),
        ));
    }

    let info = Paragraph::new(vec![
        Line::from(balance_spans),
        Line::from(vec![
            Span::styled("unlocked  ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{unlocked} XMR")),
        ]),
        Line::from(vec![
            Span::styled("height    ", Style::default().fg(Color::DarkGray)),
            Span::raw(height),
        ]),
        Line::from(vec![
            Span::styled("daemon    ", Style::default().fg(Color::DarkGray)),
            Span::styled(daemon_state, daemon_style),
        ]),
        Line::from(vec![
            Span::styled("rpc       ", Style::default().fg(Color::DarkGray)),
            Span::raw(app.rpc.bind.clone()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("address   ", Style::default().fg(Color::DarkGray)),
            Span::styled(address, Style::default().fg(Color::Cyan)),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title(" wallet "));
    frame.render_widget(info, inner[0]);

    let hint = Paragraph::new(
        "j/k list  gt next tab  r refresh  yy yank address  :send 4.. 0.1  :q\n? help",
    )
    .block(Block::default().borders(Borders::ALL).title(" keys "))
    .wrap(Wrap { trim: true });
    frame.render_widget(hint, inner[1]);
}

fn draw_transfers(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(6)])
        .split(area);

    let items: Vec<ListItem> = match &app.snapshot {
        Some(snap) if !snap.transfers.is_empty() => snap
            .transfers
            .iter()
            .enumerate()
            .map(|(i, tx)| {
                let marker = if i == app.selected { "❯ " } else { "  " };
                let color = match tx.kind.as_str() {
                    "in" | "pool" => Color::Green,
                    "out" => Color::Red,
                    "pending" => Color::Yellow,
                    "failed" => Color::Magenta,
                    _ => Color::White,
                };
                let sign = if tx.kind == "out" { "-" } else { "+" };
                let line = format!(
                    "{marker}{:<8} {sign}{:>14} XMR  {:>4}c  {}",
                    tx.kind,
                    format_xmr(tx.amount),
                    tx.confirmations.unwrap_or(0),
                    tx.timestamp,
                );
                let style = if i == app.selected {
                    Style::default().fg(color).add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().fg(color)
                };
                ListItem::new(line).style(style)
            })
            .collect(),
        _ => vec![ListItem::new("no transfers yet — :refresh")],
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" transfers  (yy yank txid) "),
    );
    frame.render_widget(list, chunks[0]);

    let detail = if let Some(tx) = app.selected_transfer() {
        vec![
            Line::from(format!("txid  {}", tx.txid)),
            Line::from(format!("addr  {}", tx.address)),
            Line::from(format!("fee   {} XMR", format_xmr(tx.fee))),
        ]
    } else {
        vec![Line::from("no transfer selected")]
    };
    frame.render_widget(
        Paragraph::new(detail).block(Block::default().borders(Borders::ALL).title(" detail ")),
        chunks[1],
    );
}

fn draw_send(frame: &mut Frame, app: &App, area: Rect) {
    let addr_style = field_style(app, SendField::Address);
    let amt_style = field_style(app, SendField::Amount);
    let unlocked = app
        .snapshot
        .as_ref()
        .map(|s| format_xmr(s.unlocked))
        .unwrap_or_else(|| "—".into());

    let text = vec![
        Line::from(format!("unlocked: {unlocked} XMR")),
        Line::from(""),
        Line::from(vec![
            Span::styled("address  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                display_field(&app.send_address, app, SendField::Address),
                addr_style,
            ),
        ]),
        Line::from(vec![
            Span::styled("amount   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                display_field(&app.send_amount, app, SendField::Amount),
                amt_style,
            ),
            Span::raw(" XMR"),
        ]),
        Line::from(""),
        Line::from("i insert   tab field   enter confirm   esc normal"),
        Line::from("or :send <address> <amount>"),
    ];
    frame.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title(" send ")),
        area,
    );
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let help = "\
xmrtui talks to official monero-wallet-rpc — same engine as monero-wallet-cli.

modes
  NORMAL    default
  INSERT    edit send form (i)
  COMMAND   : like vim
  CONFIRM   y/n before broadcast

keys
  1 2 3     dashboard / transfers / send
  gt gT     next / previous tab
  j k G gg  move in transfer list
  r         refresh (CLI: refresh)
  yy        yank address (or txid on transfers)
  :q        quit
  :refresh  rescan
  :address  show + yank
  :balance  print balances
  :transfers
  :send <addr> <amount>
  tab       complete :command  (S-tab reverse)

the wallet file never leaves the official binary.
";
    frame.render_widget(
        Paragraph::new(help)
            .block(Block::default().borders(Borders::ALL).title(" help "))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let mode = match app.mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
        Mode::Command => "COMMAND",
        Mode::Password => "PASSWORD",
        Mode::Confirm => "CONFIRM",
    };
    let mut spans = vec![
        Span::styled(
            format!(" {mode} "),
            Style::default()
                .fg(Color::Black)
                .bg(mode_color(app.mode))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::raw(&app.status),
    ];
    if app.busy {
        spans.push(Span::styled("  …", Style::default().fg(Color::Yellow)));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_cmdline(frame: &mut Frame, app: &App, area: Rect) {
    if app.mode == Mode::Command {
        let mut spans = vec![Span::styled(
            format!(":{}", app.command),
            Style::default().fg(Color::Yellow),
        )];
        if let Some(hint) = &app.completion_hint {
            spans.push(Span::styled(
                format!("  {hint}"),
                Style::default().fg(Color::DarkGray),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
        return;
    }
    if let Some(err) = &app.error {
        frame.render_widget(
            Paragraph::new(err.as_str()).style(Style::default().fg(Color::Red)),
            area,
        );
    }
}

fn draw_password_modal(frame: &mut Frame, app: &App) {
    let area = centered(frame.area(), 60, 7);
    frame.render_widget(Clear, area);
    let stars = "*".repeat(app.password.len());
    let p = Paragraph::new(vec![
        Line::from(format!("wallet: {}", app.wallet_name)),
        Line::from(""),
        Line::from(format!("password: {stars}")),
        Line::from("enter to unlock · ctrl-c to quit"),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" unlock ")
            .style(Style::default().fg(Color::Yellow)),
    );
    frame.render_widget(p, area);
}

fn draw_confirm_modal(frame: &mut Frame, app: &App) {
    let area = centered(frame.area(), 72, 5);
    frame.render_widget(Clear, area);
    let p = Paragraph::new(vec![
        Line::from(app.confirm_text.clone()),
        Line::from("y / enter = broadcast   n / esc = cancel"),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" confirm transfer ")
            .style(Style::default().fg(Color::Red)),
    );
    frame.render_widget(p, area);
}

fn field_style(app: &App, field: SendField) -> Style {
    if app.view == View::Send && app.send_field == field {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::UNDERLINED)
    } else {
        Style::default()
    }
}

fn display_field(value: &str, app: &App, field: SendField) -> String {
    if value.is_empty() {
        if app.mode == Mode::Insert && app.send_field == field {
            "▌".into()
        } else {
            "…".into()
        }
    } else if app.mode == Mode::Insert && app.send_field == field {
        format!("{value}▌")
    } else {
        value.to_string()
    }
}

fn mode_color(mode: Mode) -> Color {
    match mode {
        Mode::Normal => Color::Green,
        Mode::Insert => Color::Blue,
        Mode::Command => Color::Yellow,
        Mode::Password => Color::Magenta,
        Mode::Confirm => Color::Red,
    }
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    }
}

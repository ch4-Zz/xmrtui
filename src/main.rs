mod app;
mod rpc;
mod ui;

use std::io::stdout;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::event::{Event, EventStream};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures::StreamExt;
use futures::future::pending;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::time::{MissedTickBehavior, interval};

use app::{App, Outcome, apply_action};
use rpc::{WalletRpc, default_daemon_address, fetch_snapshot, split_wallet_path};

#[derive(Parser, Debug)]
#[command(name = "xmrtui", about = "Vim-like TUI over monero-wallet-rpc")]
struct Cli {
    /// Wallet file (without or with .keys)
    #[arg(long, default_value = "~/Monero/wallets/wallet_1")]
    wallet_file: String,

    /// monerod / remote node host:port
    #[arg(long, default_value_t = default_daemon_address())]
    daemon_address: String,

    /// Attach to an already running wallet RPC instead of spawning one
    #[arg(long)]
    rpc_url: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let wallet_path = expand_tilde(&cli.wallet_file);
    let (wallet_dir, wallet_name) = split_wallet_path(&wallet_path)?;

    let rpc = if let Some(url) = cli.rpc_url {
        WalletRpc::connect_existing(&url)?
    } else {
        WalletRpc::spawn(&wallet_dir, &cli.daemon_address).await?
    };

    let mut app = App::new(rpc, wallet_name, cli.daemon_address.clone());
    let result = run_tui(&mut app).await;
    let _ = app.rpc.shutdown().await;
    result
}

async fn run_tui(app: &mut App) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut events = EventStream::new();
    let mut ticks = interval(Duration::from_secs(5));
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut inflight: Option<tokio::task::JoinHandle<anyhow::Result<rpc::Snapshot>>> = None;
    let result = loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if app.want_refresh && inflight.is_none() {
            app.want_refresh = false;
            let client = app.rpc.client.clone();
            inflight = Some(tokio::spawn(async move { fetch_snapshot(&client).await }));
        }

        tokio::select! {
            maybe_event = events.next() => {
                let Some(event) = maybe_event else { break Ok(()); };
                match event {
                    Ok(Event::Key(key)) if key.kind == crossterm::event::KeyEventKind::Press => {
                        let action = app.handle_key(key);
                        if matches!(apply_action(app, action).await?, Outcome::Quit) {
                            break Ok(());
                        }
                    }
                    Ok(Event::Resize(_, _)) => {}
                    Err(err) => break Err(err).context("terminal event"),
                    _ => {}
                }
            }
            _ = ticks.tick() => {
                if app.unlocked && inflight.is_none() {
                    app.request_refresh();
                }
            }
            res = async {
                if let Some(handle) = inflight.as_mut() {
                    handle.await
                } else {
                    pending().await
                }
            } => {
                inflight = None;
                match res {
                    Ok(snap) => app.apply_snapshot(snap.map_err(|err| err.to_string())),
                    Err(err) => app.apply_snapshot(Err(err.to_string())),
                }
            }
        }
    };

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs_home()
    {
        return home.join(rest);
    }
    PathBuf::from(path)
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

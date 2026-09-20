use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::rpc::{
    Snapshot, TransferRow, WalletRpc, fetch_snapshot, format_xmr, open_wallet, send_xmr,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Password,
    Normal,
    Insert,
    Command,
    Confirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Dashboard,
    Transfers,
    Send,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendField {
    Address,
    Amount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Chord {
    None,
    G,
    Y,
}

pub enum Outcome {
    Continue,
    Quit,
}

pub struct App {
    pub rpc: WalletRpc,
    pub wallet_name: String,
    pub daemon: String,
    pub mode: Mode,
    pub view: View,
    pub password: String,
    pub command: String,
    pub status: String,
    pub error: Option<String>,
    pub snapshot: Option<Snapshot>,
    pub busy: bool,
    pub selected: usize,
    pub send_address: String,
    pub send_amount: String,
    pub send_field: SendField,
    pub confirm_text: String,
    chord: Chord,
    pub unlocked: bool,
}

impl App {
    pub fn new(rpc: WalletRpc, wallet_name: String, daemon: String) -> Self {
        Self {
            rpc,
            wallet_name,
            daemon,
            mode: Mode::Password,
            view: View::Dashboard,
            password: String::new(),
            command: String::new(),
            status: "enter wallet password".into(),
            error: None,
            snapshot: None,
            busy: false,
            selected: 0,
            send_address: String::new(),
            send_amount: String::new(),
            send_field: SendField::Address,
            confirm_text: String::new(),
            chord: Chord::None,
            unlocked: false,
        }
    }

    pub fn selected_transfer(&self) -> Option<&TransferRow> {
        self.snapshot.as_ref()?.transfers.get(self.selected)
    }

    pub async fn unlock(&mut self) -> Result<()> {
        self.busy = true;
        self.status = "opening wallet…".into();
        let result = open_wallet(&self.rpc.client, &self.wallet_name, &self.password).await;
        self.password.clear();
        match result {
            Ok(()) => {
                self.unlocked = true;
                self.mode = Mode::Normal;
                self.status = "wallet unlocked".into();
                self.error = None;
                self.reload().await;
            }
            Err(err) => {
                self.error = Some(err.to_string());
                self.status = "wrong password or RPC error".into();
                self.mode = Mode::Password;
            }
        }
        self.busy = false;
        Ok(())
    }

    pub async fn reload(&mut self) {
        self.busy = true;
        self.status = "refresh…".into();
        match fetch_snapshot(&self.rpc.client).await {
            Ok(snap) => {
                if self.selected >= snap.transfers.len() {
                    self.selected = snap.transfers.len().saturating_sub(1);
                }
                self.snapshot = Some(snap);
                self.error = None;
                self.status = "synced".into();
            }
            Err(err) => {
                self.error = Some(err.to_string());
                self.status = "refresh failed".into();
            }
        }
        self.busy = false;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> KeyAction {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return KeyAction::Quit;
        }

        match self.mode {
            Mode::Password => self.handle_password(key),
            Mode::Command => self.handle_command(key),
            Mode::Insert => self.handle_insert(key),
            Mode::Confirm => self.handle_confirm(key),
            Mode::Normal => self.handle_normal(key),
        }
    }

    fn handle_password(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Enter => KeyAction::Unlock,
            KeyCode::Esc => KeyAction::Quit,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => KeyAction::Quit,
            KeyCode::Backspace => {
                self.password.pop();
                KeyAction::None
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.password.push(c);
                KeyAction::None
            }
            _ => KeyAction::None,
        }
    }

    fn handle_normal(&mut self, key: KeyEvent) -> KeyAction {
        if self.chord == Chord::G {
            self.chord = Chord::None;
            return match key.code {
                KeyCode::Char('g') => {
                    self.selected = 0;
                    KeyAction::None
                }
                KeyCode::Char('t') => {
                    self.next_view();
                    KeyAction::None
                }
                KeyCode::Char('T') => {
                    self.prev_view();
                    KeyAction::None
                }
                _ => KeyAction::None,
            };
        }
        if self.chord == Chord::Y {
            self.chord = Chord::None;
            if key.code == KeyCode::Char('y') {
                return KeyAction::Yank;
            }
            return KeyAction::None;
        }

        match key.code {
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command.clear();
                KeyAction::None
            }
            KeyCode::Char('i') if self.view == View::Send => {
                self.mode = Mode::Insert;
                self.status = "INSERT  tab=field  esc=normal".into();
                KeyAction::None
            }
            KeyCode::Char('g') => {
                self.chord = Chord::G;
                KeyAction::None
            }
            KeyCode::Char('y') => {
                self.chord = Chord::Y;
                KeyAction::None
            }
            KeyCode::Char('r') => KeyAction::Reload,
            KeyCode::Char('?') => {
                self.view = View::Help;
                KeyAction::None
            }
            KeyCode::Char('1') => {
                self.view = View::Dashboard;
                KeyAction::None
            }
            KeyCode::Char('2') => {
                self.view = View::Transfers;
                KeyAction::None
            }
            KeyCode::Char('3') => {
                self.view = View::Send;
                KeyAction::None
            }
            KeyCode::Tab => {
                self.next_view();
                KeyAction::None
            }
            KeyCode::BackTab => {
                self.prev_view();
                KeyAction::None
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_sel(1);
                KeyAction::None
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_sel(-1);
                KeyAction::None
            }
            KeyCode::Char('G') => {
                if let Some(snap) = &self.snapshot {
                    self.selected = snap.transfers.len().saturating_sub(1);
                }
                KeyAction::None
            }
            KeyCode::Char('q') => {
                self.mode = Mode::Command;
                self.command = "q".into();
                KeyAction::None
            }
            _ => KeyAction::None,
        }
    }

    fn handle_insert(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "NORMAL".into();
                KeyAction::None
            }
            KeyCode::Tab => {
                self.send_field = match self.send_field {
                    SendField::Address => SendField::Amount,
                    SendField::Amount => SendField::Address,
                };
                KeyAction::None
            }
            KeyCode::Backspace => {
                self.active_field().pop();
                KeyAction::None
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                KeyAction::PromptSend
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.active_field().push(c);
                KeyAction::None
            }
            _ => KeyAction::None,
        }
    }

    fn handle_command(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command.clear();
                KeyAction::None
            }
            KeyCode::Backspace => {
                if self.command.is_empty() {
                    self.mode = Mode::Normal;
                } else {
                    self.command.pop();
                }
                KeyAction::None
            }
            KeyCode::Enter => {
                let cmd = self.command.clone();
                self.command.clear();
                self.mode = Mode::Normal;
                self.run_command(&cmd)
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.command.push(c);
                KeyAction::None
            }
            _ => KeyAction::None,
        }
    }

    fn handle_confirm(&mut self, key: KeyEvent) -> KeyAction {
        match key.code {
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.mode = Mode::Normal;
                self.status = "transfer cancelled".into();
                KeyAction::None
            }
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => KeyAction::DoSend,
            _ => KeyAction::None,
        }
    }

    fn run_command(&mut self, raw: &str) -> KeyAction {
        let line = raw.trim();
        let (cmd, rest) = line
            .split_once(char::is_whitespace)
            .map(|(a, b)| (a, b.trim()))
            .unwrap_or((line, ""));

        match cmd {
            "q" | "quit" | "q!" => KeyAction::Quit,
            "refresh" | "sync" | "redraw" => KeyAction::Reload,
            "help" | "h" => {
                self.view = View::Help;
                KeyAction::None
            }
            "address" => {
                self.view = View::Dashboard;
                KeyAction::Yank
            }
            "balance" => {
                self.view = View::Dashboard;
                if let Some(s) = &self.snapshot {
                    self.status = format!(
                        "balance {}  unlocked {}",
                        format_xmr(s.balance),
                        format_xmr(s.unlocked)
                    );
                }
                KeyAction::None
            }
            "transfers" | "history" => {
                self.view = View::Transfers;
                KeyAction::None
            }
            "send" if rest.is_empty() => {
                self.view = View::Send;
                self.mode = Mode::Insert;
                KeyAction::None
            }
            "send" => {
                let mut parts = rest.split_whitespace();
                if let (Some(addr), Some(amount)) = (parts.next(), parts.next()) {
                    self.send_address = addr.to_string();
                    self.send_amount = amount.to_string();
                    KeyAction::PromptSend
                } else {
                    self.error = Some("usage: :send <address> <amount>".into());
                    KeyAction::None
                }
            }
            "tabn" | "bn" => {
                self.next_view();
                KeyAction::None
            }
            "tabp" | "bp" => {
                self.prev_view();
                KeyAction::None
            }
            other => {
                self.error = Some(format!("unknown command: {other}"));
                KeyAction::None
            }
        }
    }

    pub fn prompt_send(&mut self) {
        if self.send_address.is_empty() || self.send_amount.is_empty() {
            self.error = Some("address and amount required".into());
            return;
        }
        self.confirm_text = format!(
            "send {} XMR to {} ?",
            self.send_amount.trim(),
            truncate(&self.send_address, 18)
        );
        self.mode = Mode::Confirm;
        self.status = "y confirm / n cancel".into();
    }

    pub async fn do_send(&mut self) {
        self.busy = true;
        self.mode = Mode::Normal;
        self.status = "broadcasting…".into();
        match send_xmr(
            &self.rpc.client,
            self.send_address.trim(),
            self.send_amount.trim(),
        )
        .await
        {
            Ok(txid) => {
                self.error = None;
                self.status = format!("sent  txid {txid}");
                self.send_amount.clear();
                self.reload().await;
            }
            Err(err) => {
                self.error = Some(err.to_string());
                self.status = "transfer failed".into();
            }
        }
        self.busy = false;
    }

    pub fn yank_current(&mut self) {
        let text = match self.view {
            View::Transfers => self
                .selected_transfer()
                .map(|t| t.txid.clone())
                .or_else(|| self.snapshot.as_ref().map(|s| s.address.clone())),
            _ => self.snapshot.as_ref().map(|s| s.address.clone()),
        };
        if let Some(text) = text {
            match arboard::Clipboard::new().and_then(|mut c| c.set_text(text.clone())) {
                Ok(()) => self.status = format!("yanked {text}"),
                Err(_) => self.status = format!("yank failed, value: {text}"),
            }
        }
    }

    fn active_field(&mut self) -> &mut String {
        match self.send_field {
            SendField::Address => &mut self.send_address,
            SendField::Amount => &mut self.send_amount,
        }
    }

    fn move_sel(&mut self, delta: i32) {
        let len = self
            .snapshot
            .as_ref()
            .map(|s| s.transfers.len())
            .unwrap_or(0) as i32;
        if len == 0 {
            return;
        }
        let next = (self.selected as i32 + delta).clamp(0, len - 1);
        self.selected = next as usize;
    }

    fn next_view(&mut self) {
        self.view = match self.view {
            View::Dashboard => View::Transfers,
            View::Transfers => View::Send,
            View::Send => View::Help,
            View::Help => View::Dashboard,
        };
    }

    fn prev_view(&mut self) {
        self.view = match self.view {
            View::Dashboard => View::Help,
            View::Help => View::Send,
            View::Send => View::Transfers,
            View::Transfers => View::Dashboard,
        };
    }
}

pub enum KeyAction {
    None,
    Quit,
    Unlock,
    Reload,
    Yank,
    PromptSend,
    DoSend,
}

fn truncate(s: &str, keep: usize) -> String {
    if s.len() <= keep * 2 + 3 {
        s.to_string()
    } else {
        format!("{}…{}", &s[..keep], &s[s.len() - keep..])
    }
}

pub async fn apply_action(app: &mut App, action: KeyAction) -> Result<Outcome> {
    match action {
        KeyAction::None => Ok(Outcome::Continue),
        KeyAction::Quit => {
            let _ = app.rpc.shutdown().await;
            Ok(Outcome::Quit)
        }
        KeyAction::Unlock => {
            app.unlock().await?;
            Ok(Outcome::Continue)
        }
        KeyAction::Reload => {
            app.reload().await;
            Ok(Outcome::Continue)
        }
        KeyAction::Yank => {
            app.yank_current();
            Ok(Outcome::Continue)
        }
        KeyAction::PromptSend => {
            app.prompt_send();
            Ok(Outcome::Continue)
        }
        KeyAction::DoSend => {
            app.do_send().await;
            Ok(Outcome::Continue)
        }
    }
}

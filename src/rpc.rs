use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use monero::util::address::Address;
use monero::util::amount::Amount;
use monero_rpc::{
    GetTransfersCategory, GetTransfersSelector, TransferOptions, TransferPriority, WalletClient,
};
use tokio::process::{Child, Command};
use tokio::time::sleep;

pub struct WalletRpc {
    child: Option<Child>,
    pub client: WalletClient,
    pub bind: String,
}

impl WalletRpc {
    pub async fn spawn(wallet_dir: &Path, daemon_address: &str) -> Result<Self> {
        let port = free_port()?;
        let bind = format!("127.0.0.1:{port}");
        let log_file = std::env::temp_dir().join(format!("xmrtui-wallet-rpc-{port}.log"));

        let mut cmd = Command::new("monero-wallet-rpc");
        cmd.arg(format!("--wallet-dir={}", wallet_dir.display()))
            .arg("--rpc-bind-ip=127.0.0.1")
            .arg(format!("--rpc-bind-port={port}"))
            .arg("--disable-rpc-login")
            .arg(format!("--daemon-address={daemon_address}"))
            .arg("--log-level=0")
            .arg(format!("--log-file={}", log_file.display()))
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        let child = cmd.spawn().with_context(
            || "failed to start monero-wallet-rpc (is it in PATH? brew install monero)",
        )?;

        let url = format!("http://{bind}");
        let client = wait_for_rpc(&url).await.with_context(|| {
            format!(
                "monero-wallet-rpc did not become ready on {url}; log: {}",
                log_file.display()
            )
        })?;

        Ok(Self {
            child: Some(child),
            client,
            bind,
        })
    }

    pub fn connect_existing(rpc_url: &str) -> Result<Self> {
        let client = monero_rpc::RpcClientBuilder::new()
            .build(rpc_url)
            .with_context(|| format!("invalid RPC url {rpc_url}"))?
            .wallet();
        Ok(Self {
            child: None,
            client,
            bind: rpc_url.to_string(),
        })
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        let _ = self.client.close_wallet().await;
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
        Ok(())
    }
}

impl Drop for WalletRpc {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }
}

fn free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

async fn wait_for_rpc(url: &str) -> Result<WalletClient> {
    let mut last_err = None;
    for _ in 0..50 {
        match monero_rpc::RpcClientBuilder::new().build(url) {
            Ok(rpc) => {
                let wallet = rpc.wallet();
                if wallet.get_version().await.is_ok() {
                    return Ok(wallet);
                }
            }
            Err(err) => last_err = Some(err),
        }
        sleep(Duration::from_millis(100)).await;
    }
    match last_err {
        Some(err) => Err(err).context("RPC client build failed"),
        None => bail!("RPC never answered get_version"),
    }
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub address: String,
    pub balance: Amount,
    pub unlocked: Amount,
    pub height: u64,
    pub transfers: Vec<TransferRow>,
}

#[derive(Debug, Clone)]
pub struct TransferRow {
    pub kind: String,
    pub amount: Amount,
    pub fee: Amount,
    pub confirmations: Option<u64>,
    pub timestamp: String,
    pub txid: String,
    pub address: String,
}

pub async fn open_wallet(client: &WalletClient, filename: &str, password: &str) -> Result<()> {
    client
        .open_wallet(filename.to_string(), Some(password.to_string()))
        .await
        .with_context(|| format!("open_wallet({filename}) failed"))
}

pub async fn fetch_snapshot(client: &WalletClient) -> Result<Snapshot> {
    let _ = client.refresh(None).await;
    let height = client.get_height().await.map(|h| h.get()).unwrap_or(0);
    let address = client
        .get_address(0, None)
        .await
        .map(|data| data.address.to_string())
        .unwrap_or_else(|_| "(unavailable)".into());
    let (balance, unlocked) = match client.get_balance(0, None).await {
        Ok(data) => (data.balance, data.unlocked_balance),
        Err(_) => (Amount::from_pico(0), Amount::from_pico(0)),
    };

    let mut categories = std::collections::HashMap::new();
    for cat in [
        GetTransfersCategory::In,
        GetTransfersCategory::Out,
        GetTransfersCategory::Pending,
        GetTransfersCategory::Failed,
        GetTransfersCategory::Pool,
    ] {
        categories.insert(cat, true);
    }

    let selector = GetTransfersSelector {
        category_selector: categories,
        account_index: Some(0),
        subaddr_indices: None,
        block_height_filter: None,
    };

    let mut transfers = Vec::new();
    if let Ok(map) = client.get_transfers(selector).await {
        for (category, list) in map {
            for tx in list {
                transfers.push(TransferRow {
                    kind: format!("{category:?}").to_lowercase(),
                    amount: tx.amount,
                    fee: tx.fee,
                    confirmations: tx.confirmations,
                    timestamp: tx.timestamp.format("%Y-%m-%d %H:%M").to_string(),
                    txid: hex::encode(&tx.txid.0),
                    address: tx.address.to_string(),
                });
            }
        }
        transfers.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    }

    Ok(Snapshot {
        address,
        balance,
        unlocked,
        height,
        transfers,
    })
}

pub async fn send_xmr(client: &WalletClient, address: &str, amount_xmr: &str) -> Result<String> {
    let dest: Address = address.parse().context("invalid Monero address")?;
    let amount: Amount = parse_xmr(amount_xmr)?;
    let mut destinations = std::collections::HashMap::new();
    destinations.insert(dest, amount);

    let result = client
        .transfer(
            destinations,
            TransferPriority::Default,
            TransferOptions::default(),
        )
        .await
        .context("transfer failed")?;

    Ok(result.tx_hash.to_string())
}

fn parse_xmr(s: &str) -> Result<Amount> {
    let s = s.trim().replace('_', "");
    if s.is_empty() {
        bail!("empty amount");
    }
    if let Ok(pico) = s.parse::<u64>()
        && !s.contains('.')
    {
        // Whole XMR typed as integer (1 = 1 XMR), not piconero.
        return Ok(Amount::from_pico(
            pico.checked_mul(1_000_000_000_000)
                .context("amount overflow")?,
        ));
    }
    let xmr: f64 = s.parse().context("amount is not a number")?;
    if !xmr.is_finite() || xmr <= 0.0 {
        bail!("amount must be > 0");
    }
    let pico = (xmr * 1_000_000_000_000.0).round();
    if pico < 1.0 || pico > u64::MAX as f64 {
        bail!("amount out of range");
    }
    Ok(Amount::from_pico(pico as u64))
}

pub fn split_wallet_path(path: &Path) -> Result<(PathBuf, String)> {
    let path = path
        .canonicalize()
        .with_context(|| format!("wallet file not found: {}", path.display()))?;
    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .context("wallet filename is not utf-8")?
        .trim_end_matches(".keys")
        .to_string();
    let dir = path
        .parent()
        .map(Path::to_path_buf)
        .context("wallet path has no parent directory")?;
    Ok((dir, filename))
}

pub fn format_xmr(amount: Amount) -> String {
    let pico = amount.as_pico();
    let whole = pico / 1_000_000_000_000;
    let frac = pico % 1_000_000_000_000;
    let mut frac_s = format!("{frac:012}");
    while frac_s.ends_with('0') && frac_s.len() > 1 {
        frac_s.pop();
    }
    format!("{whole}.{frac_s}")
}

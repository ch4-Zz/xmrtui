use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::Value;

pub async fn fetch_xmr_usd() -> Result<f64> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("xmrtui/0.1")
        .build()?;
    match kraken(&client).await {
        Ok(price) => Ok(price),
        Err(_) => coingecko(&client).await,
    }
}

async fn kraken(client: &reqwest::Client) -> Result<f64> {
    let v: Value = client
        .get("https://api.kraken.com/0/public/Ticker?pair=XMRUSD")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let last = v
        .get("result")
        .and_then(Value::as_object)
        .and_then(|m| m.values().next())
        .and_then(|ticker| ticker.get("c"))
        .and_then(Value::as_array)
        .and_then(|c| c.first())
        .and_then(Value::as_str)
        .context("kraken ticker missing last price")?;
    parse_price(last)
}

async fn coingecko(client: &reqwest::Client) -> Result<f64> {
    let v: Value = client
        .get("https://api.coingecko.com/api/v3/simple/price?ids=monero&vs_currencies=usd")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let price = v
        .get("monero")
        .and_then(|m| m.get("usd"))
        .and_then(Value::as_f64)
        .context("coingecko price missing")?;
    if price > 0.0 && price.is_finite() {
        Ok(price)
    } else {
        bail!("coingecko returned a bad price")
    }
}

fn parse_price(s: &str) -> Result<f64> {
    let price: f64 = s.parse().context("price is not a number")?;
    if price > 0.0 && price.is_finite() {
        Ok(price)
    } else {
        bail!("bad price")
    }
}

pub fn xmr_pico_usd(pico: u64, usd_per_xmr: f64) -> String {
    format_usd((pico as f64 / 1_000_000_000_000.0) * usd_per_xmr)
}

pub fn format_usd(usd: f64) -> String {
    format!("${usd:.2}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kraken_last() {
        assert_eq!(parse_price("159.23000").unwrap(), 159.23);
    }

    #[test]
    fn rejects_zero() {
        assert!(parse_price("0").is_err());
    }

    #[test]
    fn converts_pico() {
        assert_eq!(xmr_pico_usd(1_000_000_000_000, 150.0), "$150.00");
        assert_eq!(xmr_pico_usd(500_000_000_000, 150.0), "$75.00");
    }
}

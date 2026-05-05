use anyhow::Result;
use serde::Deserialize;

const SOL_MINT: &str = "So11111111111111111111111111111111111111112";

#[derive(Deserialize, Debug)]
struct QuoteResp {
    #[serde(rename = "outAmount")] out_amount: String,
    #[serde(rename = "inAmount")] _in_amount: String,
    #[serde(rename = "priceImpactPct", default)] price_impact_pct: Option<String>,
}

pub struct Jupiter {
    base_url: String,
    client: reqwest::Client,
}

impl Jupiter {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(8))
                .build().unwrap(),
        }
    }

    /// Returns price of `mint` in USD, given current SOL/USD price.
    /// Quotes 1 SOL -> mint to derive token-per-SOL, then divides sol_usd / tokens_per_sol.
    pub async fn price_in_usd(&self, mint: &str, decimals: u8, sol_usd: f64) -> Result<f64> {
        let one_sol_lamports: u64 = 1_000_000_000;
        let url = format!(
            "{}?inputMint={}&outputMint={}&amount={}&slippageBps=100",
            self.base_url, SOL_MINT, mint, one_sol_lamports
        );
        let resp: QuoteResp = self.client.get(&url).send().await?.error_for_status()?.json().await?;
        let out: f64 = resp.out_amount.parse().unwrap_or(0.0);
        if out <= 0.0 { anyhow::bail!("zero out_amount from jupiter"); }
        let tokens_per_sol = out / 10f64.powi(decimals as i32);
        Ok(sol_usd / tokens_per_sol)
    }

    /// Quick SOL/USD via Jupiter quoting against USDC.
    pub async fn sol_usd(&self) -> Result<f64> {
        const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        let url = format!(
            "{}?inputMint={}&outputMint={}&amount=1000000000&slippageBps=50",
            self.base_url, SOL_MINT, USDC
        );
        let resp: QuoteResp = self.client.get(&url).send().await?.error_for_status()?.json().await?;
        let out: f64 = resp.out_amount.parse().unwrap_or(0.0);
        Ok(out / 1_000_000.0) // USDC has 6 decimals
    }
}

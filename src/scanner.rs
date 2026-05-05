use crate::config::Config;
use crate::pumpportal::NewToken;

pub struct FilterDecision {
    pub accept: bool,
    pub reason: String,
}

pub fn evaluate(cfg: &Config, tok: &NewToken, sol_usd: f64) -> FilterDecision {
    // Liquidity in USD: pump.fun reports vSolInBondingCurve (virtual SOL liquidity).
    let liq_sol = tok.v_sol.unwrap_or(0.0);
    let liq_usd = liq_sol * sol_usd;
    if liq_usd < cfg.filters.min_initial_liquidity_usd {
        return FilterDecision {
            accept: false,
            reason: format!("low_liquidity ${:.0} < ${:.0}", liq_usd, cfg.filters.min_initial_liquidity_usd),
        };
    }
    // pump.fun tokens have mint authority renounced + no freeze authority by default — trust the platform.
    // (If we expand beyond pump.fun, re-check via RPC getAccountInfo on the mint.)
    FilterDecision { accept: true, reason: "passed".into() }
}

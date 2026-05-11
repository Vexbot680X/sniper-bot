use crate::config::Config;
use crate::pumpportal::NewToken;

pub struct FilterDecision {
    pub accept: bool,
    pub reason: String,
}

pub fn evaluate(cfg: &Config, tok: &NewToken, sol_usd: f64) -> FilterDecision {
    // Mayhem-mode detection: pump.fun's optional 24h AI-market-maker mode — the WS
    // event includes an explicit `is_mayhem_mode` boolean. v_tokens stays ~1.073B for
    // both standard and mayhem launches, so the only reliable signal is the flag.
    // (Verified empirically 2026-05-07 by capturing raw WS frames.)
    if cfg.filters.reject_mayhem_mode && tok.is_mayhem_mode.unwrap_or(false) {
        return FilterDecision {
            accept: false,
            reason: "mayhem_mode".into(),
        };
    }

    let mcap_sol = tok.mcap_sol.unwrap_or(0.0);

    // USD-denominated floor takes precedence when set. Bot converts using
    // current sol_usd at decision time — always self-consistent regardless
    // of Jupiter availability.
    if cfg.filters.min_market_cap_usd > 0.0 {
        let mcap_usd = mcap_sol * sol_usd;
        if mcap_usd < cfg.filters.min_market_cap_usd {
            return FilterDecision {
                accept: false,
                reason: format!(
                    "low_mcap ${:.0} < ${:.0} ({:.1} SOL @ ${:.2}/SOL)",
                    mcap_usd, cfg.filters.min_market_cap_usd, mcap_sol, sol_usd
                ),
            };
        }
        // Upper-bound mcap filter (band-scalp strategies). 0.0 = no ceiling.
        if cfg.filters.max_market_cap_usd > 0.0 && mcap_usd > cfg.filters.max_market_cap_usd {
            return FilterDecision {
                accept: false,
                reason: format!(
                    "high_mcap ${:.0} > ${:.0} ({:.1} SOL @ ${:.2}/SOL)",
                    mcap_usd, cfg.filters.max_market_cap_usd, mcap_sol, sol_usd
                ),
            };
        }
    } else if mcap_sol < cfg.filters.min_market_cap_sol {
        return FilterDecision {
            accept: false,
            reason: format!("low_mcap {:.2} SOL < {:.2} SOL", mcap_sol, cfg.filters.min_market_cap_sol),
        };
    }

    // pump.fun handles mint renounce / freeze authority by default.
    FilterDecision { accept: true, reason: "passed".into() }
}

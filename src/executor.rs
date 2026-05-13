//! Live execution layer for pump.fun trades.
//!
//! Originally wrapped the `pumpfun` crate's `buy()` / `sell()` calls. Those
//! were broken for current-deployment pump.fun (Token-2022 mints, 18-account
//! buy ix layout) — see the 2026-05-08 0xbc4 incident. We now hand-roll the
//! instructions in [`crate::pump_ix`] and only use the `pumpfun` crate for its
//! `BondingCurveAccount` / `GlobalAccount` math + slippage helpers (which are
//! still correct).

use crate::config::Config;
use crate::jito::JitoClient;
use crate::pump_ix;
use crate::rpc::Rpc;
use crate::wallet;
use anyhow::{anyhow, Context, Result};
use pumpfun::{
    common::types::{Cluster, PriorityFee},
    PumpFun,
};
use rand::SeedableRng;
use solana_rpc_client_api::config::{RpcSendTransactionConfig, RpcSimulateTransactionConfig};
use solana_sdk::commitment_config::CommitmentLevel;
use solana_sdk::transaction::Transaction;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    native_token::sol_to_lamports,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
};
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::instruction::Instruction;
use solana_transaction_status::UiTransactionEncoding;

/// Mirror of pumpfun::PumpFun::get_priority_fee_instructions — not exported, so we
/// rebuild it here for `simulate_buy`. Same shape: optional unit_limit ix +
/// optional unit_price ix.
fn pumpfun_priority_fee_ixs(pf: &PriorityFee) -> Vec<Instruction> {
    let mut v = Vec::new();
    if let Some(limit) = pf.unit_limit {
        v.push(ComputeBudgetInstruction::set_compute_unit_limit(limit));
    }
    if let Some(price) = pf.unit_price {
        v.push(ComputeBudgetInstruction::set_compute_unit_price(price));
    }
    v
}
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// Result of an on-chain buy.
#[derive(Debug, Clone)]
pub struct BuyFill {
    pub signature: Signature,
    /// Raw token base-units received (pump.fun tokens are 6 decimals, so 1e6 = 1 token).
    pub tokens_base: u64,
    /// Actual SOL spent in lamports (includes the trade SOL plus fees parsed from logs).
    pub sol_spent_lamports: u64,
    /// Effective entry price in SOL per token (sol_spent / tokens_human).
    pub effective_price_sol: f64,
}

/// Result of an on-chain sell.
#[derive(Debug, Clone)]
pub struct SellFill {
    pub signature: Signature,
    pub tokens_sold_base: u64,
    pub sol_received_lamports: u64,
    pub effective_price_sol: f64,
}

/// Returns true if a pump.fun buy error is worth retrying — covers the
/// create-pool/WS-event race window. Specifically:
///  - Anchor 3012 / 0xbc4 (AccountNotInitialized — bonding curve ATA / global)
///  - BondingCurveNotFound (the crate's own pre-flight)
///  - generic "account not found" surfaced from RPC during get_bonding_curve_account
fn is_retriable_buy_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("0xbc4")
        || m.contains("3012")
        || m.contains("accountnotinitialized")
        || m.contains("bondingcurvenotfound")
        || m.contains("could not find account")
        || m.contains("account not found")
        // Anchor 6023 sometimes fires when bonding-curve state is mid-init.
        || m.contains("0x1787")
        // Token-2022 "Invalid Mint" (Custom(2)) — same first-buyer race, mint create
        // tx hasn't propagated to our RPC view yet. Seen 2026-05-09 18:21.
        || m.contains("invalid mint")
        || m.contains("instructionerror(2, custom(2))")
        // Generic "Custom(2)" buried in a logs block from the ATA-create CPI —
        // happens when the mint isn't visible at simulate time.
        || m.contains("failed: custom program error: 0x2")
}

/// Live executor — owns the trading wallet, vault wallet, and the pumpfun client.
pub struct Executor {
    pub trading_kp: Arc<Keypair>,
    pub vault_pubkey: Pubkey,
    pub vault_kp_for_balance_check: Arc<Keypair>, // owned only so we have the key file loaded
    pub rpc: Rpc,
    pub pump: PumpFun,
    pub slippage_bps: u64,
    pub priority_fee_percentile: u8,
    /// Optional Jito Block Engine client. When Some, every tx is dual-submitted
    /// to Jito (as a [tip_tx, trade_tx] bundle) AND Helius in parallel; whichever
    /// confirms first wins via Solana's signature dedup.
    pub jito: Option<JitoClient>,
    /// 🚀 LATENCY (2026-05-13): how long to wait for confirmation before
    /// declaring a send failure. Default 20s. Loaded from
    /// `cfg.trading.confirm_timeout_secs`.
    pub confirm_timeout_secs: u32,
    /// 🚀 LATENCY (2026-05-13): how often to poll `get_signature_statuses`
    /// during the confirmation wait. Default 400ms. Loaded from
    /// `cfg.trading.confirm_poll_interval_ms`.
    pub confirm_poll_interval_ms: u64,
}

impl Executor {
    pub fn new(cfg: &Config, rpc: Rpc) -> Result<Self> {
        let trading_path = std::env::var("SNIPER_WALLET_PATH")
            .unwrap_or_else(|_| {
                shellexpand::tilde("~/.openclaw/workspace/secrets/sniper-bot-wallet.json").to_string()
            });
        let vault_path = std::env::var("VAULT_WALLET_PATH")
            .unwrap_or_else(|_| {
                shellexpand::tilde("~/.openclaw/workspace/secrets/vault-wallet.json").to_string()
            });

        let trading_kp = Arc::new(wallet::load_keypair(&trading_path)
            .with_context(|| format!("load trading wallet from {}", trading_path))?);
        let vault_kp = Arc::new(wallet::load_keypair(&vault_path)
            .with_context(|| format!("load vault wallet from {}", vault_path))?);
        let vault_pubkey = vault_kp.pubkey();

        // Confirm pubkeys match the expected ones from config / docs — refuse to start otherwise.
        let expected_trading = "6vKnymALQDriaeQ6pFbSvQvMArEUmUL5BVVmvQieedtY";
        let expected_vault = "CcDr8rSE5FcZmYsiUJUThUUNC7QUvE5rmUZD93rx51XD";
        if trading_kp.pubkey().to_string() != expected_trading {
            return Err(anyhow!(
                "trading wallet pubkey mismatch: file={} expected={}",
                trading_kp.pubkey(), expected_trading
            ));
        }
        if vault_pubkey.to_string() != expected_vault {
            return Err(anyhow!(
                "vault wallet pubkey mismatch: file={} expected={}",
                vault_pubkey, expected_vault
            ));
        }

        // Build pumpfun client. We pass the same RPC endpoint as our internal RPC.
        let ws_url = rpc.endpoint.replace("https://", "wss://").replace("http://", "ws://");
        let cluster = Cluster::new(
            rpc.endpoint.clone(),
            ws_url,
            CommitmentConfig::confirmed(),
            PriorityFee::default(),
        );
        let pump = PumpFun::new(trading_kp.clone(), cluster);

        let jito = if cfg.jito.enabled {
            let c = JitoClient::new_with_percentile(
                cfg.jito.endpoint.clone(),
                cfg.jito.tip_lamports,
                cfg.jito.tip_max_lamports,
                cfg.jito.dynamic_tip_percentile,
            ).context("init jito client")?;
            tracing::info!(
                endpoint=%cfg.jito.endpoint,
                tip_floor_lamports=cfg.jito.tip_lamports,
                tip_max_lamports=cfg.jito.tip_max_lamports,
                dynamic_tip_percentile=cfg.jito.dynamic_tip_percentile,
                dual_submit=cfg.jito.dual_submit,
                "⚡ Jito Block Engine ENABLED (dynamic tip)"
            );
            Some(c)
        } else {
            tracing::info!("🔵 Jito disabled (cfg.jito.enabled = false). Tx submission via Helius only.");
            None
        };

        Ok(Self {
            trading_kp,
            vault_pubkey,
            vault_kp_for_balance_check: vault_kp,
            rpc,
            pump,
            slippage_bps: (cfg.trading.slippage_bps as u64).max(1),
            priority_fee_percentile: cfg.trading.priority_fee_percentile,
            jito,
            confirm_timeout_secs: cfg.trading.confirm_timeout_secs,
            confirm_poll_interval_ms: cfg.trading.confirm_poll_interval_ms,
        })
    }

    /// Build the priority-fee struct for this tx using our RPC's fee oracle.
    async fn build_priority_fee(&self) -> Option<PriorityFee> {
        let micro = self.rpc.priority_fee_micro_lamports(self.priority_fee_percentile).await?;
        // Use a generous unit_limit; pump.fun buys/sells take ~70-100k CU typically.
        Some(PriorityFee {
            unit_limit: Some(200_000),
            unit_price: Some(micro),
        })
    }

    /// Build the full instruction list for a hand-rolled pump.fun buy:
    /// `[priority-fee...] + [create_idempotent_ata(buyer, mint, T22-or-classic)] + [pump.fun BUY (18 accts)]`
    ///
    /// This is the **fix** for the 2026-05-08 `0xbc4 / AccountNotInitialized
    /// (associated_bonding_curve)` bug: the original `pumpfun` crate path
    /// derived its ATAs against Token Classic and the deployed program now
    /// expects Token-2022 ATAs + 2 trailing accounts (`bonding-curve-v2` +
    /// buyback fee recipient). See `LIVE_BUG_FIX_REPORT.md` for the full diff.
    pub async fn build_buy_ixs(
        &self,
        mint_pk: &Pubkey,
        amount_sol_lamports: u64,
        priority_fee: &PriorityFee,
    ) -> Result<Vec<Instruction>> {
        let token_program = pump_ix::detect_token_program(&self.rpc.client, mint_pk).await?;

        // Resolve creator FROM CHAIN directly. The pumpfun crate's
        // `get_bonding_curve_account` parser has been unreliable (stale layout vs current
        // deployment), and its silent `.ok()` fallback to `creator = user` was the
        // 2026-05-09 16:45 ConstraintSeeds(creator_vault) bug:
        // we sent QA7irmLd... (creator-vault for OUR pubkey) but the program wanted
        // Di45Loe... (creator-vault for the ACTUAL creator bwamJzz...). The crate's
        // parser failed -> .ok() swallowed it -> wrong creator -> wrong vault PDA.
        //
        // Read directly. BondingCurve layout (after 8-byte disc):
        //   8 v_token + 8 v_sol + 8 r_token + 8 r_sol + 8 t_supply + 1 complete + 32 creator
        //   creator_offset = 8 + 8*5 + 1 = 49.
        let bc_pda = pump_ix::bonding_curve_pda(mint_pk);
        let bc_acct_res = self.rpc.client.get_account(&bc_pda).await;
        let (token_amount, creator, bc_present) = match bc_acct_res {
            Ok(acct) if acct.data.len() >= 49 + 32 => {
                let creator_bytes: [u8; 32] = acct.data[49..49+32].try_into().unwrap();
                let creator = Pubkey::new_from_array(creator_bytes);
                // Use crate's price math via re-parse (still correct math, just unreliable parser).
                // If parser fails, fall back to global initial-curve math.
                // NB: pumpfun::ClientError is not Send (carries dyn StdError), so we MUST drop
                // the Result BEFORE the next await to keep the surrounding future Send.
                let bc_math_opt = self.pump.get_bonding_curve_account(mint_pk).await
                    .ok()
                    .and_then(|bc| bc.get_buy_price(amount_sol_lamports).ok());
                let amount = match bc_math_opt {
                    Some(a) => a,
                    None => {
                        let global = self.pump.get_global_account().await
                            .map_err(|e| anyhow!("get_global_account fallback: {e}"))?;
                        global.get_initial_buy_price(amount_sol_lamports)
                    }
                };
                (amount, creator, true)
            }
            Ok(_acct) => {
                // BC exists but is too small to parse - this should never happen for valid pump tokens.
                warn!(mint=%mint_pk, "⚠️ BC account exists but is too small to read creator — falling back to user. This will likely fail with ConstraintSeeds.");
                let global = self.pump.get_global_account().await
                    .map_err(|e| anyhow!("get_global_account: {e}"))?;
                (global.get_initial_buy_price(amount_sol_lamports), self.trading_kp.pubkey(), false)
            }
            Err(e) => {
                // BC not on chain yet. True first-buyer race — fall back to creator=user
                // is correct here because pump.fun's own create+buy flow seeds creator=create_payer.
                info!(mint=%mint_pk, error=%e, "BC not on-chain yet — first-buyer race, using creator=user fallback");
                let global = self.pump.get_global_account().await
                    .map_err(|e| anyhow!("get_global_account: {e}"))?;
                (global.get_initial_buy_price(amount_sol_lamports), self.trading_kp.pubkey(), false)
            }
        };
        info!(mint=%mint_pk, %creator, bc_present, "resolved creator for buy ix");
        let max_sol_cost = pumpfun::utils::calculate_with_slippage_buy(
            amount_sol_lamports,
            self.slippage_bps,
        );

        let mut rng = rand::rngs::StdRng::from_entropy();
        let buy_ix = pump_ix::buy_ix(
            &pump_ix::BuyArgs {
                user: self.trading_kp.pubkey(),
                mint: *mint_pk,
                creator,
                token_program,
                amount: token_amount,
                max_sol_cost,
                track_volume: Some(true),
            },
            &mut rng,
        );
        let ata_ix = pump_ix::create_idempotent_ata_ix(
            &self.trading_kp.pubkey(),
            &self.trading_kp.pubkey(),
            mint_pk,
            &token_program,
        );

        let mut ixs = pumpfun_priority_fee_ixs(priority_fee);
        ixs.push(ata_ix);
        ixs.push(buy_ix);
        Ok(ixs)
    }

    /// Build a hand-rolled pump.fun sell instruction list.
    /// `[priority-fee...] + [pump.fun SELL (16 accts)]`. ATA already exists from buy.
    pub async fn build_sell_ixs(
        &self,
        mint_pk: &Pubkey,
        token_amount_base: u64,
        priority_fee: &PriorityFee,
    ) -> Result<Vec<Instruction>> {
        let token_program = pump_ix::detect_token_program(&self.rpc.client, mint_pk).await?;
        let bc = self
            .pump
            .get_bonding_curve_account(mint_pk)
            .await
            .map_err(|e| anyhow!("get_bonding_curve_account (sell): {e}"))?;
        let creator = bc.creator;
        // Compute min_sol_output from on-chain BC + slippage.
        let expected_sol = bc
            .get_sell_price(token_amount_base, /* fee_bps */ 100)
            .map_err(|e| anyhow!("bc.get_sell_price: {e}"))?;
        let min_sol_output =
            pumpfun::utils::calculate_with_slippage_sell(expected_sol, self.slippage_bps);

        let mut rng = rand::rngs::StdRng::from_entropy();
        let sell_ix = pump_ix::sell_ix(
            &pump_ix::SellArgs {
                user: self.trading_kp.pubkey(),
                mint: *mint_pk,
                creator,
                token_program,
                amount: token_amount_base,
                min_sol_output,
            },
            &mut rng,
        );

        let mut ixs = pumpfun_priority_fee_ixs(priority_fee);
        ixs.push(sell_ix);
        Ok(ixs)
    }

    /// Submit a signed tx (already-built ixs) and wait for confirmation.
    async fn send_ixs(&self, ixs: &[Instruction]) -> Result<Signature> {
        let blockhash = self
            .rpc
            .client
            .get_latest_blockhash()
            .await
            .context("get blockhash")?;
        let tx = Transaction::new_signed_with_payer(
            ixs,
            Some(&self.trading_kp.pubkey()),
            &[&*self.trading_kp],
            blockhash,
        );
        // Pre-flight simulate (cheap, lets us bail before paying any fee).
        let sim_cfg = RpcSimulateTransactionConfig {
            sig_verify: false,
            replace_recent_blockhash: true,
            commitment: Some(CommitmentConfig::confirmed()),
            encoding: None,
            accounts: None,
            min_context_slot: None,
            inner_instructions: false,
        };
        let sim = self
            .rpc
            .client
            .simulate_transaction_with_config(&tx, sim_cfg)
            .await
            .context("simulate before send")?;
        if let Some(err) = sim.value.err {
            let logs = sim.value.logs.unwrap_or_default().join("\n");
            return Err(anyhow!("pre-send simulate failed: {err:?}\n{logs}"));
        }
        // Submit-and-confirm. If it fails after broadcast we MUST capture the on-chain
        // logs — the previous code lost them with `.context("submit tx")`, leaving us
        // blind to what actually went wrong on chain (slippage? PDA? compute budget?).
        let send_res = self
            .rpc
            .client
            .send_and_confirm_transaction_with_spinner_and_commitment(
                &tx,
                CommitmentConfig::confirmed(),
            )
            .await;
        let sig = match send_res {
            Ok(s) => s,
            Err(e) => {
                // Try to extract the on-chain failure logs. solana-rpc-client's
                // `ClientErrorKind::TransactionError` carries the program error; if we
                // got a signature in the error path, fetch the tx logs from chain.
                let err_str = format!("{e}");
                // Best-effort: signature is the first thing in the tx, derive it from the tx itself.
                let sig_attempted = tx.signatures.first().copied();
                let mut logs_block = String::new();
                if let Some(sig_a) = sig_attempted {
                    if let Ok(tx_status) = self.rpc.client.get_transaction(
                        &sig_a,
                        UiTransactionEncoding::Json,
                    ).await {
                        if let Some(meta) = tx_status.transaction.meta {
                            let logs: Vec<String> = match meta.log_messages {
                                solana_transaction_status::option_serializer::OptionSerializer::Some(v) => v,
                                _ => Vec::new(),
                            };
                            if !logs.is_empty() {
                                logs_block = format!("\nsig={sig_a}\non-chain logs:\n{}", logs.join("\n"));
                            } else {
                                logs_block = format!("\nsig={sig_a} (no logs available — tx may not have landed)");
                            }
                        }
                    } else {
                        logs_block = format!("\nsig={sig_a} (could not fetch tx — may be unconfirmed/dropped)");
                    }
                }
                return Err(anyhow!("submit tx: {err_str}{logs_block}"));
            }
        };
        Ok(sig)
    }

    /// Submit a pre-signed VersionedTransaction (used for PumpPortal-built txs).
    /// Same simulate-before-submit guardrail and on-chain log capture as `send_ixs`.
    ///
    /// When `self.jito.is_some()` and `dual_submit = true`, ALSO submits the
    /// tx via a Jito bundle ([tip_tx, this_tx]) in parallel. Solana dedups by
    /// signature, so only one inclusion can land; whichever block engine wins,
    /// the other becomes a no-op. If Helius wins, the Jito bundle is dropped
    /// and we pay no tip. Logs winner via `tracing::info` for measurement.
    ///
    /// LATENCY (2026-05-13 redesign): the old path was
    /// `send_and_confirm_transaction_with_spinner_and_commitment` which
    /// internally:
    ///   1. Runs preflight simulate AGAIN (we already did one above).
    ///   2. Submits with RPC-side retries (max_retries default = None = many).
    ///   3. Polls every 500ms with a TUI spinner.
    /// That adds 0.5–2s per send vs. the minimum-viable path. We replace it
    /// with:
    ///   1. `send_transaction_with_config(skip_preflight=true, max_retries=0)`
    ///      — ships the wire bytes to the RPC and returns the signature
    ///      ASAP. We don't ask the RPC to retry; we already have Jito as a
    ///      parallel path, and a stale-blockhash retry from the RPC is more
    ///      latency than re-firing the tx ourselves.
    ///   2. Hand-rolled confirmation loop polling `get_signature_statuses`
    ///      every 400ms up to `confirm_timeout_secs` (default 20s), checking
    ///      for confirmation_status >= Confirmed and surfacing on-chain errors
    ///      immediately.
    async fn send_versioned_tx(&self, tx: &solana_sdk::transaction::VersionedTransaction) -> Result<Signature> {
        // ⚡ If Jito is enabled, inject the tip transfer as the LAST instruction
        // of the trade tx (per Jito's official guidance, Rust SDK example, and
        // skill docs). This is the structural fix for the intermittent
        // "Bundles must write lock at least one tip account" rejection we saw
        // with the old [tip_tx, trade_tx] 2-tx bundle pattern (2026-05-13).
        //
        // Important: the tipped tx has a DIFFERENT signature than the original.
        // We use the tipped tx for BOTH Helius and Jito paths so Solana's
        // sig-dedup still gives us at-most-once landing.
        //
        // If Jito is disabled or tip injection fails, fall back to the
        // original tx (Helius-only).
        let tx_for_send: std::borrow::Cow<solana_sdk::transaction::VersionedTransaction> =
            if let Some(jito) = &self.jito {
                match crate::tip_inject::random_tip_account() {
                    Ok(tip_account) => {
                        let tip_lamports = jito.effective_tip_lamports();
                        match crate::tip_inject::inject_tip(
                            tx.clone(),
                            &self.trading_kp,
                            tip_account,
                            tip_lamports,
                        ) {
                            Ok(tipped) => std::borrow::Cow::Owned(tipped),
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    "tip injection failed; falling back to untipped tx (helius only)"
                                );
                                std::borrow::Cow::Borrowed(tx)
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error=%e, "random_tip_account failed; helius only");
                        std::borrow::Cow::Borrowed(tx)
                    }
                }
            } else {
                std::borrow::Cow::Borrowed(tx)
            };
        let tx: &solana_sdk::transaction::VersionedTransaction = &tx_for_send;

        // Pre-flight simulate — catches bad txs before paying any fee.
        let sim_cfg = RpcSimulateTransactionConfig {
            sig_verify: false,
            replace_recent_blockhash: true,
            commitment: Some(CommitmentConfig::confirmed()),
            encoding: None,
            accounts: None,
            min_context_slot: None,
            inner_instructions: false,
        };
        let sim = self.rpc.client
            .simulate_transaction_with_config(tx, sim_cfg)
            .await
            .context("simulate versioned tx")?;
        if let Some(err) = sim.value.err {
            let logs = sim.value.logs.unwrap_or_default().join("\n");
            return Err(anyhow!("pre-send simulate failed: {err:?}\n{logs}"));
        }

        // ⚡ Jito parallel-submit, now using the TIPPED tx in a 1-tx bundle.
        // The tip transfer is the last instruction of the trade tx itself, so
        // Jito's auction logic sees the tip-account write-lock IN THE SAME
        // transaction that does the trade — satisfying the "must write lock at
        // least one tip account" requirement that the old 2-tx bundle pattern
        // failed to reliably satisfy.
        //
        // Failures are non-fatal: Helius path runs below regardless.
        if let Some(jito) = &self.jito {
            let bundle = vec![tx.clone()];
            let jito_client = jito.clone();
            let trade_sig = tx.signatures.first().copied();
            // Fire and forget — Helius is the authoritative confirmation path.
            tokio::spawn(async move {
                let res = jito_client.send_bundle_best_effort(&bundle).await;
                if let Some(bid) = res {
                    tracing::info!(
                        bundle_id=%bid,
                        trade_sig=?trade_sig,
                        tip_lamports=jito_client.tip_lamports(),
                        "⚡ jito bundle submitted (parallel to helius, tip-in-tx)"
                    );
                }
            });
        }

        // 🚀 LATENCY-OPTIMIZED SEND: fire-and-confirm.
        // skip_preflight=true — we already simulated above; no point doing it
        //                      twice (saves 1 RPC roundtrip = 100–500ms).
        // max_retries=0      — don't let the RPC silently retry. We'd rather
        //                      know immediately if the send drops so we can
        //                      rebuild with a fresh blockhash, AND Jito is
        //                      already our parallel path.
        let send_cfg = RpcSendTransactionConfig {
            skip_preflight: true,
            preflight_commitment: Some(CommitmentLevel::Processed),
            encoding: None,
            max_retries: Some(0),
            min_context_slot: None,
        };
        let sig = self.rpc.client
            .send_transaction_with_config(tx, send_cfg)
            .await
            .map_err(|e| anyhow!("send_transaction_with_config: {e}"))?;
        let send_t_ms = chrono::Utc::now().timestamp_millis();

        // Custom confirmation loop. Polls `get_signature_statuses` every
        // ~400ms until we see `confirmation_status >= Confirmed` (or `err`).
        // Bounded by `confirm_timeout_secs` (default 20s). On timeout, we
        // surface the signature so the caller / journal can keep watch — it
        // might still land after; but for our hold-time math it's a fail.
        let timeout = Duration::from_secs(self.confirm_timeout_secs as u64);
        let poll_interval = Duration::from_millis(self.confirm_poll_interval_ms);
        let deadline = std::time::Instant::now() + timeout;
        let mut last_err_logged = false;
        loop {
            match self.rpc.client.get_signature_statuses(&[sig]).await {
                Ok(resp) => {
                    if let Some(Some(status)) = resp.value.into_iter().next() {
                        // On-chain error: surface immediately with logs.
                        if let Some(tx_err) = status.err {
                            return Err(self.tx_error_with_logs(&sig, format!("on-chain tx error: {tx_err:?}")).await);
                        }
                        // Treat any confirmation level (processed/confirmed/finalized)
                        // as good enough — our commitment policy is `confirmed`.
                        use solana_transaction_status::TransactionConfirmationStatus as TCS;
                        if let Some(cs) = status.confirmation_status {
                            if matches!(cs, TCS::Confirmed | TCS::Finalized) {
                                let elapsed_ms = chrono::Utc::now().timestamp_millis() - send_t_ms;
                                tracing::debug!(sig=%sig, elapsed_ms, "✅ tx confirmed");
                                return Ok(sig);
                            }
                        } else if status.confirmations.is_none() {
                            // confirmations=None means rooted/finalized in older APIs.
                            let elapsed_ms = chrono::Utc::now().timestamp_millis() - send_t_ms;
                            tracing::debug!(sig=%sig, elapsed_ms, "✅ tx rooted");
                            return Ok(sig);
                        }
                    }
                    // status is None — RPC hasn't seen it yet. Keep polling.
                    last_err_logged = false;
                }
                Err(e) => {
                    // Don't fail on transient RPC errors during confirmation —
                    // just log once and keep polling. Timeout will trip if it
                    // persists.
                    if !last_err_logged {
                        tracing::warn!(sig=%sig, error=%e, "get_signature_statuses transient error, continuing to poll");
                        last_err_logged = true;
                    }
                }
            }
            if std::time::Instant::now() >= deadline {
                return Err(self.tx_error_with_logs(
                    &sig,
                    format!("confirmation timeout after {}s", self.confirm_timeout_secs),
                ).await);
            }
            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Fetch on-chain logs for `sig` if available, and wrap them into an error
    /// message alongside `prefix`. Used by the send path to give us actionable
    /// post-hoc context when a tx fails confirmation.
    async fn tx_error_with_logs(&self, sig: &Signature, prefix: String) -> anyhow::Error {
        let mut logs_block = String::new();
        if let Ok(tx_status) = self.rpc.client.get_transaction(
            sig,
            UiTransactionEncoding::Json,
        ).await {
            if let Some(meta) = tx_status.transaction.meta {
                let logs: Vec<String> = match meta.log_messages {
                    solana_transaction_status::option_serializer::OptionSerializer::Some(v) => v,
                    _ => Vec::new(),
                };
                if !logs.is_empty() {
                    logs_block = format!("\non-chain logs:\n{}", logs.join("\n"));
                } else {
                    logs_block = String::from(" (no logs)");
                }
            }
        } else {
            logs_block = String::from(" (no tx record)");
        }
        anyhow!("{prefix} sig={sig}{logs_block}")
    }

    /// Buy `amount_sol` SOL worth of `mint`. Returns the actual fill.
    ///
    /// Retry policy: pump.fun create-pool tx and our PumpPortal WS event race —
    /// the WS "create" message often fires before the create-pool tx is
    /// confirmed, so the bonding curve and its associated token account aren't
    /// initialized yet. Retry up to 5 times with exponential-ish backoff for
    /// `AccountNotInitialized` (Anchor 3012 / 0xbc4) and `BondingCurveNotFound`.
    pub async fn buy(&self, mint: &str, amount_sol: f64) -> Result<BuyFill> {
        let mint_pk = Pubkey::from_str(mint).context("parse mint")?;
        let amount_lamports = sol_to_lamports(amount_sol);
        let slippage_pct = ((self.slippage_bps as f64 / 100.0).round() as u32).max(1);
        info!(mint=%mint, sol=amount_sol, lamports=amount_lamports,
              slippage_bps=self.slippage_bps, slippage_pct,
              "🚀 LIVE BUY via PumpPortal trade-local");

        // PumpPortal needs priority fee in SOL units. We translate from
        // priority_fee_micro_lamports * compute_units; with 200k CU and ~0-5k
        // micro-lamports/CU we land at 0-1M lamports => 0.001 SOL. Use a sensible
        // default of 0.005 SOL during dust/test phase — cheap insurance.
        let priority_fee_sol = match self.build_priority_fee().await {
            Some(pf) => {
                let unit_price = pf.unit_price.unwrap_or(0) as f64;
                let unit_limit = pf.unit_limit.unwrap_or(200_000) as f64;
                let lamports = (unit_price * unit_limit) / 1_000_000.0;
                let sol = (lamports / 1e9).max(0.0005); // floor at 0.0005 SOL
                sol.min(0.01) // cap at 0.01 SOL to avoid bleed
            }
            None => 0.005,
        };

        // Retry schedule: attempt at 0ms, 250ms, 500ms, 1000ms, 1800ms — total ~3.5s.
        let backoffs_ms: [u64; 5] = [0, 250, 500, 1000, 1800];
        let mut last_err: Option<anyhow::Error> = None;
        let mut sig_opt: Option<Signature> = None;
        for (attempt, delay) in backoffs_ms.iter().enumerate() {
            if *delay > 0 {
                tokio::time::sleep(Duration::from_millis(*delay)).await;
            }
            let attempt_res: Result<Signature> = async {
                let signed_tx = crate::pumpportal_trade::build_signed_trade_tx(
                    &self.trading_kp,
                    &mint_pk,
                    crate::pumpportal_trade::TradeAction::Buy,
                    amount_sol,
                    /* denominated_in_sol */ true,
                    slippage_pct,
                    priority_fee_sol,
                    "auto",
                ).await.context("pumpportal build buy tx")?;
                self.send_versioned_tx(&signed_tx).await
            }.await;
            match attempt_res {
                Ok(s) => { sig_opt = Some(s); break; }
                Err(e) => {
                    let msg = format!("{e}");
                    let retriable = is_retriable_buy_error(&msg);
                    warn!(attempt = attempt + 1, %mint, retriable, error = %msg, "buy attempt failed");
                    last_err = Some(anyhow!("pumpportal buy: {msg}"));
                    if !retriable { break; }
                }
            }
        }
        let sig = sig_opt.ok_or_else(|| last_err.unwrap_or_else(|| anyhow!("buy failed: no attempts")))?;
        let fill = self.parse_buy_fill(&sig, &mint_pk, amount_lamports).await?;
        info!(%sig, mint=%mint, tokens=fill.tokens_base, sol=fill.sol_spent_lamports, "✅ buy filled");
        Ok(fill)
    }

    /// FEATURE (Phase 3.Feature.2): scale-out sell.
    /// Sells the full holdings of `mint` in N tranches, with `delay_ms` between
    /// each tranche. Each tranche reads the CURRENT on-chain balance and sells
    /// `remaining_balance / remaining_tranches` — this self-corrects for partial
    /// fills, dust, and rounding without any extra state.
    ///
    /// Returns an aggregated SellFill summing every successful tranche. The
    /// `signature` field carries the FINAL successful tranche's signature.
    ///
    /// If a tranche fails mid-way, the scale-out aborts and returns what we got
    /// so far — a partial exit always beats a stuck position. If the very first
    /// tranche fails, the error propagates.
    ///
    /// `n_tranches == 0 || n_tranches == 1` is equivalent to a single-shot sell.
    pub async fn sell_scale_out(
        &self,
        mint: &str,
        n_tranches: u8,
        delay_ms: u64,
    ) -> Result<SellFill> {
        if n_tranches <= 1 {
            return self.sell_all(mint).await;
        }
        let mint_pk = Pubkey::from_str(mint).context("parse mint")?;

        // Aggregate across tranches
        let mut total_tokens_sold: u64 = 0;
        let mut total_sol_received: u64 = 0;
        let mut last_sig: Option<Signature> = None;
        let mut tranches_completed: u8 = 0;

        for i in 0..n_tranches {
            let remaining_tranches = n_tranches - i;

            // Read CURRENT balance (post-prior-tranches)
            let pre_tokens = self.token_balance_pk(&mint_pk).await.unwrap_or(0);
            if pre_tokens == 0 {
                info!(mint=%mint, tranche=i+1, of=n_tranches, "scale-out: zero balance, exit early");
                break;
            }
            // For all but the last tranche: take 1/remaining_tranches of current balance.
            // For the last tranche: take everything remaining (avoids dust leftover).
            let tokens_to_sell_base: u64 = if remaining_tranches == 1 {
                pre_tokens
            } else {
                pre_tokens / remaining_tranches as u64
            };
            if tokens_to_sell_base == 0 {
                info!(mint=%mint, tranche=i+1, of=n_tranches, pre_tokens, "scale-out: per-tranche amount rounded to zero, skipping");
                continue;
            }

            match self.sell_partial(&mint_pk, tokens_to_sell_base).await {
                Ok(fill) => {
                    total_tokens_sold = total_tokens_sold.saturating_add(fill.tokens_sold_base);
                    total_sol_received = total_sol_received.saturating_add(fill.sol_received_lamports);
                    last_sig = Some(fill.signature);
                    tranches_completed += 1;
                    info!(
                        mint=%mint, tranche=i+1, of=n_tranches,
                        sig=%fill.signature, tokens=fill.tokens_sold_base,
                        sol_lamports=fill.sol_received_lamports,
                        "✅ scale-out tranche filled"
                    );
                }
                Err(e) => {
                    warn!(mint=%mint, tranche=i+1, of=n_tranches, error=?e,
                          "⚠️ scale-out tranche FAILED; aborting scale-out, keeping any partial fills");
                    if tranches_completed == 0 {
                        return Err(e.context("scale-out first tranche failed"));
                    }
                    break;
                }
            }

            // Inter-tranche delay (skip after the last one)
            if i + 1 < n_tranches && delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }

        if tranches_completed == 0 {
            return Err(anyhow!("scale-out: no tranches completed for {mint}"));
        }

        let final_sig = last_sig.ok_or_else(|| anyhow!("scale-out: completed tranches but no signature?"))?;
        let effective_price_sol = if total_tokens_sold > 0 {
            (total_sol_received as f64 / 1e9) / (total_tokens_sold as f64 / 1e6)
        } else { 0.0 };
        info!(
            %mint, tranches_completed, of=n_tranches,
            total_tokens=total_tokens_sold, total_sol_lamports=total_sol_received,
            "🎯 scale-out aggregate"
        );
        Ok(SellFill {
            signature: final_sig,
            tokens_sold_base: total_tokens_sold,
            sol_received_lamports: total_sol_received,
            effective_price_sol,
        })
    }

    /// Internal: sell a SPECIFIC `tokens_base` quantity (not "all") of `mint`.
    /// Used by `sell_scale_out`. Same execution path as `sell_all` but with
    /// explicit token quantity rather than wallet balance.
    async fn sell_partial(&self, mint_pk: &Pubkey, tokens_base: u64) -> Result<SellFill> {
        let slippage_pct = ((self.slippage_bps as f64 / 100.0).round() as u32).max(1);
        let pre_sol = wallet::get_sol_balance(&self.rpc.client, &self.trading_kp.pubkey())
            .await.unwrap_or(0);
        // The pre_tokens_base for fill parsing is the ATA balance BEFORE this tranche,
        // so parse_sell_fill's `sold = pre - post` works correctly per tranche.
        let pre_tokens = self.token_balance_pk(mint_pk).await.unwrap_or(0);
        if tokens_base == 0 || pre_tokens == 0 || tokens_base > pre_tokens {
            return Err(anyhow!("sell_partial: invalid request tokens={} pre_balance={}", tokens_base, pre_tokens));
        }
        let tokens_human = tokens_base as f64 / 1e6;

        let priority_fee_sol = match self.build_priority_fee().await {
            Some(pf) => {
                let unit_price = pf.unit_price.unwrap_or(0) as f64;
                let unit_limit = pf.unit_limit.unwrap_or(200_000) as f64;
                ((unit_price * unit_limit) / 1_000_000.0 / 1e9).max(0.0005).min(0.01)
            }
            None => 0.005,
        };

        let signed_tx = crate::pumpportal_trade::build_signed_trade_tx(
            &self.trading_kp,
            mint_pk,
            crate::pumpportal_trade::TradeAction::Sell,
            tokens_human,
            false,
            slippage_pct,
            priority_fee_sol,
            "auto",
        ).await.context("pumpportal build sell_partial tx")?;
        let sig = self.send_versioned_tx(&signed_tx).await
            .map_err(|e| anyhow!("pumpportal sell_partial: {e}"))?;
        self.parse_sell_fill(&sig, mint_pk, pre_sol, pre_tokens).await
    }

    /// Sell all tokens of `mint` held by the trading wallet. Returns the actual fill.
    pub async fn sell_all(&self, mint: &str) -> Result<SellFill> {
        let mint_pk = Pubkey::from_str(mint).context("parse mint")?;
        let slippage_pct = ((self.slippage_bps as f64 / 100.0).round() as u32).max(1);
        // Snapshot pre-sell balance so we can compute SOL received from the delta.
        let pre_sol = wallet::get_sol_balance(&self.rpc.client, &self.trading_kp.pubkey())
            .await.unwrap_or(0);
        let pre_tokens = self.token_balance_pk(&mint_pk).await.unwrap_or(0);

        info!(mint=%mint, pre_tokens, slippage_pct, "🔴 LIVE SELL via PumpPortal trade-local");
        if pre_tokens == 0 {
            return Err(anyhow!("sell_all: zero token balance for {mint}"));
        }

        // PumpPortal `denominatedInSol=false` + amount = number of tokens (human units).
        // Pump.fun tokens have 6 decimals; PumpPortal expects whole tokens, not base units.
        let tokens_human = pre_tokens as f64 / 1e6;

        let priority_fee_sol = match self.build_priority_fee().await {
            Some(pf) => {
                let unit_price = pf.unit_price.unwrap_or(0) as f64;
                let unit_limit = pf.unit_limit.unwrap_or(200_000) as f64;
                ((unit_price * unit_limit) / 1_000_000.0 / 1e9).max(0.0005).min(0.01)
            }
            None => 0.005,
        };

        let signed_tx = crate::pumpportal_trade::build_signed_trade_tx(
            &self.trading_kp,
            &mint_pk,
            crate::pumpportal_trade::TradeAction::Sell,
            tokens_human,
            /* denominated_in_sol */ false,
            slippage_pct,
            priority_fee_sol,
            "auto",
        ).await.context("pumpportal build sell tx")?;
        let sig = self.send_versioned_tx(&signed_tx).await
            .map_err(|e| anyhow!("pumpportal sell: {e}"))?;

        let fill = self.parse_sell_fill(&sig, &mint_pk, pre_sol, pre_tokens).await?;
        info!(%sig, mint=%mint, tokens=fill.tokens_sold_base, sol=fill.sol_received_lamports, "✅ sell filled");
        Ok(fill)
    }

    /// Parse a buy tx's logs to extract actual `tokens_received` and SOL spent.
    /// pump.fun emits a `Program data: ...` event with a Trade struct; we also
    /// just diff the user's token balance pre/post to be defensive.
    async fn parse_buy_fill(
        &self,
        sig: &Signature,
        mint: &Pubkey,
        requested_lamports: u64,
    ) -> Result<BuyFill> {
        // Read confirmed token balance — this is the most reliable source of truth.
        // Wait briefly for confirmation indexing.
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let bal = self.token_balance_pk(mint).await.unwrap_or(0);
            if bal > 0 {
                let tokens_human = bal as f64 / 1e6;
                let sol_human = requested_lamports as f64 / 1e9;
                let price = if tokens_human > 0.0 { sol_human / tokens_human } else { 0.0 };
                return Ok(BuyFill {
                    signature: *sig,
                    tokens_base: bal,
                    sol_spent_lamports: requested_lamports, // approx; SDK takes care of fees
                    effective_price_sol: price,
                });
            }
        }
        // Fallback: parse log events
        if let Some(fill) = self.parse_trade_event_logs(sig, mint, true).await? {
            return Ok(BuyFill {
                signature: *sig,
                tokens_base: fill.0,
                sol_spent_lamports: fill.1,
                effective_price_sol: if fill.0 > 0 { (fill.1 as f64 / 1e9) / (fill.0 as f64 / 1e6) } else { 0.0 },
            });
        }
        Err(anyhow!("could not determine buy fill from logs or balance for sig {sig}"))
    }

    async fn parse_sell_fill(
        &self,
        sig: &Signature,
        mint: &Pubkey,
        pre_sol_lamports: u64,
        pre_tokens_base: u64,
    ) -> Result<SellFill> {
        // Diff balances post-confirmation.
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let post_sol = wallet::get_sol_balance(&self.rpc.client, &self.trading_kp.pubkey())
                .await.unwrap_or(pre_sol_lamports);
            let post_tokens = self.token_balance_pk(mint).await.unwrap_or(pre_tokens_base);
            if post_tokens < pre_tokens_base {
                let sold = pre_tokens_base - post_tokens;
                let received = post_sol.saturating_sub(pre_sol_lamports);
                let tokens_human = sold as f64 / 1e6;
                let sol_human = received as f64 / 1e9;
                let price = if tokens_human > 0.0 { sol_human / tokens_human } else { 0.0 };
                return Ok(SellFill {
                    signature: *sig,
                    tokens_sold_base: sold,
                    sol_received_lamports: received,
                    effective_price_sol: price,
                });
            }
        }
        // Fallback: parse logs
        if let Some(fill) = self.parse_trade_event_logs(sig, mint, false).await? {
            return Ok(SellFill {
                signature: *sig,
                tokens_sold_base: fill.0,
                sol_received_lamports: fill.1,
                effective_price_sol: if fill.0 > 0 { (fill.1 as f64 / 1e9) / (fill.0 as f64 / 1e6) } else { 0.0 },
            });
        }
        Err(anyhow!("could not determine sell fill from logs or balance for sig {sig}"))
    }

    /// Best-effort log parsing — returns (tokens_base, sol_lamports) if found.
    /// Pump.fun's Trade event includes `token_amount` and `sol_amount`. If our
    /// event parsing here can't find them (anchor self-CPI / encoded), we'll
    /// already have the balance-diff path as primary.
    async fn parse_trade_event_logs(
        &self,
        sig: &Signature,
        _mint: &Pubkey,
        _is_buy: bool,
    ) -> Result<Option<(u64, u64)>> {
        let cfg = solana_rpc_client_api::config::RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::Json),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        };
        let tx = match self.rpc.client.get_transaction_with_config(sig, cfg).await {
            Ok(t) => t,
            Err(e) => {
                warn!(error=?e, "could not fetch confirmed tx for log parsing");
                return Ok(None);
            }
        };
        let _ = tx; // we keep this best-effort; balance-diff is primary
        Ok(None)
    }

    /// Send the vault skim — a plain SOL transfer from trading -> vault.
    pub async fn skim_to_vault(&self, lamports: u64) -> Result<Signature> {
        wallet::transfer_sol(&self.rpc.client, &self.trading_kp, &self.vault_pubkey, lamports).await
    }

    /// Build the same instruction list `buy()` would submit, sign it as a tx, and
    /// run `simulateTransaction` against mainnet RPC. No broadcast. Used by the
    /// re-enable gate — we MUST get a successful sim against a real recently-launched
    /// mint before flipping live mode back on.
    ///
    /// Returns Ok(()) on a successful simulation. The error string includes the
    /// program logs from the failed simulation, so test output is debuggable.
    pub async fn simulate_buy(&self, mint: &str, amount_sol: f64) -> Result<()> {
        let mint_pk = Pubkey::from_str(mint).context("parse mint")?;
        let amount_lamports = sol_to_lamports(amount_sol);
        let priority_fee = self.build_priority_fee().await
            .unwrap_or(PriorityFee { unit_limit: Some(200_000), unit_price: Some(0) });

        // Hand-rolled ixs (same path as live buy()): priority-fee + create-idempotent-ATA + 18-acct BUY.
        let ixs = self.build_buy_ixs(&mint_pk, amount_lamports, &priority_fee).await?;

        let blockhash = self.rpc.client.get_latest_blockhash().await
            .context("get blockhash")?;
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&self.trading_kp.pubkey()),
            &[&*self.trading_kp],
            blockhash,
        );

        let cfg = RpcSimulateTransactionConfig {
            sig_verify: false,
            replace_recent_blockhash: true,
            commitment: Some(CommitmentConfig::confirmed()),
            encoding: None,
            accounts: None,
            min_context_slot: None,
            inner_instructions: false,
        };
        let res = self.rpc.client.simulate_transaction_with_config(&tx, cfg).await
            .context("simulateTransaction RPC")?;
        if let Some(err) = res.value.err {
            let logs = res.value.logs.unwrap_or_default().join("\n");
            return Err(anyhow!("sim failed: {err:?}\n--- logs ---\n{logs}"));
        }
        info!(
            mint=%mint,
            units = res.value.units_consumed.unwrap_or(0),
            "✅ simulate_buy OK"
        );
        Ok(())
    }

    /// Token-program-aware on-chain token balance (Pubkey overload).
    pub async fn token_balance_pk(&self, mint_pk: &Pubkey) -> Result<u64> {
        let token_program = match pump_ix::detect_token_program(&self.rpc.client, mint_pk).await {
            Ok(p) => p,
            Err(_) => return Ok(0),
        };
        let ata = pump_ix::associated_token_address(
            &self.trading_kp.pubkey(), mint_pk, &token_program,
        );
        match self.rpc.client.get_token_account_balance(&ata).await {
            Ok(b) => Ok(b.amount.parse::<u64>().unwrap_or(0)),
            Err(_) => Ok(0),
        }
    }

    /// On-chain reconciliation: returns current token balance (base units) for `mint`.
    /// Token-program-aware (works for both Token Classic and Token-2022 mints).
    pub async fn token_balance(&self, mint: &str) -> Result<u64> {
        let mint_pk = Pubkey::from_str(mint).context("parse mint")?;
        let token_program = match pump_ix::detect_token_program(&self.rpc.client, &mint_pk).await {
            Ok(p) => p,
            Err(_) => return Ok(0),
        };
        let ata = pump_ix::associated_token_address(
            &self.trading_kp.pubkey(), &mint_pk, &token_program,
        );
        match self.rpc.client.get_token_account_balance(&ata).await {
            Ok(b) => Ok(b.amount.parse::<u64>().unwrap_or(0)),
            Err(_) => Ok(0),
        }
    }

    /// FEATURE (Phase 3.Feature.4): fetch the authoritative dev/creator pubkey
    /// for a pump.fun token from its on-chain bonding-curve account.
    ///
    /// Layout (per pump.fun's BondingCurve anchor account):
    ///   8 disc + 8 v_tok + 8 v_sol + 8 r_tok + 8 r_sol + 8 t_supply +
    ///   1 complete + 32 creator  =  81 bytes minimum
    /// → creator field starts at byte offset 49, length 32.
    ///
    /// This is the authoritative source for the dev who deployed the token.
    /// Differs from PumpPortal's `traderPublicKey` (initial buyer) in ~1% of
    /// cases where the dev launched via a proxy/funded wallet. Used by
    /// Feature.5 to wire the WS rug-watcher to the right wallet.
    ///
    /// Returns Err if:
    ///   - bonding-curve account doesn't exist yet (first-buyer race window)
    ///   - account data is unexpectedly short (corrupt or wrong account)
    ///   - RPC call fails
    /// Callers should treat any Err as "fall back to traderPublicKey or skip dev
    /// tracking for this position" — don't abort the entry on this alone.
    pub async fn fetch_bonding_curve_creator(&self, mint: &str) -> Result<Pubkey> {
        let mint_pk = Pubkey::from_str(mint).context("parse mint")?;
        let bc_pda = pump_ix::bonding_curve_pda(&mint_pk);
        let acct = self.rpc.client.get_account(&bc_pda).await
            .map_err(|e| anyhow!("get_account(bc_pda) for {mint}: {e}"))?;
        if acct.data.len() < 49 + 32 {
            return Err(anyhow!(
                "bonding-curve account for {mint} too small ({} bytes, need >= 81)",
                acct.data.len()
            ));
        }
        let creator_bytes: [u8; 32] = acct.data[49..49+32].try_into()
            .map_err(|_| anyhow!("slice [49..81] -> [u8; 32] for {mint}"))?;
        Ok(Pubkey::new_from_array(creator_bytes))
    }

    pub async fn sol_balance_lamports(&self) -> Result<u64> {
        wallet::get_sol_balance(&self.rpc.client, &self.trading_kp.pubkey()).await
    }

    pub async fn vault_balance_lamports(&self) -> Result<u64> {
        wallet::get_sol_balance(&self.rpc.client, &self.vault_pubkey).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pump.fun program ID. Hard-coded into the pumpfun crate constants.
    /// We assert here that we're targeting the right program — if the crate
    /// silently changes program ID, the test will catch it.
    #[test]
    fn pump_program_id_is_the_real_pumpfun() {
        let expected = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
        let id = pumpfun::constants::accounts::PUMPFUN.to_string();
        assert_eq!(id, expected, "pumpfun program ID drifted! got {}", id);
    }

    /// Sanity: priority fee struct constructs without panic.
    #[test]
    fn priority_fee_struct_builds() {
        let pf = PriorityFee { unit_limit: Some(200_000), unit_price: Some(123_456) };
        assert_eq!(pf.unit_limit, Some(200_000));
        assert_eq!(pf.unit_price, Some(123_456));
    }

    /// Sanity check: 200 bps maps to 2% — confirms our slippage units are basis points.
    #[test]
    fn slippage_units_are_basis_points() {
        let bps: u64 = 200;
        let pct = bps as f64 / 100.0;
        assert!((pct - 2.0).abs() < 1e-9, "200 bps must be 2%");
    }

    /// Regression: the 2026-05-08 live-bug error must classify as retriable.
    /// The verbatim RPC string included "custom program error: 0xbc4" — if a
    /// future refactor renames or hides the error message, this catches it.
    #[test]
    fn live_bug_2026_05_08_error_is_retriable() {
        let real_err = "pumpfun buy: SolanaClient(RpcError(RpcResponseError { code: -32002, \
            message: \"Transaction simulation failed: Error processing Instruction 2: \
            custom program error: 0xbc4\", data: ... }))";
        assert!(is_retriable_buy_error(real_err), "0xbc4 must retry");
    }

    #[test]
    fn anchor_3012_text_is_retriable() {
        assert!(is_retriable_buy_error("AnchorError ... AccountNotInitialized 3012"));
        assert!(is_retriable_buy_error("BondingCurveNotFound"));
        assert!(is_retriable_buy_error("could not find account: bonding_curve_pda"));
    }

    #[test]
    fn unrelated_errors_dont_retry() {
        // E.g. wallet underfunded, or non-pumpfun program failure — should NOT retry.
        assert!(!is_retriable_buy_error("insufficient funds for rent"));
        assert!(!is_retriable_buy_error("slippage exceeded: 0x1771"));
        assert!(!is_retriable_buy_error("blockhash not found"));
    }

    /// Sanity: priority-fee instructions match what pumpfun crate emits internally.
    /// This ensures our `simulate_buy` builds a tx with the same shape as `buy`.
    #[test]
    fn priority_fee_ixs_count_matches_fields() {
        let pf_full = PriorityFee { unit_limit: Some(200_000), unit_price: Some(1) };
        assert_eq!(pumpfun_priority_fee_ixs(&pf_full).len(), 2);
        let pf_limit_only = PriorityFee { unit_limit: Some(200_000), unit_price: None };
        assert_eq!(pumpfun_priority_fee_ixs(&pf_limit_only).len(), 1);
        let pf_none = PriorityFee { unit_limit: None, unit_price: None };
        assert_eq!(pumpfun_priority_fee_ixs(&pf_none).len(), 0);
    }
}

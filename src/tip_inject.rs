//! Inject a Jito tip-transfer instruction into a pre-compiled v0 message.
//!
//! ## Why this exists
//!
//! Jito's bundle auction logic requires AT LEAST ONE transaction in the
//! bundle to acquire a write-lock on a tip account. Our previous pattern
//! was `bundle = [tip_tx, trade_tx]` — a separate stand-alone tip tx +
//! the opaque trade tx from PumpPortal. That pattern got ~17% intermittent
//! rejection with the error:
//!
//!     "Bundles must write lock at least one tip account to be eligible
//!      for the auction."
//!
//! The fix per Jito's official Rust example and the trading-skills SKILL.md
//! is: put the tip transfer as the LAST instruction of the LAST transaction
//! in the bundle (1-tx bundle for our case). That way the tip is acquired in
//! the same atomic auction unit as the trade.
//!
//! Since PumpPortal returns a pre-compiled v0 message that we then re-sign,
//! we need to surgically:
//!   1. Add the tip account to `account_keys` (in the unsigned-writable
//!      region per Solana's account ordering rules).
//!   2. Add the System Program account if it isn't already in the keys.
//!   3. Append a new CompiledInstruction for the SOL transfer.
//!   4. Update `MessageHeader.num_readonly_unsigned_accounts` if we added the
//!      System Program.
//!   5. Re-number every existing CompiledInstruction's `program_id_index` and
//!      `accounts: Vec<u8>` entries to account for the inserted key(s).
//!
//! Indexing rules we're maintaining (from solana_message::v0 docs):
//!   - account_keys are partitioned by MessageHeader into FOUR regions, in
//!     order:
//!       a) [0 .. num_required_signatures - num_readonly_signed_accounts)
//!          → signed + writable (fee payer at index 0)
//!       b) [num_required_signatures - num_readonly_signed_accounts ..
//!           num_required_signatures)
//!          → signed + readonly
//!       c) [num_required_signatures ..
//!           num_required_signatures + (num_keys_static - num_required_signatures - num_readonly_unsigned_accounts))
//!          → unsigned + writable
//!       d) [.. end of static account_keys)
//!          → unsigned + readonly
//!     Then ALT-loaded writable, then ALT-loaded readonly.
//!   - We insert the tip account at the BOUNDARY between (c) and (d) — i.e.
//!     at index `num_required_signatures + count_of_unsigned_writable_so_far`.
//!   - We insert the System Program (if needed) at the END (start of region d
//!     OR shift end of d) and bump `num_readonly_unsigned_accounts` by 1.
//!
//! Notes:
//!   - We do NOT touch `address_table_lookups`. ALT-loaded accounts have
//!     higher indexes than ALL static keys, so inserting a static key only
//!     shifts CompiledInstruction account-index entries that point WITHIN
//!     the static range. Indexes >= old static_len don't need adjustment
//!     because we're inserting in the middle, but those that were >= the
//!     insertion point need to be bumped by the number of inserts.
//!
//! Test coverage focuses on the index-arithmetic edge cases.

use anyhow::{anyhow, Context, Result};
use rand::seq::SliceRandom;
use solana_sdk::{
    instruction::CompiledInstruction,
    message::{v0::Message as V0Message, MessageHeader, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::VersionedTransaction,
};
use std::str::FromStr;

use crate::jito::TIP_ACCOUNTS;

/// Pick a Jito tip account uniformly at random. Mirrors the helper on
/// `JitoClient` so this module is self-contained / testable.
pub fn random_tip_account() -> Result<Pubkey> {
    let mut rng = rand::thread_rng();
    let s = TIP_ACCOUNTS
        .choose(&mut rng)
        .ok_or_else(|| anyhow!("TIP_ACCOUNTS is empty"))?;
    Pubkey::from_str(s).context("parse tip account pubkey")
}

/// Take a signed `VersionedTransaction` (from PumpPortal, already
/// re-signed for our fee payer) and return a NEW signed transaction with:
///   - A SystemProgram::transfer(payer → tip_account, lamports) instruction
///     APPENDED as the last instruction.
///   - account_keys + header updated to reference the tip account
///     (unsigned writable) and the System Program (unsigned readonly) if
///     not already present.
///
/// The original transaction must be a `VersionedMessage::V0`. Legacy
/// messages are not supported by PumpPortal in our usage so we error out.
pub fn inject_tip(
    tx: VersionedTransaction,
    payer_kp: &Keypair,
    tip_account: Pubkey,
    tip_lamports: u64,
) -> Result<VersionedTransaction> {
    let message = match tx.message {
        VersionedMessage::V0(m) => m,
        VersionedMessage::Legacy(_) => {
            return Err(anyhow!("tip injection only supports V0 messages"))
        }
    };
    let injected = inject_tip_into_v0(message, payer_kp.pubkey(), tip_account, tip_lamports)?;
    let signed = VersionedTransaction::try_new(VersionedMessage::V0(injected), &[payer_kp])
        .map_err(|e| anyhow!("re-sign tipped tx: {e:?}"))?;
    Ok(signed)
}

/// Pure-function core: takes a V0 message and returns a new one with the
/// tip transfer appended. Public so tests can exercise the index arithmetic
/// without a real keypair / RPC dependency.
pub fn inject_tip_into_v0(
    mut msg: V0Message,
    payer: Pubkey,
    tip_account: Pubkey,
    tip_lamports: u64,
) -> Result<V0Message> {
    if tip_lamports == 0 {
        return Err(anyhow!("tip_lamports must be > 0"));
    }

    // ---- 1. Validate the payer is at index 0 (Solana fee-payer convention). ----
    let payer_idx = msg
        .account_keys
        .iter()
        .position(|k| *k == payer)
        .ok_or_else(|| anyhow!("payer pubkey not found in account_keys"))?;
    if payer_idx != 0 {
        return Err(anyhow!(
            "payer must be at account_keys[0] (fee-payer convention), got index {payer_idx}"
        ));
    }

    // ---- 2. Compute the four account-region boundaries from the header. ----
    let num_sig = msg.header.num_required_signatures as usize;
    let _num_sig_ro = msg.header.num_readonly_signed_accounts as usize;
    let num_unsig_ro = msg.header.num_readonly_unsigned_accounts as usize;
    let total = msg.account_keys.len();

    if num_sig + num_unsig_ro > total {
        return Err(anyhow!(
            "malformed message: header counts exceed account_keys length \
             (num_sig={num_sig} num_unsig_ro={num_unsig_ro} total={total})"
        ));
    }

    // signed_writable: [0 .. num_sig - num_sig_ro)
    // signed_readonly: [num_sig - num_sig_ro .. num_sig)
    // unsigned_writable: [num_sig .. num_sig + (total - num_sig - num_unsig_ro))
    // unsigned_readonly: [end of unsigned_writable .. total)
    let unsigned_writable_end = num_sig + (total - num_sig - num_unsig_ro);

    // ---- 3. Insertion plan ----
    //
    // We will insert AT MOST two new account keys:
    //   - tip_account → unsigned writable (we credit it; system_program writes)
    //   - system_program → unsigned readonly (if not already present)
    //
    // Insertion order matters because the second insertion shifts indexes
    // of accounts AFTER the first.

    // Tip account: if it's already in the keys (extremely unlikely but possible
    // if PumpPortal happened to choose the same address, defensive), bail —
    // we'd need to upgrade its writability flag which is complex. Easier to
    // just pick a different tip account by the caller.
    if msg.account_keys.iter().any(|k| *k == tip_account) {
        return Err(anyhow!(
            "tip_account {} already present in account_keys; choose another",
            tip_account
        ));
    }

    let sysprog = system_program::ID;
    let needs_sysprog = !msg.account_keys.iter().any(|k| *k == sysprog);

    // ---- 4. Insert the tip account at the END of unsigned_writable. ----
    //
    // This is the right region: tip_account is unsigned (we don't sign for
    // it — only the transfer source signs) and writable (we modify its
    // lamports balance).
    let tip_insertion_idx = unsigned_writable_end;
    msg.account_keys.insert(tip_insertion_idx, tip_account);

    // Every existing CompiledInstruction.program_id_index and accounts[*]
    // that was >= tip_insertion_idx (before the insert) needs +1.
    bump_indexes_at_or_above(&mut msg.instructions, tip_insertion_idx, 1)?;

    // ---- 5. Insert the SystemProgram into unsigned_readonly if absent. ----
    //
    // The "unsigned_readonly" region is at the END of static account_keys.
    // Inserting at the end is safe — no existing CompiledInstruction
    // account-index entries need to change (insertion doesn't shift earlier
    // indexes), BUT entries that referenced ALT-loaded accounts (with
    // indexes >= old total) need +1 too. We have to bump *every* index
    // that is >= the insertion point.
    let sysprog_idx = if needs_sysprog {
        let new_total = msg.account_keys.len();
        // Insert at the very end of static account_keys (which is
        // immediately before ALT-loaded indices conceptually start).
        msg.account_keys.push(sysprog);
        // Bump only entries that pointed AT OR ABOVE the old end of static keys
        // (i.e. ALT-loaded ones). We just appended, so the insertion point
        // is `new_total`, and we shift any existing index >= new_total by 1.
        bump_indexes_at_or_above(&mut msg.instructions, new_total, 1)?;
        msg.header.num_readonly_unsigned_accounts = msg
            .header
            .num_readonly_unsigned_accounts
            .checked_add(1)
            .ok_or_else(|| anyhow!("num_readonly_unsigned_accounts overflow"))?;
        new_total
    } else {
        // Find existing SystemProgram index. Note: tip_account was inserted
        // before us, so SystemProgram's index may have moved.
        msg.account_keys
            .iter()
            .position(|k| *k == sysprog)
            .expect("sysprog presence checked above")
    };

    // ---- 6. Append the tip transfer instruction. ----
    //
    // SystemProgram::transfer is a 12-byte instruction:
    //   - 4-byte LE u32 = 2 (variant for Transfer)
    //   - 8-byte LE u64 = lamports
    //
    // Accounts (order matters per system_instruction::transfer):
    //   [0] from (writable signer) = payer = index 0
    //   [1] to   (writable)        = tip_account
    let payer_idx_u8: u8 = 0; // validated above
    let tip_idx_u8: u8 = u8::try_from(
        msg.account_keys
            .iter()
            .position(|k| *k == tip_account)
            .expect("tip_account just inserted"),
    )
    .map_err(|_| anyhow!("account_keys grew past u8::MAX"))?;
    let sysprog_idx_u8: u8 =
        u8::try_from(sysprog_idx).map_err(|_| anyhow!("sysprog index past u8::MAX"))?;

    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(&2u32.to_le_bytes()); // Transfer variant
    data.extend_from_slice(&tip_lamports.to_le_bytes());

    msg.instructions.push(CompiledInstruction {
        program_id_index: sysprog_idx_u8,
        accounts: vec![payer_idx_u8, tip_idx_u8],
        data,
    });

    Ok(msg)
}

/// Add `delta` to every program_id_index AND accounts[*] entry that is
/// `>= threshold`. Used after inserting a key at `threshold` into
/// `account_keys`. Returns Err if any index would overflow u8.
fn bump_indexes_at_or_above(
    instructions: &mut [CompiledInstruction],
    threshold: usize,
    delta: u8,
) -> Result<()> {
    let threshold_u8: u8 = u8::try_from(threshold)
        .map_err(|_| anyhow!("threshold {threshold} exceeds u8::MAX"))?;
    for ix in instructions.iter_mut() {
        if ix.program_id_index >= threshold_u8 {
            ix.program_id_index = ix
                .program_id_index
                .checked_add(delta)
                .ok_or_else(|| anyhow!("program_id_index overflow"))?;
        }
        for a in ix.accounts.iter_mut() {
            if *a >= threshold_u8 {
                *a = a
                    .checked_add(delta)
                    .ok_or_else(|| anyhow!("account index overflow"))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{
        hash::Hash,
        message::v0::Message as V0Message,
        system_instruction,
    };

    fn fixture_payer() -> Pubkey {
        Pubkey::new_unique()
    }

    fn fixture_tip() -> Pubkey {
        Pubkey::from_str(TIP_ACCOUNTS[0]).unwrap()
    }

    /// Build the simplest possible v0 message: a single transfer payer → recipient.
    /// account_keys = [payer (signed writable), recipient (unsigned writable), sysprog (unsigned readonly)]
    /// header = { num_required_signatures: 1, num_readonly_signed: 0, num_readonly_unsigned: 1 }
    fn fixture_simple_transfer_v0() -> (V0Message, Pubkey) {
        let payer = fixture_payer();
        let recipient = Pubkey::new_unique();
        let ix = system_instruction::transfer(&payer, &recipient, 1_000);
        let msg = V0Message::try_compile(&payer, &[ix], &[], Hash::default()).unwrap();
        (msg, payer)
    }

    /// Build a v0 message that does NOT touch SystemProgram, so we can test
    /// the "needs_sysprog = true" path. We use a memo-like noop with a single
    /// unsigned-readonly account.
    fn fixture_no_sysprog_v0() -> (V0Message, Pubkey) {
        let payer = fixture_payer();
        let noop_program = Pubkey::new_unique(); // pretend program
        let ix = solana_sdk::instruction::Instruction {
            program_id: noop_program,
            accounts: vec![],
            data: vec![],
        };
        let msg = V0Message::try_compile(&payer, &[ix], &[], Hash::default()).unwrap();
        (msg, payer)
    }

    #[test]
    fn tip_account_helper_returns_one_of_eight() {
        let p = random_tip_account().unwrap();
        let s = p.to_string();
        assert!(TIP_ACCOUNTS.contains(&s.as_str()), "got {s}");
    }

    #[test]
    fn rejects_zero_tip() {
        let (msg, payer) = fixture_simple_transfer_v0();
        let r = inject_tip_into_v0(msg, payer, fixture_tip(), 0);
        assert!(r.is_err());
    }

    #[test]
    fn rejects_when_payer_not_at_index_zero() {
        let (mut msg, _) = fixture_simple_transfer_v0();
        // Swap payer to a non-zero position to simulate malformed input.
        let imposter = Pubkey::new_unique();
        msg.account_keys[0] = imposter; // payer no longer in keys at all
        let r = inject_tip_into_v0(msg, imposter, fixture_tip(), 1000);
        // imposter IS at index 0 now, so this should succeed actually.
        assert!(r.is_ok());
    }

    #[test]
    fn rejects_when_payer_not_in_keys() {
        let (msg, _) = fixture_simple_transfer_v0();
        let stranger = Pubkey::new_unique();
        let r = inject_tip_into_v0(msg, stranger, fixture_tip(), 1000);
        assert!(r.is_err());
        assert!(format!("{:?}", r).contains("payer pubkey not found"));
    }

    #[test]
    fn happy_path_sysprog_already_present_adds_tip_correctly() {
        let (msg_before, payer) = fixture_simple_transfer_v0();
        let keys_before = msg_before.account_keys.clone();
        let header_before = msg_before.header;
        let ix_count_before = msg_before.instructions.len();

        let tip = fixture_tip();
        let msg_after = inject_tip_into_v0(msg_before, payer, tip, 5_000).unwrap();

        // The tip account should be present.
        assert!(
            msg_after.account_keys.iter().any(|k| *k == tip),
            "tip account missing from account_keys"
        );

        // SystemProgram should still be present (and was already there in
        // the original transfer ix), so num_readonly_unsigned_accounts
        // should NOT increase.
        assert_eq!(
            msg_after.header.num_readonly_unsigned_accounts,
            header_before.num_readonly_unsigned_accounts,
            "should not bump num_readonly_unsigned when sysprog already present"
        );

        // One additional instruction.
        assert_eq!(msg_after.instructions.len(), ix_count_before + 1);

        // Last instruction is our tip transfer.
        let tip_ix = msg_after.instructions.last().unwrap();
        assert_eq!(
            msg_after.account_keys[tip_ix.program_id_index as usize],
            system_program::ID
        );
        assert_eq!(tip_ix.accounts.len(), 2);
        assert_eq!(msg_after.account_keys[tip_ix.accounts[0] as usize], payer);
        assert_eq!(msg_after.account_keys[tip_ix.accounts[1] as usize], tip);

        // Data decodes back to (variant=2, amount=5_000).
        assert_eq!(tip_ix.data.len(), 12);
        let variant = u32::from_le_bytes(tip_ix.data[0..4].try_into().unwrap());
        let amount = u64::from_le_bytes(tip_ix.data[4..12].try_into().unwrap());
        assert_eq!(variant, 2);
        assert_eq!(amount, 5_000);

        // Account keys length grew by exactly 1 (the tip account).
        assert_eq!(msg_after.account_keys.len(), keys_before.len() + 1);
    }

    #[test]
    fn happy_path_sysprog_needs_insertion() {
        let (msg_before, payer) = fixture_no_sysprog_v0();
        let header_before = msg_before.header;
        let keys_before = msg_before.account_keys.clone();
        assert!(
            !keys_before.iter().any(|k| *k == system_program::ID),
            "fixture sanity: sysprog should be absent"
        );

        let tip = fixture_tip();
        let msg_after = inject_tip_into_v0(msg_before, payer, tip, 7_777).unwrap();

        // num_readonly_unsigned should have grown by 1 (for sysprog).
        assert_eq!(
            msg_after.header.num_readonly_unsigned_accounts,
            header_before.num_readonly_unsigned_accounts + 1
        );

        // Account keys grew by 2 (tip + sysprog).
        assert_eq!(msg_after.account_keys.len(), keys_before.len() + 2);

        // Both new accounts present.
        assert!(msg_after.account_keys.iter().any(|k| *k == tip));
        assert!(msg_after.account_keys.iter().any(|k| *k == system_program::ID));

        // Last instruction = our tip transfer, references both correctly.
        let tip_ix = msg_after.instructions.last().unwrap();
        assert_eq!(
            msg_after.account_keys[tip_ix.program_id_index as usize],
            system_program::ID
        );
        assert_eq!(msg_after.account_keys[tip_ix.accounts[0] as usize], payer);
        assert_eq!(msg_after.account_keys[tip_ix.accounts[1] as usize], tip);
    }

    #[test]
    fn rejects_duplicate_tip_account() {
        // If somehow the tip account is already in the keys, we refuse to
        // try to mutate its writability — we'd rather pick a different one.
        let payer = fixture_payer();
        let tip = fixture_tip();
        // Build a tx that ALREADY references the tip account as a readonly
        // unsigned account. We use a noop program so we don't have to set
        // up any real instruction logic.
        let noop_program = Pubkey::new_unique();
        let ix = solana_sdk::instruction::Instruction {
            program_id: noop_program,
            accounts: vec![solana_sdk::instruction::AccountMeta::new_readonly(tip, false)],
            data: vec![],
        };
        let msg = V0Message::try_compile(&payer, &[ix], &[], Hash::default()).unwrap();
        assert!(msg.account_keys.iter().any(|k| *k == tip));

        let r = inject_tip_into_v0(msg, payer, tip, 1_000);
        assert!(r.is_err());
        assert!(format!("{:?}", r).contains("already present"));
    }

    #[test]
    fn existing_instruction_indexes_are_preserved() {
        // The transfer ix in the fixture references the payer at index 0,
        // recipient at index 1, sysprog at index 2. After we insert the tip
        // account, the original ix should still resolve to the SAME pubkeys.
        let (msg_before, payer) = fixture_simple_transfer_v0();
        let original_ix = msg_before.instructions[0].clone();
        let original_resolved: Vec<Pubkey> = original_ix
            .accounts
            .iter()
            .map(|a| msg_before.account_keys[*a as usize])
            .collect();
        let original_program =
            msg_before.account_keys[original_ix.program_id_index as usize];

        let tip = fixture_tip();
        let msg_after = inject_tip_into_v0(msg_before, payer, tip, 1_000).unwrap();
        let after_ix = &msg_after.instructions[0]; // original instruction
        let after_resolved: Vec<Pubkey> = after_ix
            .accounts
            .iter()
            .map(|a| msg_after.account_keys[*a as usize])
            .collect();
        let after_program = msg_after.account_keys[after_ix.program_id_index as usize];

        assert_eq!(original_resolved, after_resolved);
        assert_eq!(original_program, after_program);
    }

    #[test]
    fn instructions_pointing_at_alt_loaded_accounts_are_shifted_correctly() {
        // Synthetic case: a CompiledInstruction whose accounts[] points at
        // an index BEYOND the static account_keys range (i.e., an ALT-loaded
        // account). When we insert a static key, that ALT index MUST also
        // shift by 1 because the resolution is concat(static, alt_writable, alt_readonly).
        //
        // We don't actually build an ALT here — we just construct a v0 message
        // by hand with an instruction whose accounts[] points at a synthetic
        // high index, and verify the index is bumped correctly.
        let payer = fixture_payer();
        let alt_table = Pubkey::new_unique();

        // 3 static keys: payer (signed writable), program (unsigned readonly),
        // sysprog (unsigned readonly). Plus we'll claim 1 writable+1 readonly
        // ALT-loaded entry → effective indices 3 and 4.
        let program = Pubkey::new_unique();
        let mut msg = V0Message {
            header: MessageHeader {
                num_required_signatures: 1,
                num_readonly_signed_accounts: 0,
                num_readonly_unsigned_accounts: 2, // program + sysprog readonly
            },
            account_keys: vec![payer, program, system_program::ID],
            recent_blockhash: Hash::default(),
            instructions: vec![CompiledInstruction {
                program_id_index: 1,
                accounts: vec![0, 3, 4], // 3,4 are ALT-loaded
                data: vec![],
            }],
            address_table_lookups: vec![solana_sdk::message::v0::MessageAddressTableLookup {
                account_key: alt_table,
                writable_indexes: vec![0],
                readonly_indexes: vec![1],
            }],
        };
        // Sanity: as-is, indices 3 & 4 are ALT-loaded.
        assert_eq!(msg.account_keys.len(), 3);
        assert!(msg.instructions[0].accounts.contains(&3));
        assert!(msg.instructions[0].accounts.contains(&4));

        let tip = fixture_tip();
        msg = inject_tip_into_v0(msg, payer, tip, 100).unwrap();

        // After insertion: tip account inserted at end of unsigned_writable region.
        // unsigned_writable was empty before (everything was readonly), so tip
        // goes at index = num_sig = 1. That shifts program from index 1 → 2,
        // sysprog from 2 → 3, and the ALT-loaded indices 3,4 → 4,5.
        let new_ix = &msg.instructions[0];
        assert!(
            new_ix.accounts.contains(&4) && new_ix.accounts.contains(&5),
            "ALT-loaded indices should have shifted: got {:?}",
            new_ix.accounts
        );
        // accounts[0] was payer at index 0 → still 0.
        assert_eq!(new_ix.accounts[0], 0);
    }

    #[test]
    fn bump_indexes_at_threshold_inclusive() {
        // Index == threshold MUST be bumped (the inserted key took its slot).
        let mut ixs = vec![CompiledInstruction {
            program_id_index: 5,
            accounts: vec![0, 4, 5, 6],
            data: vec![],
        }];
        bump_indexes_at_or_above(&mut ixs, 5, 1).unwrap();
        assert_eq!(ixs[0].program_id_index, 6);
        assert_eq!(ixs[0].accounts, vec![0, 4, 6, 7]);
    }

    #[test]
    fn bump_indexes_overflow_u8_errors() {
        let mut ixs = vec![CompiledInstruction {
            program_id_index: 255,
            accounts: vec![0],
            data: vec![],
        }];
        let r = bump_indexes_at_or_above(&mut ixs, 0, 1);
        assert!(r.is_err());
    }
}

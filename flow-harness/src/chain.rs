//! On-chain seam — the single staging-live touch-point the flow bodies delegate
//! to. Decodes the worker's unsigned base64 transaction, signs it with the
//! funded payer, submits it to the cluster, and reads/decodes on-chain accounts.
//!
//! ## Why this is safe to write before staging exists
//!
//! Everything format-critical is pure and unit-tested offline:
//!  - [`sign_worker_tx`] — decode → sign → re-encode round-trip (tested against
//!    a `solana-sdk`-serialized tx, which is the exact wire format the worker
//!    emits: `worker/src/solana_escrow/wire.rs` mirrors bincode/short-vec).
//!  - [`parse_rpc_error`] — maps a Solana JSON-RPC `InstructionError: Custom(N)`
//!    to an [`EscrowCode`], so an on-chain revert is matched the same way a
//!    server-side rejection is (negative-test flows key on `EscrowCode`).
//!  - [`parse_account_value`] / [`decode_attendee_deposit`] — `getAccountInfo`
//!    envelope + `AttendeeDeposit` field decode at fixed offsets.
//!
//! Only [`submit_tx`] and [`fetch_account`] touch the network; they compose the
//! pure helpers above with two JSON-RPC calls.
//!
//! ## Signing model
//!
//! The worker builds the transaction with a recent blockhash already embedded
//! and the fee-payer / attendee as the (only) required signer, serialized with
//! zero-filled signature placeholders. So the harness signs over the existing
//! message — `ctx.payer` MUST be that required signer (for deposit/refund, the
//! attendee wallet). If the payer is not a required signer, [`sign_worker_tx`]
//! fails with a clear message rather than producing an unverifiable tx.

use std::str::FromStr;

use base64::Engine as _;
use serde_json::{json, Value};
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signature,
    signer::keypair::Keypair,
    transaction::Transaction,
};

use crate::context::StagingContext;
use crate::error::{EscrowCode, HarnessError, HarnessResult, WorkerError};

const B64: base64::engine::general_purpose::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// How many times to poll `getSignatureStatuses` before giving up (≈ the
/// blockhash validity window at ~1s between polls).
const CONFIRM_POLLS: usize = 45;

// ── Transaction signing (pure, offline-tested) ───────────────────────────────

/// Decode the worker's unsigned base64 wire transaction, sign it with `payer`
/// over the blockhash the worker embedded, and return `(signed_base64, signature)`.
pub fn sign_worker_tx(tx_b64: &str, payer: &Keypair) -> HarnessResult<(String, Signature)> {
    let bytes = B64
        .decode(tx_b64.trim())
        .map_err(|e| HarnessError::Solana(format!("base64 decode: {e}")))?;
    let mut tx: Transaction = bincode::deserialize(&bytes).map_err(|e| {
        HarnessError::Solana(format!("tx deserialize (expected legacy wire tx): {e}"))
    })?;
    let blockhash = tx.message.recent_blockhash;
    // `try_sign` positions the payer's signature by matching its pubkey against
    // the message account keys, so the payer must be a required signer.
    tx.try_sign(&[payer], blockhash).map_err(|e| {
        HarnessError::Solana(format!(
            "sign failed (is ctx.payer the required signer / fee payer?): {e}"
        ))
    })?;
    let signed = bincode::serialize(&tx)
        .map_err(|e| HarnessError::Solana(format!("tx serialize: {e}")))?;
    let sig = tx
        .signatures
        .first()
        .copied()
        .ok_or_else(|| HarnessError::Solana("signed tx has no signature".to_string()))?;
    Ok((B64.encode(signed), sig))
}

// ── RPC-response parsing (pure, offline-tested) ──────────────────────────────

/// Map a Solana JSON-RPC `error` object into a [`HarnessError`]. A program
/// revert (`InstructionError: [_, {"Custom": N}]`) becomes
/// `Worker(WorkerError { code: Some(EscrowCode::from_u32(N)) })` so negative
/// flows match on-chain reverts exactly like server-side rejections; anything
/// else becomes [`HarnessError::Solana`].
pub fn parse_rpc_error(err: &Value) -> HarnessError {
    let message = err
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("solana rpc error")
        .to_string();
    if let Some(code) = extract_custom_code(err) {
        return HarnessError::Worker(WorkerError {
            http_status: 0, // 0 = originated on-chain, not from an HTTP status
            code: Some(EscrowCode::from_u32(code)),
            message,
        });
    }
    HarnessError::Solana(message)
}

/// Find an `InstructionError: [idx, {"Custom": N}]` anywhere under an RPC error
/// or a `getSignatureStatuses` status object, returning `N`.
fn extract_custom_code(v: &Value) -> Option<u32> {
    // Locations observed: sendTransaction preflight → error.data.err.InstructionError;
    // getSignatureStatuses → result.value[i].err.InstructionError.
    let err_obj = v
        .pointer("/data/err")
        .or_else(|| v.get("err"))
        .or(Some(v))?;
    let arr = err_obj.get("InstructionError")?.as_array()?;
    let custom = arr.get(1)?.get("Custom")?;
    custom.as_u64().map(|n| n as u32)
}

/// An on-chain account as returned by `getAccountInfo`.
#[derive(Debug, Clone)]
pub struct FetchedAccount {
    pub owner: Pubkey,
    pub data: Vec<u8>,
}

/// Parse the `result.value` of a `getAccountInfo` response. `null` → `Ok(None)`
/// (account does not exist / was closed).
pub fn parse_account_value(value: &Value) -> HarnessResult<Option<FetchedAccount>> {
    if value.is_null() {
        return Ok(None);
    }
    let owner = value
        .get("owner")
        .and_then(Value::as_str)
        .ok_or_else(|| HarnessError::Solana("getAccountInfo: missing owner".to_string()))?;
    let owner = Pubkey::from_str(owner)
        .map_err(|e| HarnessError::Solana(format!("getAccountInfo: bad owner: {e}")))?;
    let data_b64 = value
        .pointer("/data/0")
        .and_then(Value::as_str)
        .ok_or_else(|| HarnessError::Solana("getAccountInfo: missing base64 data".to_string()))?;
    let data = B64
        .decode(data_b64)
        .map_err(|e| HarnessError::Solana(format!("getAccountInfo: data decode: {e}")))?;
    Ok(Some(FetchedAccount { owner, data }))
}

/// Decoded view over the fields of an on-chain `AttendeeDeposit` account.
///
/// Layout (see `bethere-escrow/src/state.rs` + the SVM test fixtures):
/// `[0]` discriminator=2, `[1]` version, `[2..34]` attendee, `[34..66]` event,
/// `[66..74]` amount (u64 LE), `[74..82]` deposited_at (i64 LE), `[82]`
/// checked_in, `[83]` refunded, `[84]` bump, `[85..96]` padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttendeeDepositView {
    pub version: u8,
    pub amount: u64,
    pub checked_in: bool,
    pub refunded: bool,
}

/// AttendeeDeposit account discriminator (`#[account(discriminator = 2)]`).
const ATTENDEE_DEPOSIT_DISCRIMINATOR: u8 = 2;
/// Minimum serialized length of an `AttendeeDeposit` account (disc + struct).
const ATTENDEE_DEPOSIT_MIN_LEN: usize = 96;

/// Decode the fixed-offset fields of an `AttendeeDeposit` account.
pub fn decode_attendee_deposit(data: &[u8]) -> HarnessResult<AttendeeDepositView> {
    if data.len() < ATTENDEE_DEPOSIT_MIN_LEN {
        return Err(HarnessError::Solana(format!(
            "AttendeeDeposit too short: {} < {ATTENDEE_DEPOSIT_MIN_LEN}",
            data.len()
        )));
    }
    if data[0] != ATTENDEE_DEPOSIT_DISCRIMINATOR {
        return Err(HarnessError::Solana(format!(
            "AttendeeDeposit discriminator = {}, expected {ATTENDEE_DEPOSIT_DISCRIMINATOR}",
            data[0]
        )));
    }
    let amount = u64::from_le_bytes(data[66..74].try_into().expect("8 bytes"));
    Ok(AttendeeDepositView {
        version: data[1],
        amount,
        checked_in: data[82] != 0,
        refunded: data[83] != 0,
    })
}

// ── Network (composes the pure helpers above) ────────────────────────────────

/// Sign the worker transaction and submit it, confirming it landed. An on-chain
/// program revert surfaces as `HarnessError::Worker` with the parsed
/// [`EscrowCode`]; RPC/transport problems surface as [`HarnessError::Solana`].
pub async fn submit_tx(ctx: &StagingContext, tx_b64: &str) -> HarnessResult<Signature> {
    let (signed_b64, sig) = sign_worker_tx(tx_b64, &ctx.payer)?;
    let http = reqwest::Client::new();
    let resp = rpc_call(
        &http,
        ctx.rpc_url.as_str(),
        "sendTransaction",
        json!([
            signed_b64,
            { "encoding": "base64", "preflightCommitment": "confirmed", "maxRetries": 3 }
        ]),
    )
    .await?;
    if let Some(err) = resp.get("error") {
        return Err(parse_rpc_error(err));
    }
    confirm_signature(&http, ctx.rpc_url.as_str(), &sig).await?;
    Ok(sig)
}

/// Read an on-chain account. `Ok(None)` if it does not exist (or was closed).
pub async fn fetch_account(
    ctx: &StagingContext,
    pubkey: &Pubkey,
) -> HarnessResult<Option<FetchedAccount>> {
    let http = reqwest::Client::new();
    let resp = rpc_call(
        &http,
        ctx.rpc_url.as_str(),
        "getAccountInfo",
        json!([
            pubkey.to_string(),
            { "encoding": "base64", "commitment": "confirmed" }
        ]),
    )
    .await?;
    if let Some(err) = resp.get("error") {
        return Err(parse_rpc_error(err));
    }
    parse_account_value(&resp["result"]["value"])
}

/// Poll `getSignatureStatuses` until the signature is confirmed/finalized, the
/// transaction errored (→ mapped revert), or the poll budget is exhausted.
async fn confirm_signature(
    http: &reqwest::Client,
    rpc_url: &str,
    sig: &Signature,
) -> HarnessResult<()> {
    for _ in 0..CONFIRM_POLLS {
        let resp = rpc_call(
            http,
            rpc_url,
            "getSignatureStatuses",
            json!([[sig.to_string()], { "searchTransactionHistory": false }]),
        )
        .await?;
        if let Some(err) = resp.get("error") {
            return Err(parse_rpc_error(err));
        }
        let status = &resp["result"]["value"][0];
        if !status.is_null() {
            if let Some(err) = status.get("err").filter(|e| !e.is_null()) {
                return Err(parse_rpc_error(err));
            }
            let confirmed = status
                .get("confirmationStatus")
                .and_then(Value::as_str)
                .map(|s| matches!(s, "confirmed" | "finalized"))
                .unwrap_or(false);
            if confirmed {
                return Ok(());
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    Err(HarnessError::Solana(format!(
        "timed out confirming {sig} after {CONFIRM_POLLS} polls"
    )))
}

/// Issue a JSON-RPC 2.0 call and return the parsed response body.
async fn rpc_call(
    http: &reqwest::Client,
    rpc_url: &str,
    method: &str,
    params: Value,
) -> HarnessResult<Value> {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let resp = http
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(HarnessError::from)?;
    resp.json::<Value>().await.map_err(HarnessError::from)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::hash::Hash;
    use solana_sdk::instruction::{AccountMeta, Instruction};
    use solana_sdk::message::Message;
    use solana_sdk::signer::Signer;

    /// Build an unsigned wire transaction the way the worker does (payer as the
    /// sole required signer, a recent blockhash embedded, zero-filled signature
    /// placeholder) and return its base64.
    fn unsigned_worker_tx_b64(payer: &Keypair, blockhash: Hash) -> String {
        let program = Pubkey::new_unique();
        let ix = Instruction::new_with_bytes(
            program,
            &[1, 2, 3],
            vec![AccountMeta::new(payer.pubkey(), true)],
        );
        let mut msg = Message::new(&[ix], Some(&payer.pubkey()));
        msg.recent_blockhash = blockhash;
        // new_unsigned sizes the signatures vec to num_required_signatures with
        // default (zero) signatures — identical to the worker's placeholders.
        let tx = Transaction::new_unsigned(msg);
        B64.encode(bincode::serialize(&tx).unwrap())
    }

    #[test]
    fn sign_worker_tx_round_trips_and_verifies() {
        let payer = Keypair::new();
        let bh = Hash::new_unique();
        let tx_b64 = unsigned_worker_tx_b64(&payer, bh);

        let (signed_b64, sig) = sign_worker_tx(&tx_b64, &payer).expect("sign");

        let bytes = B64.decode(signed_b64).unwrap();
        let signed: Transaction = bincode::deserialize(&bytes).unwrap();
        // The signature verifies against the message, and it is the returned one.
        signed.verify().expect("signature verifies");
        assert_eq!(signed.signatures[0], sig);
        assert_ne!(sig, Signature::default());
        assert_eq!(signed.message.recent_blockhash, bh);
    }

    #[test]
    fn sign_worker_tx_rejects_payer_that_is_not_the_signer() {
        let real_signer = Keypair::new();
        let wrong_payer = Keypair::new();
        let tx_b64 = unsigned_worker_tx_b64(&real_signer, Hash::new_unique());
        let err = sign_worker_tx(&tx_b64, &wrong_payer).unwrap_err();
        assert!(matches!(err, HarnessError::Solana(_)), "got {err:?}");
    }

    #[test]
    fn sign_worker_tx_rejects_garbage_base64() {
        let err = sign_worker_tx("not+valid+tx+bytes", &Keypair::new()).unwrap_err();
        assert!(matches!(err, HarnessError::Solana(_)), "got {err:?}");
    }

    #[test]
    fn parse_rpc_error_maps_custom_code_to_escrow_worker_error() {
        // sendTransaction preflight failure shape.
        let err = json!({
            "code": -32002,
            "message": "Transaction simulation failed",
            "data": { "err": { "InstructionError": [0, { "Custom": 1 }] } }
        });
        match parse_rpc_error(&err) {
            HarnessError::Worker(w) => {
                assert_eq!(w.code, Some(EscrowCode::RefundNotYetAllowed));
                assert_eq!(w.http_status, 0);
            }
            other => panic!("expected Worker, got {other:?}"),
        }
    }

    #[test]
    fn parse_rpc_error_maps_deadline_passed_code() {
        let err = json!({
            "message": "revert",
            "data": { "err": { "InstructionError": [1, { "Custom": 19 }] } }
        });
        match parse_rpc_error(&err) {
            HarnessError::Worker(w) => assert_eq!(w.code, Some(EscrowCode::RefundDeadlinePassed)),
            other => panic!("expected Worker, got {other:?}"),
        }
    }

    #[test]
    fn parse_rpc_error_without_custom_code_is_solana() {
        let err = json!({ "code": -32000, "message": "Blockhash not found" });
        assert!(matches!(parse_rpc_error(&err), HarnessError::Solana(_)));
    }

    #[test]
    fn extract_custom_code_from_signature_status_err() {
        // getSignatureStatuses places the err at the top level of the status.
        let status_err = json!({ "InstructionError": [0, { "Custom": 22 }] });
        assert_eq!(extract_custom_code(&status_err), Some(22));
    }

    #[test]
    fn parse_account_value_null_is_none() {
        assert!(parse_account_value(&Value::Null).unwrap().is_none());
    }

    #[test]
    fn parse_account_value_decodes_owner_and_data() {
        let owner = Pubkey::new_unique();
        let raw = vec![2u8, 1, 42, 7];
        let value = json!({
            "owner": owner.to_string(),
            "data": [B64.encode(&raw), "base64"],
            "lamports": 1_500_000u64,
        });
        let acct = parse_account_value(&value).unwrap().expect("some");
        assert_eq!(acct.owner, owner);
        assert_eq!(acct.data, raw);
    }

    #[test]
    fn decode_attendee_deposit_reads_fixed_offsets() {
        let mut data = vec![0u8; ATTENDEE_DEPOSIT_MIN_LEN];
        data[0] = ATTENDEE_DEPOSIT_DISCRIMINATOR;
        data[1] = 1; // version
        data[66..74].copy_from_slice(&15_000_000u64.to_le_bytes()); // amount
        data[82] = 0; // checked_in = false
        data[83] = 1; // refunded = true
        let view = decode_attendee_deposit(&data).unwrap();
        assert_eq!(
            view,
            AttendeeDepositView { version: 1, amount: 15_000_000, checked_in: false, refunded: true }
        );
    }

    #[test]
    fn decode_attendee_deposit_rejects_wrong_discriminator() {
        let mut data = vec![0u8; ATTENDEE_DEPOSIT_MIN_LEN];
        data[0] = 1; // EventEscrow's discriminator, not AttendeeDeposit's
        assert!(matches!(
            decode_attendee_deposit(&data),
            Err(HarnessError::Solana(_))
        ));
    }

    #[test]
    fn decode_attendee_deposit_rejects_short_buffer() {
        assert!(matches!(
            decode_attendee_deposit(&[2, 1, 0]),
            Err(HarnessError::Solana(_))
        ));
    }
}

//! Unit tests for the USDC deposit on-chain verification helpers.
//!
//! The decision logic for `verify_tx_with_signer` is extracted into
//! `parse_get_transaction_response` so it can be unit-tested without needing
//! the Cloudflare Workers `Fetch`/`Delay` runtime. The fixtures below mirror
//! the shape of real `getTransaction` RPC responses observed on devnet.
//!
//! Run: `cargo test -p worker --lib parse_get_transaction` (or `verify_signer`)
//! from the repo root.

use super::discovery::binding_conflict;
use super::rpc::{parse_get_transaction_response, parse_signatures_for_address_response};
use super::types::VerifyWithSignerOutcome;

// ─── binding_conflict (plan 003 — claim-binding guard) ───────────────

#[test]
fn test_binding_conflict_no_owners_returns_false() {
    assert!(!binding_conflict("att_1", None, None));
}

#[test]
fn test_binding_conflict_self_match_returns_false() {
    // Idempotent re-recovery: both bound to the current attendee.
    assert!(!binding_conflict("att_1", Some("att_1"), Some("att_1")));
}

#[test]
fn test_binding_conflict_wallet_other_attendee_returns_true() {
    assert!(binding_conflict("att_1", Some("att_2"), None));
}

#[test]
fn test_binding_conflict_tx_other_attendee_returns_true() {
    assert!(binding_conflict("att_1", None, Some("att_2")));
}

#[test]
fn test_binding_conflict_both_other_attendee_returns_true() {
    assert!(binding_conflict("att_1", Some("att_2"), Some("att_3")));
}

#[test]
fn test_binding_conflict_wallet_self_tx_other_returns_true() {
    // Wallet legitimately bound to current attendee, but tx is bound to
    // a different one — still flagged as a conflict.
    assert!(binding_conflict("att_1", Some("att_1"), Some("att_2")));
}

/// Build a confirmed `getTransaction` result with the given fee-payer.
fn confirmed_tx(signer: &str, confirmation: &str) -> serde_json::Value {
    serde_json::json!({
        "result": {
            "transaction": {
                "message": {
                    "accountKeys": [signer, "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"]
                }
            },
            "meta": {
                "err": null,
                "confirmationStatus": confirmation
            }
        }
    })
}

/// Build a failed-on-chain TX result (e.g., insufficient funds, program error).
fn failed_tx(_err_msg: &str) -> serde_json::Value {
    serde_json::json!({
        "result": {
            "transaction": {
                "message": {
                    "accountKeys": ["AqdrF1bMEayzZC72R7SxsC2KFqybT5rHPYswkFWe5Mkn"]
                }
            },
            "meta": {
                "err": { "InstructionError": [0, "Custom(1)"] },
                "confirmationStatus": "confirmed"
            }
        }
    })
}

// ─── VerifyWithSignerOutcome methods ──────────────────────────────────

#[test]
fn test_outcome_is_confirmed_and_matched_true() {
    let outcome = VerifyWithSignerOutcome::Confirmed {
        signer_matched: true,
        signer: "WalletA".to_string(),
    };
    assert!(outcome.is_confirmed());
    assert!(outcome.is_confirmed_and_matched());
    assert_eq!(outcome.signer(), Some("WalletA"));
}

#[test]
fn test_outcome_is_confirmed_but_not_matched() {
    let outcome = VerifyWithSignerOutcome::Confirmed {
        signer_matched: false,
        signer: "WalletB".to_string(),
    };
    assert!(outcome.is_confirmed());
    assert!(!outcome.is_confirmed_and_matched());
    assert_eq!(outcome.signer(), Some("WalletB"));
}

#[test]
fn test_outcome_pending_neither_confirmed_nor_matched() {
    let outcome = VerifyWithSignerOutcome::Pending;
    assert!(!outcome.is_confirmed());
    assert!(!outcome.is_confirmed_and_matched());
    assert_eq!(outcome.signer(), None);
}

#[test]
fn test_outcome_rpc_error_neither_confirmed_nor_matched() {
    let outcome = VerifyWithSignerOutcome::RpcError;
    assert!(!outcome.is_confirmed());
    assert!(!outcome.is_confirmed_and_matched());
    assert_eq!(outcome.signer(), None);
}

// ─── parse_get_transaction_response — happy path ─────────────────────

#[test]
fn test_parse_confirmed_signer_matches_expected_wallet() {
    let parsed = confirmed_tx("AqdrF1bMEayzZC72R7SxsC2KFqybT5rHPYswkFWe5Mkn", "confirmed");
    let outcome = parse_get_transaction_response(
        &parsed,
        "sig123",
        Some("AqdrF1bMEayzZC72R7SxsC2KFqybT5rHPYswkFWe5Mkn"),
    );
    assert!(outcome.is_confirmed_and_matched());
    assert_eq!(
        outcome.signer(),
        Some("AqdrF1bMEayzZC72R7SxsC2KFqybT5rHPYswkFWe5Mkn")
    );
}

#[test]
fn test_parse_finalized_signer_matches() {
    // "finalized" should also count as confirmed.
    let parsed = confirmed_tx("WalletX", "finalized");
    let outcome = parse_get_transaction_response(&parsed, "sig", Some("WalletX"));
    assert!(outcome.is_confirmed_and_matched());
}

#[test]
fn test_parse_confirmed_no_expected_wallet_backfills_signer() {
    // No expected_wallet → signer is extracted, signer_matched defaults to true.
    let parsed = confirmed_tx("ResolvedSignerABC", "confirmed");
    let outcome = parse_get_transaction_response(&parsed, "sig", None);
    assert!(outcome.is_confirmed_and_matched());
    assert_eq!(outcome.signer(), Some("ResolvedSignerABC"));
}

// ─── parse_get_transaction_response — signer mismatch (security) ─────

#[test]
fn test_parse_confirmed_signer_does_not_match_expected_wallet() {
    let parsed = confirmed_tx("AttackerWallet", "confirmed");
    let outcome = parse_get_transaction_response(&parsed, "sig", Some("ExpectedAttendeeWallet"));
    // Confirmed, but NOT matched — caller must refuse verification.
    assert!(outcome.is_confirmed());
    assert!(!outcome.is_confirmed_and_matched());
    assert_eq!(outcome.signer(), Some("AttackerWallet"));
}

#[test]
fn test_parse_signer_match_is_case_insensitive() {
    // Defensive case-insensitive comparison handles base58 encoding quirks.
    let parsed = confirmed_tx("WalletABC", "confirmed");
    let outcome = parse_get_transaction_response(&parsed, "sig", Some("walletabc"));
    assert!(outcome.is_confirmed_and_matched());
}

// ─── parse_get_transaction_response — Pending paths ──────────────────

#[test]
fn test_parse_result_null_is_pending() {
    let parsed = serde_json::json!({ "result": null });
    let outcome = parse_get_transaction_response(&parsed, "sig", Some("WalletA"));
    assert!(matches!(outcome, VerifyWithSignerOutcome::Pending));
}

#[test]
fn test_parse_result_missing_is_pending() {
    let parsed = serde_json::json!({});
    let outcome = parse_get_transaction_response(&parsed, "sig", Some("WalletA"));
    assert!(matches!(outcome, VerifyWithSignerOutcome::Pending));
}

#[test]
fn test_parse_tx_failed_on_chain_is_pending() {
    let parsed = failed_tx("InstructionError");
    let outcome = parse_get_transaction_response(&parsed, "sig", Some("WalletA"));
    assert!(matches!(outcome, VerifyWithSignerOutcome::Pending));
}

#[test]
fn test_parse_tx_processed_but_not_confirmed_is_pending() {
    // Some RPCs return "processed" before it reaches "confirmed".
    let parsed = confirmed_tx("WalletA", "processed");
    let outcome = parse_get_transaction_response(&parsed, "sig", Some("WalletA"));
    assert!(matches!(outcome, VerifyWithSignerOutcome::Pending));
}

// ─── parse_get_transaction_response — RpcError paths ─────────────────

#[test]
fn test_parse_rpc_level_error_is_rpc_error() {
    let parsed = serde_json::json!({
        "error": { "code": -32000, "message": "memory allocation failed" }
    });
    let outcome = parse_get_transaction_response(&parsed, "sig", Some("WalletA"));
    assert!(matches!(outcome, VerifyWithSignerOutcome::RpcError));
}

#[test]
fn test_parse_confirmed_but_account_keys_missing_is_rpc_error() {
    // Malformed response: TX is confirmed but no account keys present.
    let parsed = serde_json::json!({
        "result": {
            "transaction": { "message": {} },
            "meta": { "err": null, "confirmationStatus": "confirmed" }
        }
    });
    let outcome = parse_get_transaction_response(&parsed, "sig", Some("WalletA"));
    assert!(matches!(outcome, VerifyWithSignerOutcome::RpcError));
}

#[test]
fn test_parse_confirmed_but_account_keys_empty_is_rpc_error() {
    // Edge case: accountKeys is present but an empty array.
    let parsed = serde_json::json!({
        "result": {
            "transaction": { "message": { "accountKeys": [] } },
            "meta": { "err": null, "confirmationStatus": "confirmed" }
        }
    });
    let outcome = parse_get_transaction_response(&parsed, "sig", Some("WalletA"));
    assert!(matches!(outcome, VerifyWithSignerOutcome::RpcError));
}

// ─── Realistic fixture from islanddao-v4-demo (organizer self-deposit) ─

#[test]
fn test_parse_realistic_devnet_self_deposit_signer_matches() {
    // Mimics the islanddao-v4-demo scenario from the live investigation:
    // organizer deposited on devnet from their own wallet, so the fee-payer
    // (accountKeys[0]) equals the recorded attendee wallet. This is exactly
    // the "web2 missed the event but on-chain is done" case the signer
    // cross-check is designed to recover from.
    let organizer_wallet = "AqdrF1bMEayzZC72R7SxsC2KFqybT5rHPYswkFWe5Mkn";
    let parsed = confirmed_tx(organizer_wallet, "finalized");
    let outcome = parse_get_transaction_response(&parsed, "real-sig", Some(organizer_wallet));
    assert!(outcome.is_confirmed_and_matched());
    assert_eq!(outcome.signer(), Some(organizer_wallet));
}

// ─── parse_signatures_for_address_response — discovery path ──────────
//
// Used by `discover_deposit_tx_on_chain` to find the most recent
// successful TX touching an AttendeeDeposit PDA when the deposit record
// has no tx_signature stored (web2 missed the event, on-chain is done).

fn sig_entry(sig: &str, err: Option<&str>, confirmation: &str) -> serde_json::Value {
    let err_val = match err {
        Some(msg) => serde_json::json!({ "InstructionError": [0, msg] }),
        None => serde_json::Value::Null,
    };
    serde_json::json!({
        "signature": sig,
        "err": err_val,
        "confirmationStatus": confirmation,
        "slot": 12345
    })
}

#[test]
fn test_parse_signatures_returns_most_recent_successful() {
    // Array is ordered newest-first per Solana RPC spec.
    let parsed = serde_json::json!({
        "result": [
            sig_entry("sig-newest", None, "finalized"),
            sig_entry("sig-older", None, "confirmed")
        ]
    });
    let result = parse_signatures_for_address_response(&parsed);
    assert_eq!(result.as_deref(), Some("sig-newest"));
}

#[test]
fn test_parse_signatures_skips_failed_txs() {
    // Newest entry has an on-chain error — should fall through to the
    // next successful one rather than returning the failed signature.
    let parsed = serde_json::json!({
        "result": [
            sig_entry("sig-failed", Some("Custom(1)"), "confirmed"),
            sig_entry("sig-good", None, "confirmed")
        ]
    });
    let result = parse_signatures_for_address_response(&parsed);
    assert_eq!(result.as_deref(), Some("sig-good"));
}

#[test]
fn test_parse_signatures_all_failed_returns_none() {
    let parsed = serde_json::json!({
        "result": [
            sig_entry("sig-1", Some("InsufficientFunds"), "confirmed"),
            sig_entry("sig-2", Some("Custom(1)"), "confirmed")
        ]
    });
    let result = parse_signatures_for_address_response(&parsed);
    assert_eq!(result, None);
}

#[test]
fn test_parse_signatures_empty_result_returns_none() {
    let parsed = serde_json::json!({ "result": [] });
    let result = parse_signatures_for_address_response(&parsed);
    assert_eq!(result, None);
}

#[test]
fn test_parse_signatures_missing_result_returns_none() {
    let parsed = serde_json::json!({});
    let result = parse_signatures_for_address_response(&parsed);
    assert_eq!(result, None);
}

#[test]
fn test_parse_signatures_rpc_error_returns_none() {
    let parsed = serde_json::json!({
        "error": { "code": -32000, "message": "rate limit exceeded" }
    });
    let result = parse_signatures_for_address_response(&parsed);
    assert_eq!(result, None);
}

#[test]
fn test_parse_signatures_null_result_returns_none() {
    // Defensive: some RPCs return null for "no history" rather than [].
    let parsed = serde_json::json!({ "result": null });
    let result = parse_signatures_for_address_response(&parsed);
    assert_eq!(result, None);
}

#[test]
fn test_parse_signatures_single_successful_entry() {
    let parsed = serde_json::json!({
        "result": [sig_entry("only-sig", None, "finalized")]
    });
    let result = parse_signatures_for_address_response(&parsed);
    assert_eq!(result.as_deref(), Some("only-sig"));
}

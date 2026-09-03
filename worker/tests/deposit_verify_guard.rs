//! Guard: every path that flips a USDC deposit to `verified` must go through
//! the guarded read-path recovery, and the webhook must not leak or clobber.
//!
//! `handlers/deposit/usdc/` has two entry points that can set
//! `DepositStatus::verified = true`: the read path (`recover_and_verify_deposit`,
//! called from `/public/ticket` and friends) and the Helius/frontend webhook
//! (`verify_and_confirm_deposit`, detached via `wait_until`).
//!
//! The read path grew two guards the webhook path never received:
//!
//! - **F1** — `verify_attendee_deposit_onchain`: bind verification to the
//!   on-chain `AttendeeDeposit` PDA (amount matches, not refunded). The signer
//!   cross-check alone only proves the wallet signed *a* confirmed tx, so a
//!   self-transfer by the right fee-payer earned a free verified ticket. And
//!   when the deposit record carried no `wallet_address`,
//!   `parse_get_transaction_response` treats "no expected wallet" as a match
//!   (see `test_parse_confirmed_no_expected_wallet_backfills_signer`), so *any*
//!   confirmed signature on the cluster verified the deposit.
//! - **Guard 2** — `binding_conflict`: refuse when the wallet or the signature
//!   is already bound to a different `attendee_id` (plan 003).
//!
//! The fix was to delete the webhook's private copy of the transition and
//! delegate. These tests fail if a future change reintroduces a second,
//! unguarded verification path.

const CONFIRM: &str = include_str!("../src/handlers/deposit/usdc/confirm.rs");
const RECOVER: &str = include_str!("../src/handlers/deposit/usdc/recover.rs");
const WEBHOOK: &str = include_str!("../src/handlers/deposit/usdc/handlers/webhook.rs");

/// Source with `//`, `///` and `//!` lines stripped, so a rule that talks about
/// code is never satisfied (or broken) by prose describing it.
fn code_only(source: &str) -> String {
    source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn webhook_verification_delegates_to_the_guarded_read_path() {
    let code = code_only(CONFIRM);

    assert!(
        code.contains("recover_and_verify_deposit(state, &event, status)"),
        "confirm.rs must delegate the verified transition to \
         `recover_and_verify_deposit` — it is the only implementation carrying \
         the F1 and Guard 2 checks"
    );

    // A private re-implementation always starts here: calling the RPC helper
    // directly and acting on its outcome, instead of handing the record over.
    for banned in ["verify_tx_with_signer", "is_confirmed_and_matched"] {
        assert!(
            !code.contains(banned),
            "confirm.rs calls `{banned}` directly — that is the shape of the \
             unguarded copy of the verification transition that F1 was written \
             to close. Delegate to `recover_and_verify_deposit` instead."
        );
    }

    assert!(
        !code.contains("verified = true"),
        "confirm.rs sets `verified` itself — only `recover_and_verify_deposit` \
         may perform that transition"
    );
}

#[test]
fn confirm_fails_closed_when_the_event_config_is_unavailable() {
    let code = code_only(CONFIRM);
    assert!(
        code.contains("get_event_config_with_fallback"),
        "confirm.rs must load the event config — F1 needs `organizer_wallet`, \
         `on_chain_event_id` and `deposit_amount_usdc` to check the PDA"
    );
    // Both the `Ok(None)` and `Err(_)` arms must bail rather than fall through
    // to a verification that could not have been guarded.
    assert_eq!(
        code.matches("return;").count(),
        4,
        "confirm.rs should bail on exactly four conditions (event config \
         missing / unreadable, deposit status missing / unreadable). A change \
         here means a path now continues without one of them — verify it still \
         fails closed."
    );
}

#[test]
fn the_read_path_still_carries_both_guards() {
    let code = code_only(RECOVER);
    assert!(
        code.contains("verify_attendee_deposit_onchain"),
        "recover.rs lost the F1 on-chain `AttendeeDeposit` PDA check — without \
         it a confirmed-but-unrelated tx by the right fee-payer earns a free \
         verified ticket"
    );
    assert!(
        code.contains("binding_conflict"),
        "recover.rs lost Guard 2 — a single on-chain deposit could then be \
         claimed by several attendee rows (plan 003)"
    );
}

#[test]
fn the_webhook_never_logs_the_authorization_header() {
    let code = code_only(WEBHOOK);
    assert!(
        !code.contains("auth = %auth_header"),
        "the webhook logs the raw Authorization header on rejection. A rejected \
         header is still credential material — a near-miss `WEBHOOK_SECRET`, or \
         a JWT valid on another route — and request logs are a weaker boundary \
         than the secret store. Log only its shape."
    );
    assert!(
        code.contains("auth_present") && code.contains("auth_len"),
        "the webhook should still record the *shape* of a rejected \
         Authorization header, so failures stay diagnosable"
    );
}

#[test]
fn the_webhook_refuses_to_rewrite_a_verified_deposits_signature() {
    let code = code_only(WEBHOOK);
    let assign = code
        .find("deposit_status.tx_signature = Some(")
        .expect("webhook.rs no longer writes `tx_signature` — guard is stale");
    let guard = code.find("if deposit_status.verified {").expect(
        "webhook.rs must refuse to overwrite the signature of an \
             already-verified deposit: the JWT branch authenticates the caller \
             but does not bind them to `body.attendee_id`, so any token holder \
             could otherwise replace a settled money record's proof-of-payment",
    );
    assert!(
        guard < assign,
        "the `verified` check must come *before* the `tx_signature` write, \
         otherwise the clobber has already happened"
    );
}

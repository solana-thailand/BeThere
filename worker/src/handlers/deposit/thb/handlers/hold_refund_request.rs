//! Phase 3 exit path — "Request Return of Held Credit" (Issue #061 §D3),
//! hardened in `.issues/190` (owner option (a), 2026-10-08).
//!
//! Four endpoints:
//!
//!   POST /api/deposit/request-credit-refund        — attendee sets the flag and
//!                                                    says where to send the money
//!   GET  /api/deposit/credit-refund-request        — attendee reads own flag state
//!   GET  /api/deposit/credit-refund-requests       — organizer lists open requests
//!   POST /api/deposit/clear-credit-refund-request  — organizer records the payout
//!
//! ## The flow
//!
//! 1. The attendee asks for their held credit back. If they already gave a
//!    refund account with their THB deposit (copied when the deposit was held
//!    as credit) or on an earlier request, the card shows it masked and one tap
//!    requests the refund to it (`use_saved`). Otherwise — or if they choose
//!    "use a different account" — they give a PromptPay ID or a bank account
//!    (same rules as the THB deposit refund account). The flag and the account
//!    are written in one D1 batch.
//! 2. The organizer sees the request — amount, account, age against the 7-day
//!    promise — in the payout queue (organizers only, scoped to the
//!    organizations they run), transfers the money out-of-band, and clears the
//!    request with the amount they transferred and, optionally, the slip.
//! 3. The clear reverses the credit in ONE guarded ledger statement that only
//!    writes if the payable balance still equals the confirmed amount
//!    (`credit_ledger::try_refund`) — a mismatch is a 409 and writes nothing.
//!    Then it audits who paid, and clears the flag and deletes the account.
//!
//! The THB deposit refund tooling (`/refund/mark`, `/refund/batch-thb`) is not
//! part of this: it refuses held deposits by design.
//!
//! ## Why on `contacts`, not `thb_deposits`?
//!
//! Rolling credit is a cross-event balance — a single contact may hold credit
//! from multiple past deposits across different events. A refund-from-credit
//! request is against the rolling balance, not any specific source deposit.
//!
//! ## Dual-write (D1 + Sheets)
//!
//! - **D1** is the source of truth for every read in this module.
//! - **Sheets** is a display mirror; its writes are best-effort and logged.
//!
//! ## Idempotency
//!
//! Re-calls from the attendee re-stamp `credit_refund_requested_at` and replace
//! the account. A second clear of the same request finds its reversal already
//! in the ledger and writes nothing new.

use axum::{Extension, Json, extract::State};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use serde::{Deserialize, Serialize};

use event_checkin_domain::models::credit_payout::{
    PaidAmounts, RefundAccount, SavedAccountPreview,
};

use crate::error::{ApiOk, WorkerError};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// POST /api/deposit/request-credit-refund (attendee)
// ---------------------------------------------------------------------------

/// Response for the attendee "request return" action.
#[derive(Debug, Serialize)]
pub struct RequestCreditRefundResponse {
    /// Always `true` on success — included for a consistent JSON shape the
    /// frontend can destructure without special-casing.
    pub requested: bool,
    pub message: String,
}

/// Body of the attendee's request: where to send the money — a new
/// `account`, or `use_saved: true` for the account already on file (from their
/// deposit, or an earlier request). A new account wins if both are sent.
///
/// Both are optional in the type only so that a client built before
/// `.issues/190` (which posted `{}`) gets a validation message rather than a
/// bare JSON rejection.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RequestCreditRefundBody {
    #[serde(default)]
    pub account: Option<RefundAccount>,
    #[serde(default)]
    pub use_saved: bool,
}

/// What the attendee asked to be paid to.
enum PayoutTarget {
    New(RefundAccount),
    Saved,
}

/// Turn the body into a target, validating a new account.
fn payout_target(body: RequestCreditRefundBody) -> Result<PayoutTarget, AppError> {
    match (body.account, body.use_saved) {
        (Some(account), _) => account
            .normalized()
            .map(PayoutTarget::New)
            .map_err(|e| AppError::Validation(e.to_string())),
        (None, true) => Ok(PayoutTarget::Saved),
        (None, false) => Err(AppError::Validation(NO_ACCOUNT_MESSAGE.to_string())),
    }
}

/// No account in the request and none on file.
const NO_ACCOUNT_MESSAGE: &str =
    "add a PromptPay ID or a bank account so the organizer can pay you";

/// Attendee requests return of their held rolling credit. Sets the
/// `credit_refund_requested` flag on their own contact row and stores the
/// payout account with it (D1, one batch), then mirrors the flag to Sheets.
///
/// **JWT-gated** — the email comes from `claims.email`, never from the request
/// body (VULN-012 pattern, same as `hold_deposit_handler`). The body carries
/// only the payout account, validated by the same rules as the THB deposit
/// refund account (`credit_payout::RefundAccount::normalized`).
///
/// **Idempotent** — a re-call re-stamps the timestamp and replaces the account.
/// The account number is never logged.
#[worker::send]
pub async fn request_credit_refund_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<RequestCreditRefundBody>,
) -> Result<ApiOk<RequestCreditRefundResponse>, WorkerError> {
    // Issue 070: the contact email is the primary key of this whole flow, so it
    // appears on every branch below. Fingerprint it once under the deployment
    // key — the D1 contact row and the Sheets mirror remain the access-
    // controlled records that hold the address itself.
    let redactor = state.log_redactor();
    let attendee_fingerprint = redactor.fingerprint(&claims.email);

    let target = payout_target(body)?;

    tracing::info!(
        attendee_fingerprint = %attendee_fingerprint,
        method = match &target {
            PayoutTarget::New(new) => new.method().as_str(),
            PayoutTarget::Saved => "saved",
        },
        "credit refund requested (attendee) — setting flag"
    );

    // 1. D1 write — the *only* path the organizer's payout queue reads.
    //    `credit_refund_requests` selects on `contacts.credit_refund_requested`;
    //    it never looks at the sheet. So this write, not the Sheets mirror, is
    //    what decides whether the attendee's money comes back, and it fails
    //    closed on both of its failure modes:
    //
    //      * no D1 binding ⇒ the flag cannot be stored at all;
    //      * a D1 error, or an `UPDATE` matching no contact row (registered via
    //        Sheets without `upsert_contact` firing) — the latter is not an
    //        `Err`, which is why the helper reports rows-affected.
    //
    //    Reporting `requested: true` on either would tell the attendee their
    //    refund is queued while no organizer will ever see it. A 500 is
    //    recoverable — the write is idempotent, so a retry just re-stamps.
    let db = state.d1.as_deref().ok_or_else(|| {
        AppError::Internal("D1 not configured — credit refund request cannot be queued".to_string())
    })?;

    let new_account = match &target {
        PayoutTarget::New(new) => Some(new),
        PayoutTarget::Saved => {
            // Nothing on file (never had one, or it was paid out / purged):
            // the attendee is asked for an account, as before `.issues/190`.
            let saved = crate::db::credit_refund_accounts::chosen_for_person(db, &claims.email)
                .await
                .map_err(AppError::Internal)?;
            if saved.is_none() {
                return Err(AppError::Validation(NO_ACCOUNT_MESSAGE.to_string()).into());
            }
            None
        }
    };
    let flagged = crate::db::contacts::set_credit_refund_requested(db, &claims.email, new_account)
        .await
        .map_err(AppError::Internal)?;

    if !flagged {
        tracing::error!(
            attendee_fingerprint = %attendee_fingerprint,
            "credit refund request matched no contact row — not queued"
        );
        return Err(AppError::Internal(
            "no contact record for this account — credit refund request was not queued".to_string(),
        )
        .into());
    }

    // 2. Sheets write (human-readable master) — a display-only mirror, so every
    //    part of it is best-effort now that step 1 is authoritative and has
    //    already succeeded. The contacts sheet is org-blind and no read path
    //    treats it as authoritative (`docs/deposit-refund-flows.md`), so neither
    //    a missing binding nor a failed write may block or fail the request:
    //    500-ing here would tell the attendee their refund request failed when
    //    it is in fact already in the organizer's queue, and their retry would
    //    re-stamp a timestamp that is serving as the reversal's idempotency key.
    //
    //    The flag is cross-event (on the contact, not any specific deposit), so
    //    the sheet resolves from global config with no event context — mirrors
    //    `credit_balance_handler`.
    let resolved = event_checkin_domain::models::org::ResolvedContactsSheet {
        sheet_id: state.config.sheets.contacts_sheet_id.clone(),
        contacts_sheet_name: state.config.sheets.contacts_sheet_name.clone(),
        events_sheet_name: state.config.sheets.events_sheet_name.clone(),
    };

    match (state.events_kv.as_ref(), resolved.sheet_id.is_empty()) {
        (Some(kv), false) => {
            if let Err(e) = crate::sheets::contacts::set_credit_refund_requested(
                &state,
                &resolved.sheet_id,
                &resolved.contacts_sheet_name,
                Some(kv),
                &claims.email,
            )
            .await
            {
                tracing::warn!(
                    attendee_fingerprint = %attendee_fingerprint,
                    error = %e,
                    "Sheets credit-refund-request mirror failed (non-fatal; D1 queue already has it)"
                );
            }
        }
        _ => tracing::warn!(
            attendee_fingerprint = %attendee_fingerprint,
            "contacts sheet or EVENTS KV not configured — skipping credit-refund-request mirror"
        ),
    }

    tracing::info!(
        attendee_fingerprint = %attendee_fingerprint,
        "credit refund requested flag set on contact"
    );

    // The request itself is not audited — the flag and the account row are its
    // record. The money-moving step, the payout, is: `audit_payout` in the
    // clear handler (`.issues/190`).

    Ok(ApiOk::new(RequestCreditRefundResponse {
        requested: true,
        message: "Your request has been recorded. The organizer will process your refund."
            .to_string(),
    }))
}

// ---------------------------------------------------------------------------
// GET /api/deposit/credit-refund-request (attendee)
// ---------------------------------------------------------------------------

/// Response for the attendee's own flag-state read.
#[derive(Debug, Serialize)]
pub struct CreditRefundRequestStatus {
    /// Whether the attendee has an open "credit refund requested" flag.
    pub requested: bool,
    /// The payout account on file, **masked** (bank, last four digits, holder's
    /// first name and initial, where it came from and when). Never the full
    /// number: this endpoint is the attendee's, and a session is not proof of
    /// owning the bank account. `None` when nothing is on file.
    pub saved_account: Option<SavedAccountPreview>,
}

/// Returns whether the authenticated attendee has an open "credit refund
/// requested" flag — backs the ticket page's `RequestCreditRefundCard`
/// already-requested state on reload (mirrors the `held_as_credit` UX pattern
/// — Issue #061 idempotency).
///
/// Reads from D1 only. If D1 is unreachable, returns `requested: false` so the
/// attendee sees the CTA (the defense-in-depth backstop is the idempotent
/// write — re-requesting is a no-op that refreshes the timestamp).
#[worker::send]
pub async fn credit_refund_request_status_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<CreditRefundRequestStatus>, WorkerError> {
    let (requested, saved_account) = match state.d1.as_deref() {
        Some(db) => {
            let requested =
                crate::db::contacts::get_credit_refund_requested(db, &claims.email).await;
            // A failed read shows the form instead of the one-tap card: the
            // attendee types an account, which loses nothing.
            let saved = crate::db::credit_refund_accounts::preview_for_person(db, &claims.email)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        attendee_fingerprint = %state.log_fingerprint(&claims.email),
                        error = %e,
                        "saved payout account read failed — showing the form"
                    );
                    None
                });
            (requested, saved)
        }
        // No D1 → cannot read the flag. Degrade to `false` so the attendee
        // sees the CTA rather than a broken "already requested" state. The
        // write path is idempotent so a false-negative just means they can
        // re-trigger (which re-stamps the timestamp — no harm).
        None => (false, None),
    };

    Ok(ApiOk::new(CreditRefundRequestStatus {
        requested,
        saved_account,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/deposit/credit-refund-requests (admin)
// ---------------------------------------------------------------------------

/// Response for the admin "credit refund requested" listing. Cross-event
/// (global) — backs the badge on the Held-as-Credit tab (Issue #061 Phase 3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditRefundRequestsResponse {
    /// Contacts with an open `credit_refund_requested` flag, ordered by most
    /// recent request first (newest `credit_refund_requested_at`).
    pub requests: Vec<crate::db::contacts::CreditRefundRequest>,
}

/// Lists the open "credit refund requested" rows the caller may pay out —
/// backs the payout queue on the Held-as-Credit tab.
///
/// Organizers only, org-scoped (`credit_payout::payout_scope`): a row is shown
/// only when every organization the person's credit belongs to is in the
/// caller's scope, because the row carries the attendee's account number and
/// clearing it moves all of that credit. A per-event scanner gets 403.
///
/// Returns an empty list when D1 is unreachable so the admin view still
/// renders.
#[worker::send]
pub async fn credit_refund_requests_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<CreditRefundRequestsResponse>, WorkerError> {
    tracing::info!(
        staff_fingerprint = %state.log_fingerprint(&claims.email),
        "credit refund requests listed (admin)"
    );
    let scope = super::credit_payout::payout_scope(&state, &claims.email).await?;
    super::credit_payout::require_payout_operator(&scope)?;

    let requests = match state.d1.as_deref() {
        Some(db) => crate::db::contacts::credit_refund_requests(db)
            .await
            .into_iter()
            .filter(|row| {
                // An unparseable org list is hidden from every scoped caller.
                row.organization_ids()
                    .is_some_and(|orgs| scope.covers(orgs.iter().map(String::as_str)))
                    || scope == event_checkin_domain::models::credit_payout::PayoutScope::All
            })
            .collect(),
        // No D1 → cannot read the queue. Degrade to empty (the badge hides
        // when count == 0).
        None => Vec::new(),
    };

    Ok(ApiOk::new(CreditRefundRequestsResponse { requests }))
}

// ---------------------------------------------------------------------------
// POST /api/deposit/clear-credit-refund-request (admin)
// ---------------------------------------------------------------------------

/// Request body for the organizer's "paid — clear the request" action. The
/// contact is identified by email in the body (emails are awkward in a path).
#[derive(Debug, Clone, Deserialize)]
pub struct ClearCreditRefundRequest {
    pub email: String,
    /// What the organizer actually transferred, per currency (`.issues/190`).
    /// Required; `Option` only so an old client gets a message, not a JSON
    /// rejection. The reversal writes only if this equals the payable balance
    /// at the moment of the write.
    #[serde(default)]
    pub paid: Option<PaidAmounts>,
    /// The organizer's transfer slip as an uploaded image (`data:` URL).
    #[serde(default)]
    pub proof: Option<String>,
}

/// Response for the admin "clear request" action.
#[derive(Debug, Serialize)]
pub struct ClearCreditRefundResponse {
    /// Always `true` on success — included for a consistent JSON shape.
    pub cleared: bool,
    pub message: String,
}

/// The organizer records a held-credit payout and clears the request
/// (Issue #061 §D3, hardened in `.issues/190`).
///
/// Organizers only, org-scoped like the queue: the caller must cover every
/// organization the credit belongs to.
///
/// **The guarded reversal gates the clear.** The organizer confirms what they
/// transferred (`paid`); `credit_payout::settle_payout` writes only if that still
/// equals the payable balance, in the same statement as the write, so a
/// registration that spent the credit after the organizer looked turns into a
/// 409 with nothing recorded instead of a reversal of a number nobody paid.
/// Any failure aborts before the flag is touched.
///
/// Then the payout is audited (who, how much, the slip) and the flag cleared
/// with the payout account deleted. The flag clears stay best-effort (logged,
/// not fatal): they are idempotent, and the nightly purge deletes an account
/// whose request is no longer open.
#[worker::send]
pub async fn clear_credit_refund_request_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<ClearCreditRefundRequest>,
) -> Result<ApiOk<ClearCreditRefundResponse>, WorkerError> {
    // Issue 070: both the acting admin and the target contact are identified by
    // email. The audit trail keeps the attributable actor under its own access
    // controls; the general log stream gets fingerprints only.
    let redactor = state.log_redactor();
    let staff_fingerprint = redactor.fingerprint(&claims.email);
    let target_fingerprint = redactor.fingerprint(&body.email);

    tracing::info!(
        staff_fingerprint = %staff_fingerprint,
        target_fingerprint = %target_fingerprint,
        "admin clearing credit refund request flag"
    );

    let scope = super::credit_payout::payout_scope(&state, &claims.email).await?;
    super::credit_payout::require_payout_operator(&scope)?;
    let paid = body.paid.ok_or_else(|| {
        AppError::Validation(
            "confirm the amount you transferred before clearing the request".to_string(),
        )
    })?;
    paid.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let db = state
        .d1
        .as_deref()
        // No D1 ⇒ the ledger is unreachable ⇒ the reversal cannot be written.
        // Fail closed rather than clear the flag: an unreversed clear is a
        // double payout, and the request staying in the queue is recoverable.
        .ok_or_else(|| AppError::Internal("D1 not configured".to_string()))?;

    if let Some(requested_at) =
        crate::db::contacts::get_credit_refund_requested_at(db, &body.email).await
    {
        let buckets = super::credit_payout::scoped_buckets(db, &scope, &body.email).await?;

        // Issue #120 §3. Nothing to reverse *and* credit still locked to an
        // event that has not ended means the organizer is about to clear a
        // request that pays out nothing: the reversal writes no rows, the flag
        // goes away, and when the event ends the credit comes back with no open
        // request and no record that one was ever made. Refuse, keep the flag,
        // and name the event the money is waiting on. A genuinely empty request
        // (no balance, nothing locked) still clears — that is the organizer
        // dismissing a stale row, and it loses nothing.
        if buckets.is_empty() {
            let locked = crate::db::credit_ledger::locked_applies(db, &body.email)
                .await
                .map_err(AppError::Internal)?;
            if let Some(last) = locked.first() {
                // Per currency, not one sum: the ledger is multi-currency, and
                // a single "1500" spanning THB and USDC is a number that means
                // nothing to the organizer reading it.
                let mut totals: std::collections::BTreeMap<&str, i64> =
                    std::collections::BTreeMap::new();
                for entry in &locked {
                    *totals.entry(entry.currency.as_str()).or_default() += entry.amount;
                }
                let amount = totals
                    .iter()
                    .map(|(currency, total)| format!("{total} {}", currency.to_uppercase()))
                    .collect::<Vec<_>>()
                    .join(" + ");
                tracing::warn!(
                    staff_fingerprint = %staff_fingerprint,
                    target_fingerprint = %target_fingerprint,
                    locked = %amount,
                    locked_events = locked.len(),
                    "refused to clear credit refund request — balance is 0 and credit is still locked"
                );
                // The events mirror can be missing the row; the id is a slug,
                // so it still names something the organizer can look up.
                let event = match last.event_name.is_empty() {
                    true => last.event_id.clone(),
                    false => last.event_name.clone(),
                };
                return Err(AppError::Conflict(format!(
                    "nothing to pay out yet — {amount} is covering {event} and returns when \
                     that event ends. The request stays open until then."
                ))
                .into());
            }
        }

        super::credit_payout::settle_payout(
            &state,
            db,
            &claims.email,
            &body.email,
            &requested_at,
            paid,
            body.proof.as_deref(),
            &buckets,
            super::credit_payout::PayoutInitiator::Attendee,
        )
        .await?;
    }

    // D1 clear — source of truth. Best-effort log on failure: an unreachable
    // D1 is non-fatal for the response shape (the organizer sees the request
    // disappear from the admin list on next refresh either way), but we still
    // surface success because the action is idempotent — a retry on next
    // refresh will pick up the clear.
    if let Err(e) = crate::db::contacts::clear_credit_refund_requested(db, &body.email).await {
        tracing::warn!(
            target_fingerprint = %target_fingerprint,
            error = %e,
            "D1 clear_credit_refund_requested failed — flag may persist"
        );
    }

    // Sheets clear — human-readable master (column N). Best-effort, mirroring
    // the D1 clear's leniency: a transient Sheets outage must NOT block the
    // admin's clear action or surface as a 500. The D1 clear above is the
    // source of truth; a logged Sheets miss is reconciliation-cosmetic and
    // will be picked up on the next clear retry. Skipping when KV/sheet is
    // unconfigured (rather than erroring) preserves the clear's idempotent
    // "always succeeds" contract.
    let resolved = event_checkin_domain::models::org::ResolvedContactsSheet {
        sheet_id: state.config.sheets.contacts_sheet_id.clone(),
        contacts_sheet_name: state.config.sheets.contacts_sheet_name.clone(),
        events_sheet_name: state.config.sheets.events_sheet_name.clone(),
    };

    if state.events_kv.is_some() && !resolved.sheet_id.is_empty() {
        if let Err(e) = crate::sheets::contacts::clear_credit_refund_requested(
            &state,
            &resolved.sheet_id,
            &resolved.contacts_sheet_name,
            state.events_kv.as_ref(),
            &body.email,
        )
        .await
        {
            tracing::warn!(
                target_fingerprint = %target_fingerprint,
                error = %e,
                "Sheets clear_credit_refund_requested failed — column N may stay stale"
            );
        }
    } else {
        tracing::debug!(
            target_fingerprint = %target_fingerprint,
            "Sheets clear skipped (KV or contacts sheet not configured)"
        );
    }

    tracing::info!(
        staff_fingerprint = %staff_fingerprint,
        target_fingerprint = %target_fingerprint,
        "credit refund request flag cleared"
    );

    Ok(ApiOk::new(ClearCreditRefundResponse {
        cleared: true,
        message: "Credit refund request cleared.".to_string(),
    }))
}

//! Phase 3 exit path — "Request Return of Held Credit" (Issue #061 §D3).
//!
//! Four endpoints:
//!
//!   POST /api/deposit/request-credit-refund        — attendee sets the flag (own contact)
//!   GET  /api/deposit/credit-refund-request        — attendee reads own flag state
//!   GET  /api/deposit/credit-refund-requests       — admin lists all open requests
//!   POST /api/deposit/clear-credit-refund-request  — admin clears the flag after payout
//!
//! ## Why a visibility-only flag (not a payout endpoint)?
//!
//! Issue #061 §D3 resolved this as a *visibility-only* signal — attendees need
//! an exit so "hold forever" doesn't feel like a trap (Issue #032 trust risk),
//! but automated payout = cash-on-hand liability + queue complexity not needed
//! for v1. The organizer processes the actual payout through the existing THB
//! refund queue tooling (`POST /api/refund/mark/{attendee_id}` /
//! `/refund/batch-thb`); this flag is the queue signal only.
//!
//! ## Why on `contacts`, not `thb_deposits`?
//!
//! Rolling credit is a cross-event balance — a single contact may hold credit
//! from multiple past deposits across different events. A refund-from-credit
//! request is against the rolling balance, not any specific source deposit.
//! Mirrors the existing `deposit_credit_thb/usdc/since` columns K–M on the
//! same table (Issue #032 architecture decision).
//!
//! ## Dual-write (D1 + Sheets)
//!
//! - **D1** is the source of truth for the admin/attendee reads in this module
//!   (consistent with `credit_liability`'s D1-only read path, handover 104).
//! - **Sheets** is the human-readable master; the write is best-effort but logged.
//!   A failed/non-existent D1 contact row degrades to a Sheets-only record —
//!   the attendee's request is still visible to a human scanning column N, and
//!   the admin badge may miss it (same trade-off as the credit-liability chip).
//!
//! ## Idempotency
//!
//! Re-calls from the attendee just re-stamp `credit_refund_requested_at` —
//! surfaces "still waiting" to the organizer without a per-click counter. The
//! organizer clears the flag manually after processing the payout through the
//! existing refund tooling (`clear_credit_refund_requested` in `db/contacts.rs`).

use axum::{Extension, Json, extract::State};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use serde::{Deserialize, Serialize};

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

/// Attendee requests return of their held rolling credit. Sets the
/// `credit_refund_requested` flag on their own contact row (D1 + Sheets
/// dual-write).
///
/// **JWT-gated** — the email comes from `claims.email`, never from the request
/// body (VULN-012 pattern, same as `hold_deposit_handler`). No body needed: the
/// flag is on the contact (cross-event), not event-scoped.
///
/// **Idempotent** — a re-call re-stamps the timestamp (surfaces "still waiting"
/// to the organizer without a per-click counter for v1).
#[worker::send]
pub async fn request_credit_refund_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<RequestCreditRefundResponse>, WorkerError> {
    tracing::info!(
        email = %claims.email,
        "credit refund requested (attendee) — setting flag"
    );

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;

    // 1. Resolve contacts sheet from global config. The flag is cross-event
    //    (on the contact, not any specific deposit), so no event context is
    //    needed — mirrors `credit_balance_handler`.
    let resolved = event_checkin_domain::models::org::ResolvedContactsSheet {
        sheet_id: state.config.sheets.contacts_sheet_id.clone(),
        contacts_sheet_name: state.config.sheets.contacts_sheet_name.clone(),
        events_sheet_name: state.config.sheets.events_sheet_name.clone(),
    };

    if resolved.sheet_id.is_empty() {
        return Err(AppError::Internal("contacts sheet not configured".to_string()).into());
    }

    // 2. D1 write (source of truth for reads in this module). Best-effort:
    //    if the contact row doesn't exist in D1 yet (edge case — registered
    //    via Sheets but the `upsert_contact` path didn't fire), the UPDATE
    //    affects 0 rows silently. Log and continue to the Sheets write so the
    //    attendee's request is still recorded somewhere.
    if let Some(db) = state.d1.as_deref()
        && let Err(e) = crate::db::contacts::set_credit_refund_requested(db, &claims.email).await
    {
        tracing::warn!(
            email = %claims.email,
            error = %e,
            "D1 set_credit_refund_requested failed — falling back to Sheets-only write"
        );
    }

    // 3. Sheets write (human-readable master). This is the canonical visibility
    //    path matching the existing `increment_credit` pattern — if this fails,
    //    the attendee's request is not recorded and they should see an error.
    crate::sheets::contacts::set_credit_refund_requested(
        &state,
        &resolved.sheet_id,
        &resolved.contacts_sheet_name,
        Some(kv),
        &claims.email,
    )
    .await
    .map_err(AppError::Internal)?;

    tracing::info!(
        email = %claims.email,
        "credit refund requested flag set on contact"
    );

    // Audit entry deferred — the flag itself IS the persistent record for v1.
    // A dedicated `AuditAction::CreditRefundRequested` variant is a follow-up
    // (requires touching the enum + all its consumers; out of scope for the
    // self-contained Phase 3 exit path).

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
    let requested = match state.d1.as_deref() {
        Some(db) => crate::db::contacts::get_credit_refund_requested(db, &claims.email).await,
        // No D1 → cannot read the flag. Degrade to `false` so the attendee
        // sees the CTA rather than a broken "already requested" state. The
        // write path is idempotent so a false-negative just means they can
        // re-trigger (which re-stamps the timestamp — no harm).
        None => false,
    };

    Ok(ApiOk::new(CreditRefundRequestStatus { requested }))
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

/// Lists all contacts with an open "credit refund requested" flag — backs the
/// admin badge on the Held-as-Credit tab. Cross-event (global), one D1
/// round-trip via the partial index `idx_contacts_credit_refund_requested`
/// (migration `0023`). Returns an empty list when D1 is unreachable so the
/// admin view always renders.
#[worker::send]
pub async fn credit_refund_requests_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<CreditRefundRequestsResponse>, WorkerError> {
    tracing::info!(admin_email = %claims.email, "credit refund requests listed (admin)");

    let requests = match state.d1.as_deref() {
        Some(db) => crate::db::contacts::credit_refund_requests(db).await,
        // No D1 → cannot read the queue. Degrade to empty (the badge hides
        // when count == 0). Logged upstream if needed.
        None => Vec::new(),
    };

    Ok(ApiOk::new(CreditRefundRequestsResponse { requests }))
}

// ---------------------------------------------------------------------------
// POST /api/deposit/clear-credit-refund-request (admin)
// ---------------------------------------------------------------------------

/// Request body for the admin clear endpoint. The contact is identified by
/// email (the `contacts` primary key) — using a JSON body rather than a path
/// parameter because emails can contain characters that are awkward to URL-
/// encode in a path segment. Mirrors the `AdminHoldRequest` body shape.
#[derive(Debug, Clone, Deserialize)]
pub struct ClearCreditRefundRequest {
    pub email: String,
}

/// Response for the admin "clear request" action.
#[derive(Debug, Serialize)]
pub struct ClearCreditRefundResponse {
    /// Always `true` on success — included for a consistent JSON shape.
    pub cleared: bool,
    pub message: String,
}

/// Admin clears the `credit_refund_requested` flag on a contact after
/// processing the payout through the existing refund tooling (Issue #061 §D3).
/// Sets the flag to 0 and nulls the timestamp so a subsequent attendee request
/// starts a fresh timestamp.
///
/// **Admin/staff-authed** via `resolve_event_with_access` is not appropriate
/// here — the contact is cross-event (not scoped to one event), so there is no
/// event_id to resolve against. The route is registered in the `protected`
/// router block which already requires staff auth via `require_staff`.
///
/// **Dual-write (D1 + Sheets)** — mirrors `request_credit_refund_handler`'s
/// write path: D1 is source of truth, Sheets is the human-readable master.
/// Both writes are best-effort (logged, not fatal): the clear is idempotent,
/// so a transient failure on either store is recovered by the next refresh's
/// retry. The read paths (admin badge, attendee status) read from D1, which
/// is why a Sheets lag is reconciliation-cosmetic rather than a correctness
/// issue.
#[worker::send]
pub async fn clear_credit_refund_request_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<ClearCreditRefundRequest>,
) -> Result<ApiOk<ClearCreditRefundResponse>, WorkerError> {
    tracing::info!(
        admin_email = %claims.email,
        target_email = %body.email,
        "admin clearing credit refund request flag"
    );

    // Reverse the held credit in the ledger BEFORE clearing the flag. Clearing
    // the request == the organizer paid the credit back out-of-band, so it must
    // leave the ledger — otherwise the attendee keeps usable credit AND got the
    // cash (double payout). Read requested_at while the flag is still set and use
    // it as the idempotency key: a double-clear finds the flag already gone
    // (None) and skips, so the reversal happens exactly once per request.
    // Single-org scope today (organization_id ""); multi-org would reverse per org.
    if let Some(db) = state.d1.as_deref()
        && let Some(requested_at) =
            crate::db::contacts::get_credit_refund_requested_at(db, &body.email).await
    {
        let bal = crate::db::credit_ledger::balance(db, &body.email, "", "thb")
            .await
            .unwrap_or(0);
        if bal > 0 {
            let key = format!("refund:{}:{}", body.email.to_lowercase(), requested_at);
            match crate::db::credit_ledger::record(
                db,
                &body.email,
                "",
                "thb",
                -bal,
                crate::db::credit_ledger::REASON_REFUND,
                None,
                Some(&key),
                Some("held-credit payout processed by organizer"),
            )
            .await
            {
                Ok(_) => tracing::info!(email = %body.email, amount = bal, "reversed held credit in ledger on payout"),
                Err(e) => tracing::error!(email = %body.email, error = %e, "ledger refund reversal failed — credit NOT reversed; reconcile manually"),
            }
        }
    }

    // D1 clear — source of truth. Best-effort log on failure: an unreachable
    // D1 is non-fatal for the response shape (the organizer sees the request
    // disappear from the admin list on next refresh either way), but we still
    // surface success because the action is idempotent — a retry on next
    // refresh will pick up the clear.
    if let Some(db) = state.d1.as_deref()
        && let Err(e) = crate::db::contacts::clear_credit_refund_requested(db, &body.email).await
    {
        tracing::warn!(
            email = %body.email,
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
                email = %body.email,
                error = %e,
                "Sheets clear_credit_refund_requested failed — column N may stay stale"
            );
        }
    } else {
        tracing::debug!(
            email = %body.email,
            "Sheets clear skipped (KV or contacts sheet not configured)"
        );
    }

    tracing::info!(
        admin_email = %claims.email,
        target_email = %body.email,
        "credit refund request flag cleared"
    );

    Ok(ApiOk::new(ClearCreditRefundResponse {
        cleared: true,
        message: "Credit refund request cleared.".to_string(),
    }))
}

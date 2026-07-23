//! Admin THB slip upload handler — record slip on behalf of attendee.
//!
//! Endpoint: `POST /api/deposit/thb/admin-upload`
//!
//! Sibling of `slip_upload::upload_thb_slip_handler`, but:
//! - Mounted on the **protected** router (staff auth via `require_auth`)
//! - Uses `resolve_event_with_access` for per-event organizer gate
//! - **Skips the VULN-012 email-match check** (admin is acting on attendee's
//!   behalf — audited via `AuditAction::SlipRecordedByAdmin` instead)
//! - Supports `auto_verify` flag (default `true`): when true, marks the deposit
//!   verified AND fires the same side effects as `verify_thb_slip_handler`
//!   (QR generation, sheet mirror, D1 dual-write)
//!
//! ## Why this exists
//!
//! Attendees sometimes cannot upload their own slip:
//! - JWT expired (24h) and they can't sign in
//! - Browser/SW bug blocking the upload UI
//! - Sent the slip via LINE/email and never logged into the platform
//!
//! Without this path the organizer is stuck — they have a confirmed payment
//! but no way to record it in the system. Recording requires admin privileges
//! and is audited with the admin's email as the actor.
//!
//! ## Safety
//!
//! - Staff auth required (`require_auth` middleware on the protected router)
//! - Per-event organizer gate (`resolve_event_with_access`)
//! - Rejects duplicates (same as attendee endpoint — no admin override)
//! - Bank info required (refund pipeline depends on it)
//! - Deposit deadline/reclaim flow mirrored exactly from attendee endpoint
//! - All actions audited via `AuditAction::SlipRecordedByAdmin`

use axum::{Extension, Json, extract::State};
use chrono::Utc;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::{DepositMethod, DepositStatus, ThbDeposit};
use event_checkin_domain::models::error::AppError;

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::handlers::ext::resolve_event_with_access;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// POST /api/deposit/thb/admin-upload
// ---------------------------------------------------------------------------

/// Request body for admin slip upload. Mirrors `ThbSlipUploadRequest` plus
/// an `auto_verify` flag (defaults to `true` — admin recording a confirmed
/// payment typically also verifies it in the same call).
#[derive(Debug, serde::Deserialize)]
pub struct AdminSlipUploadRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// Slip image as a data URL (will be uploaded to R2) or an HTTPS URL.
    pub slip_url: String,
    /// Bank account number for THB refund (required).
    #[serde(default)]
    pub bank_account: Option<String>,
    /// Bank name for THB refund (required).
    #[serde(default)]
    pub bank_name: Option<String>,
    /// Account holder name for THB refund (required).
    #[serde(default)]
    pub account_name: Option<String>,
    /// Whether to also verify the deposit in the same call. Default: `true`.
    /// When `false`, the deposit is recorded but pending review (same as the
    /// attendee endpoint) — the admin must verify via `/deposit/thb/verify`.
    #[serde(default = "default_auto_verify")]
    pub auto_verify: bool,
}

fn default_auto_verify() -> bool {
    true
}

/// Admin records a THB payment slip on behalf of an attendee.
///
/// Use case: attendee sent the slip via off-platform channel (LINE/email)
/// and cannot upload themselves (JWT expired, browser bug, etc.).
///
/// Auth: staff (via `require_auth` on the protected router) + per-event
/// organizer gate via `resolve_event_with_access`.
///
/// Audit: emits `AuditAction::SlipRecordedByAdmin` with the admin's email as
/// actor and a note indicating whether auto-verify was applied.
#[worker::send]
pub async fn admin_upload_thb_slip_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<AdminSlipUploadRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %body.event_id,
        admin_email = %claims.email,
        auto_verify = body.auto_verify,
        "admin slip upload initiated"
    );

    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;
    let d1 = state.d1.as_deref();

    // 1. Staff auth + per-event organizer gate.
    let event = resolve_event_with_access(&state, &claims, Some(&body.event_id)).await?;

    if !event.deposit_enabled {
        return Err(AppError::Validation("deposit not enabled for this event".to_string()).into());
    }

    if event.deposit_amount_thb == 0 {
        return Err(AppError::Validation("THB deposit amount not configured".to_string()).into());
    }

    // 2. Validate slip URL (MIME, size, no SVG/XSS) — same gate as attendee.
    super::slip_upload::validate_slip_url(&body.slip_url)?;

    // 3. Bank info required (refund pipeline depends on it — same as attendee).
    if body
        .bank_account
        .as_ref()
        .is_none_or(|v| v.trim().is_empty())
    {
        return Err(AppError::Validation("bank_account is required".to_string()).into());
    }
    if body.bank_name.as_ref().is_none_or(|v| v.trim().is_empty()) {
        return Err(AppError::Validation("bank_name is required".to_string()).into());
    }
    if body
        .account_name
        .as_ref()
        .is_none_or(|v| v.trim().is_empty())
    {
        return Err(AppError::Validation("account_name is required".to_string()).into());
    }

    // 4. Verify attendee exists (NO email-match check — admin path).
    //    The attendee lookup also gives us row_index, registration_date, and
    //    qr_code_url — all needed downstream.
    let attendee = crate::sheets::get_attendee_by_id(
        &body.attendee_id,
        &state,
        &event.sheet_id,
        &event.sheet_name,
        Some(kv),
    )
    .await
    .map_err(AppError::Internal)?
    .ok_or_else(|| AppError::NotFound(format!("attendee '{}' not found", body.attendee_id)))?;

    // 5. Reject duplicates (same as attendee endpoint — no admin override).
    let existing = event_store::get_deposit_status(kv, &event.id, &body.attendee_id, d1)
        .await
        .map_err(AppError::Internal)?;
    if existing.is_some() {
        return Err(AppError::Validation("attendee already has a deposit".to_string()).into());
    }

    // 6. Deposit deadline check: reject OR reclaim (mirror attendee endpoint).
    //    If deadline expired but in-person capacity is still available, switch
    //    the attendee back to In-Person and allow the deposit (reclaim flow).
    //    If capacity is full, reject — the attendee was already moved online.
    if let Some(deadline_hours) = event.deposit_deadline_hours
        && let Some(reg_str) = &attendee.registration_date
        && let Ok(reg_time) = chrono::DateTime::parse_from_rfc3339(reg_str)
    {
        let deadline = reg_time.with_timezone(&chrono::Utc)
            + chrono::Duration::hours(i64::from(deadline_hours));
        if chrono::Utc::now() > deadline {
            let capacity_available = if let Some(cap) = event.in_person_capacity {
                let in_person_count = crate::sheets::get_attendees_for_event(
                    &state,
                    &event.sheet_id,
                    &event.sheet_name,
                    Some(kv),
                    &event.id,
                )
                .await
                .map(|a| a.iter().filter(|a| a.is_in_person()).count() as u32)
                .unwrap_or(u32::MAX);
                in_person_count < cap
            } else {
                true
            };

            if capacity_available {
                if let Ok(mapping) = crate::sheets::get_column_mapping(
                    &state,
                    &event.sheet_id,
                    &event.sheet_name,
                    Some(kv),
                )
                .await
                {
                    if let Some(ctx) = &state.worker_ctx {
                        ctx.wait_until(crate::sheets::bg_sync::update_participation_type(
                            state.clone(),
                            attendee.row_index,
                            "In-Person".to_string(),
                            mapping,
                            event.sheet_id.clone(),
                            event.sheet_name.clone(),
                            Some(kv.clone()),
                        ));
                        tracing::info!(
                            attendee_id = %attendee.api_id,
                            "admin upload deadline reclaim: switched back to In-Person (bg)"
                        );
                    } else {
                        match crate::sheets::write::update_participation_type(
                            attendee.row_index,
                            "In-Person",
                            &mapping,
                            &state,
                            &event.sheet_id,
                            &event.sheet_name,
                            Some(kv),
                        )
                        .await
                        {
                            Ok(()) => tracing::info!(
                                attendee_id = %attendee.api_id,
                                "admin upload deadline reclaim: switched back to In-Person"
                            ),
                            Err(e) => tracing::warn!(
                                attendee_id = %attendee.api_id,
                                error = %e,
                                "admin upload deadline reclaim: failed to switch back"
                            ),
                        }
                    }
                }
            } else {
                return Err(AppError::Validation(
                    "deposit deadline has passed and in-person spots are now full".to_string(),
                )
                .into());
            }
        }
    }

    // 7. Upload slip image to R2 if available (reduces KV storage by ~6x).
    let slip_url = super::maybe_upload_to_r2(
        &state,
        &event.id,
        &body.attendee_id,
        &body.slip_url,
        crate::storage::PREFIX_SLIPS,
    )
    .await;

    let now = Utc::now().to_rfc3339();
    let verified = body.auto_verify;

    // 8. Create + persist THB deposit record.
    //    When auto_verify=true, mark verified with admin as verifier — mirrors
    //    what verify_thb_slip_handler would do as a second call.
    let thb_deposit = ThbDeposit {
        attendee_id: body.attendee_id.clone(),
        event_id: event.id.clone(),
        amount_thb: event.deposit_amount_thb,
        slip_url: Some(slip_url),
        verified,
        verified_by: if verified {
            Some(claims.email.clone())
        } else {
            None
        },
        verified_at: if verified { Some(now.clone()) } else { None },
        uploaded_at: now.clone(),
        refunded: false,
        refunded_at: None,
        held_as_credit: false,
        held_as_credit_at: None,
        attendee_name: None,
        bank_account: body.bank_account.clone(),
        bank_name: body.bank_name.clone(),
        account_name: body.account_name.clone(),
        refund_proof_url: None,
    };

    event_store::save_thb_deposit(kv, &thb_deposit, d1)
        .await
        .map_err(AppError::Internal)?;

    // 9. Write bank info to Google Sheet for organizer refund reference.
    if let Ok(mapping) =
        crate::sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, Some(kv))
            .await
    {
        if let Some(ctx) = &state.worker_ctx {
            ctx.wait_until(crate::sheets::bg_sync::write_bank_info(
                state.clone(),
                attendee.row_index,
                body.bank_account.clone(),
                body.bank_name.clone(),
                mapping,
                event.sheet_id.clone(),
                event.sheet_name.clone(),
                Some(kv.clone()),
            ));
        } else if let Err(e) = crate::sheets::write::write_bank_info(
            attendee.row_index,
            body.bank_account.as_deref(),
            body.bank_name.as_deref(),
            body.account_name.as_deref(),
            &mapping,
            &state,
            &event.sheet_id,
            &event.sheet_name,
            Some(kv),
        )
        .await
        {
            tracing::warn!(
                attendee_id = %body.attendee_id,
                error = %e,
                "failed to write bank info to sheet (non-blocking)"
            );
        }
    }

    // 10. Atomically increment deposit counter for this event.
    let deposit_order = event_store::increment_deposit_counter_with_fallback(
        Some(kv),
        state.d1.as_deref(),
        &event.id,
    )
    .await
    .map_err(AppError::Internal)?;

    let refundable =
        event.max_refundable_deposits == 0 || deposit_order <= event.max_refundable_deposits;

    // 11. Create + persist deposit status.
    let deposit_status = DepositStatus {
        attendee_id: body.attendee_id.clone(),
        event_id: event.id.clone(),
        method: DepositMethod::Thb,
        amount: event.deposit_amount_thb,
        currency: "THB".to_string(),
        tx_signature: None,
        verified,
        deposited_at: now.clone(),
        wallet_address: None,
        deposit_order,
        refundable,
        rejected: false,
    };

    event_store::save_deposit_status(kv, &deposit_status, d1)
        .await
        .map_err(AppError::Internal)?;

    // 12. Write deposit columns (N=method, O=amount, Q=verified) to Google Sheet.
    //     Mirrors D1 state so the sheet stays in sync. QR auto-generation only
    //     fires when auto_verify=true (sibling of verify_thb_slip_handler).
    let deposit_amount_thb = event.deposit_amount_thb.to_string();
    if let Ok(mapping) =
        crate::sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, Some(kv))
            .await
    {
        if let Some(wctx) = &state.worker_ctx {
            wctx.wait_until(crate::sheets::bg_sync::write_deposit_verification(
                state.clone(),
                attendee.row_index,
                "THB".to_string(),
                deposit_amount_thb.clone(),
                verified,
                mapping.clone(),
                event.sheet_id.clone(),
                event.sheet_name.clone(),
                Some(kv.clone()),
            ));

            // 13. Auto-generate QR if verifying and attendee doesn't have one.
            //     D1 write is inline so the ticket page sees the QR immediately.
            if verified && attendee.qr_code_url.as_ref().is_none_or(|u| u.is_empty()) {
                let server_url = &state.config.server.url;
                let qr_url = format!("{server_url}/staff/?scan={}", attendee.api_id);

                if let Some(ref d1) = state.d1
                    && let Err(e) =
                        crate::db::attendees::set_qr_url(d1, &attendee.api_id, &qr_url).await
                {
                    tracing::warn!(
                        attendee_id = %attendee.api_id,
                        error = %e,
                        "D1 set_qr_url failed on admin upload (non-fatal)"
                    );
                }

                wctx.wait_until(crate::sheets::bg_sync::update_qr_urls(
                    state.clone(),
                    vec![(attendee.row_index, qr_url)],
                    mapping,
                    event.sheet_id.clone(),
                    event.sheet_name.clone(),
                    Some(kv.clone()),
                ));
            }
        } else {
            // Fallback: blocking Sheets write when worker_ctx unavailable (tests).
            let ctx = crate::sheets::write::SheetContext {
                mapping: &mapping,
                state: &state,
                sheet_id: &event.sheet_id,
                sheet_name: &event.sheet_name,
                kv: Some(kv),
            };

            if let Err(e) = crate::sheets::write::write_deposit_verification(
                attendee.row_index,
                "THB",
                &deposit_amount_thb,
                verified,
                &ctx,
            )
            .await
            {
                tracing::warn!(
                    attendee_id = %body.attendee_id,
                    error = %e,
                    "failed to write deposit verification to sheet (non-fatal)"
                );
            }

            if verified && attendee.qr_code_url.as_ref().is_none_or(|u| u.is_empty()) {
                let server_url = &state.config.server.url;
                let qr_url = format!("{server_url}/staff/?scan={}", attendee.api_id);

                if let Some(ref d1) = state.d1
                    && let Err(e) =
                        crate::db::attendees::set_qr_url(d1, &attendee.api_id, &qr_url).await
                {
                    tracing::warn!(
                        attendee_id = %attendee.api_id,
                        error = %e,
                        "D1 set_qr_url failed on admin upload (non-fatal)"
                    );
                }

                if let Err(e) = crate::sheets::write::update_qr_urls(
                    &[(attendee.row_index, qr_url)],
                    &mapping,
                    &state,
                    &event.sheet_id,
                    &event.sheet_name,
                    Some(kv),
                )
                .await
                {
                    tracing::warn!(
                        error = %e,
                        "failed to auto-generate QR for admin-uploaded deposit (non-fatal)"
                    );
                }
            }
        }
    }

    // 14. D1 dual-write for verified deposit (non-fatal, Phase 2a).
    //     Mirrors verify_thb_slip_handler — only fires when auto_verify=true.
    if verified
        && let Some(ref d1) = state.d1
        && let Err(e) = crate::db::attendees::verify_deposit(
            d1,
            &body.attendee_id,
            "verified",
            "THB",
            0, // THB amount tracked in KV, not USDC
            &now,
            &claims.email,
        )
        .await
    {
        tracing::warn!(
            attendee_id = %body.attendee_id,
            error = %e,
            "D1 deposit verify failed (non-fatal)"
        );
    }

    tracing::info!(
        attendee_id = %body.attendee_id,
        event_id = %event.id,
        amount_thb = event.deposit_amount_thb,
        verified,
        admin_email = %claims.email,
        "admin slip upload completed"
    );

    // 15. Audit log — always emit SlipRecordedByAdmin.
    let _ = crate::audit_store::append_event_audit(
        kv,
        &event.id,
        crate::audit_store::create_entry(
            &claims.email,
            crate::audit_store::AuditAction::SlipRecordedByAdmin,
            &body.attendee_id,
            &format!(
                "admin recorded THB slip{} for attendee",
                if verified { " and auto-verified" } else { "" }
            ),
        ),
        state.d1.as_deref(),
    )
    .await;

    let msg = if verified {
        "slip recorded and verified"
    } else {
        "slip recorded, pending verification"
    };

    Ok(ApiOk::new(serde_json::json!({
        "success": true,
        "verified": verified,
        "message": msg,
    })))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// Scope: pure deserialization + default-flag surface of `AdminSlipUploadRequest`.
//
// Full handler integration is not covered here — the worker crate has no
// pattern for mocking AppState's KV/D1/Sheets/R2 surfaces, and stand-in mocks
// would not exercise the production code paths. The handler's correctness
// rests on reusing `slip_upload::validate_slip_url`, `event_store`, and
// `sheets::bg_sync`, all of which have their own coverage. These tests guard
// the contract for the request body that this handler owns.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_auto_verify_is_true() {
        // The default must remain `true` — the entire UX of the admin "Record
        // Slip for Attendee" modal is built around record+verify in one step.
        // Flipping this default silently would change every modal submit into
        // a two-step (record-then-manually-verify) flow.
        assert!(default_auto_verify());
    }

    #[test]
    fn deserializes_with_auto_verify_defaulted_when_omitted() {
        // serde(default = ...) path — field absent from JSON.
        let json = r#"{
            "event_id": "evt_1",
            "attendee_id": "att_1",
            "slip_url": "data:image/png;base64,iVBORw0KGgo=",
            "bank_account": "123-4-56789-0",
            "bank_name": "KBank",
            "account_name": "Somchai"
        }"#;
        let req: AdminSlipUploadRequest = serde_json::from_str(json).unwrap();
        assert!(req.auto_verify, "auto_verify must default to true");
        assert_eq!(req.event_id, "evt_1");
        assert_eq!(req.attendee_id, "att_1");
        assert_eq!(req.bank_account.as_deref(), Some("123-4-56789-0"));
    }

    #[test]
    fn deserializes_with_auto_verify_false_when_explicit() {
        // Opt-out path — admin wants to record but defer verification.
        let json = r#"{
            "event_id": "evt_1",
            "attendee_id": "att_1",
            "slip_url": "https://r2/storage/slips/evt_1/att_1",
            "bank_account": "999",
            "bank_name": "SCB",
            "account_name": "Suda",
            "auto_verify": false
        }"#;
        let req: AdminSlipUploadRequest = serde_json::from_str(json).unwrap();
        assert!(!req.auto_verify, "auto_verify must respect explicit false");
    }

    #[test]
    fn deserializes_with_optional_bank_fields_absent() {
        // Mirrors the attendee endpoint's `Option<String>` with serde default.
        // The handler rejects empty/None bank info at the validation layer,
        // but the *deserialization* must tolerate absent fields without
        // panicking — a client that forgets to send one should get a 400,
        // not a 422 parse error.
        let json = r#"{
            "event_id": "evt_1",
            "attendee_id": "att_1",
            "slip_url": "data:image/jpeg;base64,/9j/4AAQ="
        }"#;
        let req: AdminSlipUploadRequest = serde_json::from_str(json).unwrap();
        assert!(req.bank_account.is_none());
        assert!(req.bank_name.is_none());
        assert!(req.account_name.is_none());
        assert!(req.auto_verify);
    }

    #[test]
    fn deserializes_https_slip_url() {
        // The handler accepts both data URLs and HTTPS URLs (mirrors the
        // attendee endpoint's `validate_slip_url`). Verifying the JSON shape
        // accepts a long R2 path without truncation.
        let https_url = "https://cdn.example.com/slips/evt_1/att_1/large-key.jpg";
        let json = format!(
            r#"{{
                "event_id": "evt_1",
                "attendee_id": "att_1",
                "slip_url": "{https_url}",
                "bank_account": "1",
                "bank_name": "BBL",
                "account_name": "Anon"
            }}"#
        );
        let req: AdminSlipUploadRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req.slip_url, https_url);
    }

    #[test]
    fn rejects_malformed_json() {
        // Defensive: a broken body must surface as a serde error, not a panic.
        // Axum converts this into a 400 via its `Json` extractor — the test
        // confirms the contract.
        let bad = r#"{ not valid json"#;
        let result: Result<AdminSlipUploadRequest, _> = serde_json::from_str(bad);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_missing_required_event_id() {
        // `event_id` is non-optional — deserialization must fail without it.
        let json = r#"{
            "attendee_id": "att_1",
            "slip_url": "data:image/png;base64,aaa",
            "bank_account": "1",
            "bank_name": "BBL",
            "account_name": "Anon"
        }"#;
        let result: Result<AdminSlipUploadRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_missing_required_attendee_id() {
        // `attendee_id` is non-optional — deserialization must fail without it.
        let json = r#"{
            "event_id": "evt_1",
            "slip_url": "data:image/png;base64,aaa",
            "bank_account": "1",
            "bank_name": "BBL",
            "account_name": "Anon"
        }"#;
        let result: Result<AdminSlipUploadRequest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}

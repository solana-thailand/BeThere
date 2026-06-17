//! Sheet → D1 sync handler.
//!
//! Reads attendee data from the Google Sheet for a specific event
//! and upserts all rows (including lifecycle data) into D1.
//!
//! Used to recover from KV data loss — the Sheet is the source of truth
//! for identity + registration + lifecycle state.

use axum::Extension;
use axum::extract::{Path, State};
use serde::Serialize;

use event_checkin_domain::models::attendee::Attendee;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::error::ApiOk;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct SheetSyncResponse {
    pub event_id: String,
    pub total_in_sheet: usize,
    pub synced: usize,
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
    pub errors: usize,
    /// First per-row error message — surfaced for diagnostics because
    /// `tracing-wasm` buffers `warn!` and does not flush inside request
    /// handlers under `wrangler dev --remote`. `None` when there are no errors.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_error: Option<String>,
}

// ---------------------------------------------------------------------------
// POST /api/events/{id}/sync-sheet
// ---------------------------------------------------------------------------

/// Sync all attendees from the Google Sheet into D1 for a specific event.
///
/// Reads the full Sheet (identity + lifecycle columns) and upserts each
/// row into D1. Idempotent — safe to re-run.
///
/// Requires staff auth (admin/organizer).
#[worker::send]
pub async fn sync_sheet_to_d1(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<String>,
) -> Result<ApiOk<SheetSyncResponse>, crate::error::WorkerError> {
    tracing::info!(
        event_id = %event_id,
        staff_email = %claims.email,
        "sheet → D1 sync requested"
    );

    let d1 = state
        .d1
        .as_ref()
        .ok_or_else(|| AppError::Internal("D1 database not configured".into()))?;

    // Resolve event config (KV → D1 fallback)
    let config = crate::event_store::get_event_config_with_fallback(
        state.events_kv.as_ref(),
        state.d1.as_deref(),
        &event_id,
    )
    .await
    .map_err(AppError::Internal)?
    .ok_or_else(|| AppError::NotFound(format!("event '{event_id}' not found")))?;

    // Access check
    if let Err(e) = crate::auth::check_event_access(&claims.email, &state, &config).await {
        return Err(AppError::Forbidden(e).into());
    }

    // Read attendees from Google Sheet (NOT from D1 — we want the Sheet data)
    let kv = state.events_kv.as_ref().or(state.quiz_kv.as_ref());
    let sheet_attendees = crate::sheets::get_attendees_for_event(
        &state,
        &config.sheet_id,
        &config.sheet_name,
        kv,
        &event_id,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to read attendees from sheet");
        AppError::External {
            service: "google_sheets".into(),
            status: 502,
            body: format!("failed to read sheet: {e}"),
        }
    })?;

    let total_in_sheet = sheet_attendees.len();

    if total_in_sheet == 0 {
        return Ok(ApiOk::new(SheetSyncResponse {
            event_id,
            total_in_sheet: 0,
            synced: 0,
            inserted: 0,
            updated: 0,
            skipped: 0,
            errors: 0,
            first_error: None,
        }));
    }

    // Count existing D1 attendees to distinguish inserted vs updated
    let existing_ids: std::collections::HashSet<String> = {
        let existing = crate::db::attendees::get_attendees_by_event(d1, &event_id)
            .await
            .unwrap_or_default();
        existing.iter().map(|a| a.api_id.clone()).collect()
    };

    // Detect claim_tokens that appear more than once within this batch.
    // Legacy / misaligned sheets (e.g. Luma imports where `claim_token` reads
    // a garbage column) can yield repeated values that collide on the
    // `claim_token UNIQUE` constraint and abort the whole row. We null out the
    // colliding tokens below — the live check-in flow mints fresh tokens
    // anyway, and the sync's job is identity + lifecycle, not the historical
    // (often meaningless) claim token from the sheet.
    let dup_tokens: std::collections::HashSet<String> = {
        let mut counts: std::collections::HashMap<&str, usize> =
            std::collections::HashMap::new();
        for a in &sheet_attendees {
            if let Some(t) = a.claim_token.as_deref().filter(|t| !t.is_empty()) {
                *counts.entry(t).or_default() += 1;
            }
        }
        counts
            .into_iter()
            .filter_map(|(t, n)| if n > 1 { Some(t.to_string()) } else { None })
            .collect()
    };

    let mut synced = 0usize;
    let mut inserted = 0usize;
    let mut updated = 0usize;
    let mut skipped = 0usize;
    let mut errors = 0usize;
    let mut first_error: Option<String> = None;

    for attendee in &sheet_attendees {
        let email = attendee.email.trim();
        if email.is_empty() {
            skipped += 1;
            continue;
        }

        let is_new = !existing_ids.contains(&attendee.api_id);

        // If this attendee's claim_token collides within the batch, clone the
        // row with claim_token = None so the UNIQUE constraint doesn't abort
        // the import (see `dup_tokens` build above for rationale).
        let needs_null_token = attendee
            .claim_token
            .as_deref()
            .filter(|t| !t.is_empty())
            .is_some_and(|t| dup_tokens.contains(t));

        let owned_normalised: Attendee;
        let upsert_ref: &Attendee = if needs_null_token {
            owned_normalised = {
                let mut c = attendee.clone();
                c.claim_token = None;
                c
            };
            &owned_normalised
        } else {
            attendee
        };

        match sync_one_attendee(d1, &event_id, upsert_ref).await {
            Ok(()) => {
                if is_new {
                    inserted += 1;
                } else {
                    updated += 1;
                }
                synced += 1;
            }
            Err(e) => {
                tracing::warn!(
                    attendee_id = %attendee.api_id,
                    email = %email,
                    error = %e,
                    "failed to sync attendee"
                );
                // Capture the first error verbatim so it reaches the caller —
                // tracing-wasm swallows `warn!` under `wrangler dev --remote`.
                if first_error.is_none() {
                    first_error = Some(format!(
                        "api_id={api_id} email={email}: {e}",
                        api_id = attendee.api_id,
                    ));
                }
                errors += 1;
            }
        }
    }

    tracing::info!(
        event_id = %event_id,
        total_in_sheet,
        synced,
        inserted,
        updated,
        skipped,
        errors,
        "sheet → D1 sync completed"
    );

    Ok(ApiOk::new(SheetSyncResponse {
        event_id,
        total_in_sheet,
        synced,
        inserted,
        updated,
        skipped,
        errors,
        first_error,
    }))
}

// ---------------------------------------------------------------------------
// Internal: map Attendee → D1 upsert
// ---------------------------------------------------------------------------

/// Map a Sheet Attendee to the full D1 upsert, translating Sheet column
/// values to D1 column values.
async fn sync_one_attendee(
    d1: &worker::D1Database,
    event_id: &str,
    a: &Attendee,
) -> Result<(), String> {
    // Normalize approval_status from sheet format (e.g. "Approved") to D1 format (e.g. "approved")
    let approval_status = normalize_approval_status(&a.approval_status.to_string());
    let participation_type = normalize_participation_type(&a.participation_type);
    let deposit_status = derive_deposit_status(a);

    crate::db::attendees::upsert_attendee_full(
        d1,
        &a.api_id,
        event_id,
        a.email.trim(),
        &a.name,
        &approval_status,
        &participation_type,
        req_str(&a.contact_channel),
        req_str(&a.contact_handle),
        opt_str(&a.checked_in_at),
        opt_str(&a.checked_in_by),
        opt_str(&a.claim_token),
        opt_str(&a.claimed_at),
        opt_str(&a.qr_code_url),
        &deposit_status,
        opt_str(&a.deposit_tx_signature),
        // refund_tx_hash — not in sheet columns
        None,
        opt_str(&a.refund_link),
        opt_str(&a.bank_name),
        opt_str(&a.bank_account),
        opt_str(&a.account_name),
        Some(a.row_index as i32),
    )
    .await
}

/// Normalize approval status from Sheet display format to D1 snake_case.
fn normalize_approval_status(s: &str) -> String {
    match s.to_lowercase().as_str() {
        "approved" => "approved".to_string(),
        "pending_approval" | "pendingapproval" | "pending approval" => {
            "pending_approval".to_string()
        }
        "invited" => "invited".to_string(),
        "checked_in" | "checked in" => "checked_in".to_string(),
        other => other.to_lowercase(),
    }
}

/// Normalize participation type (sheet may have "In-Person", D1 stores "in_person").
fn normalize_participation_type(s: &str) -> String {
    match s.to_lowercase().as_str() {
        s if s.contains("in-person") || s.contains("in person") || s.contains("physical") => {
            "in_person".to_string()
        }
        s if s.contains("online") || s.contains("virtual") => "online".to_string(),
        _ => s.to_lowercase(),
    }
}

/// Derive deposit_status from the sheet's deposit columns.
fn derive_deposit_status(a: &Attendee) -> String {
    let verified = a
        .deposit_verified
        .as_deref()
        .is_some_and(|v| v.eq_ignore_ascii_case("yes"));
    let agreed = a
        .deposit_agreed
        .as_deref()
        .is_some_and(|v| v.eq_ignore_ascii_case("yes"));
    let has_tx = a
        .deposit_tx_signature
        .as_deref()
        .is_some_and(|v| !v.trim().is_empty());

    if verified && has_tx {
        "verified".to_string()
    } else if has_tx {
        "pending".to_string()
    } else if agreed {
        "agreed".to_string()
    } else {
        "none".to_string()
    }
}

/// Convert Option<String> to Option<&str> for binding.
/// Empty strings become None so COALESCE in D1 preserves existing values.
fn opt_str(opt: &Option<String>) -> Option<&str> {
    opt.as_deref().filter(|s| !s.is_empty())
}

/// Convert Option<String> to &str for required (non-nullable) D1 columns.
fn req_str(opt: &Option<String>) -> &str {
    opt.as_deref().unwrap_or("")
}

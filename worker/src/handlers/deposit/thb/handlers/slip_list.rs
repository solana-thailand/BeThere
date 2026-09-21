use axum::{
    Extension,
    extract::{Query, State},
};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::deposit::{PendingSlipResponse, RefundQueueResponse, ThbDeposit};
use event_checkin_domain::models::error::AppError;
use serde::{Deserialize, Serialize};

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::handlers::ext::EventIdQuery;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// GET /api/deposit/thb/pending (admin)
// ---------------------------------------------------------------------------

/// List all unverified THB deposits for admin review.
#[worker::send]
pub async fn pending_thb_slips_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<PendingSlipResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;
    let d1 = state.d1.as_deref();

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, query.event_id.as_deref())
            .await?;

    let all_deposits = event_store::list_thb_deposits(kv, &event.id, d1)
        .await
        .map_err(AppError::Internal)?;

    // Which slip images appear on more than one attendee. Computed over EVERY
    // deposit for the event, not just the pending ones: the case the organizer
    // most needs to see is a new slip that matches one they already approved,
    // and filtering first would hide exactly that.
    //
    // Keyed on (hash -> distinct attendee ids) rather than a raw count, so an
    // attendee re-uploading their own slip after a rejection never shows up as
    // a duplicate of themselves.
    let duplicate_slip_hashes = duplicate_slip_hashes(&all_deposits);

    let mut pending: Vec<ThbDeposit> = all_deposits
        .into_iter()
        .filter(|d| !d.verified && d.slip_url.is_some())
        .collect();

    // Migrate any inline base64 slip URLs to R2 (keeps response payload small)
    super::migrate_data_urls(&state, kv, d1, &event.id, &mut pending).await;

    // Enrich with attendee names from Google Sheets
    let attendee_names =
        super::resolve_attendee_names(&state, &event.sheet_id, &event.sheet_name, &pending).await;
    let slips: Vec<ThbDeposit> = pending
        .into_iter()
        .map(|mut d| {
            d.attendee_name = attendee_names.get(&d.attendee_id).cloned();
            d
        })
        .collect();

    Ok(ApiOk::new(PendingSlipResponse {
        slips,
        duplicate_slip_hashes,
    }))
}

/// Slip fingerprints carried by two or more *distinct* attendees.
///
/// Rows with no fingerprint are skipped entirely: every deposit uploaded before
/// migration 0046 has none, and treating absent-as-equal would report the whole
/// backlog as one enormous collision.
fn duplicate_slip_hashes(deposits: &[ThbDeposit]) -> Vec<String> {
    let mut by_hash: std::collections::BTreeMap<&str, std::collections::BTreeSet<&str>> =
        std::collections::BTreeMap::new();
    for d in deposits {
        if let Some(hash) = d.slip_blake3.as_deref().filter(|h| !h.is_empty()) {
            by_hash
                .entry(hash)
                .or_default()
                .insert(d.attendee_id.as_str());
        }
    }
    by_hash
        .into_iter()
        .filter(|(_, attendees)| attendees.len() > 1)
        .map(|(hash, _)| hash.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// GET /api/deposit/credit-used (admin) — who got in via credit + source summary
// ---------------------------------------------------------------------------

/// Count + ฿ total of an event's deposits, classified by [`DepositSource`].
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DepositSourceSummary {
    pub cash_count: u32,
    pub cash_thb: u64,
    pub credit_count: u32,
    pub credit_thb: u64,
    pub comp_count: u32,
}

/// Response for GET /api/deposit/credit-used.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CreditUsedResponse {
    pub summary: DepositSourceSummary,
    /// The credit-covered deposits (source == Credit), name-enriched.
    pub credit_used: Vec<ThbDeposit>,
}

/// List the attendees who got in by SPENDING rolling credit, plus a
/// Cash/Credit/Comp summary so the money reconciles at a glance. Keys off the
/// single `DepositSource` classification (not sentinel sniffing).
#[worker::send]
pub async fn credit_used_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<CreditUsedResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;
    let d1 = state.d1.as_deref();

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, query.event_id.as_deref())
            .await?;

    let all_deposits = event_store::list_thb_deposits(kv, &event.id, d1)
        .await
        .map_err(AppError::Internal)?;

    use event_checkin_domain::models::deposit::DepositSource;
    let mut summary = DepositSourceSummary::default();
    let mut credit_used: Vec<ThbDeposit> = Vec::new();
    for d in &all_deposits {
        match d.source() {
            DepositSource::Cash => {
                summary.cash_count += 1;
                summary.cash_thb += d.amount_thb;
            }
            DepositSource::Credit => {
                summary.credit_count += 1;
                summary.credit_thb += d.amount_thb;
                credit_used.push(d.clone());
            }
            DepositSource::Comp => summary.comp_count += 1,
        }
    }

    let names =
        super::resolve_attendee_names(&state, &event.sheet_id, &event.sheet_name, &credit_used)
            .await;
    for d in &mut credit_used {
        d.attendee_name = names.get(&d.attendee_id).cloned();
    }

    Ok(ApiOk::new(CreditUsedResponse {
        summary,
        credit_used,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/refund/queue (admin)
// ---------------------------------------------------------------------------

/// List THB deposits that need refund (verified + checked-in + not yet refunded).
#[worker::send]
pub async fn refund_queue_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<RefundQueueResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;
    let d1 = state.d1.as_deref();

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, query.event_id.as_deref())
            .await?;

    let all_deposits = event_store::list_thb_deposits(kv, &event.id, d1)
        .await
        .map_err(AppError::Internal)?;

    // A deposit held as rolling credit is also terminal-settled (organizer
    // retains funds as liability); exclude it from the refund queue so the
    // admin does not double-process a deposit the attendee already converted.
    let mut pending: Vec<ThbDeposit> = all_deposits
        .into_iter()
        // Exclude non-cash deposits (rolling-credit applications, staff comps, ฿0):
        // they were never funded with cash, so they must not enter the refund
        // queue — refunding one pays out money that was never deposited.
        .filter(|d| d.verified && !d.refunded && !d.held_as_credit && !d.is_non_cash())
        .collect();

    // Migrate any inline base64 slip/refund URLs to R2 (keeps response payload small)
    super::migrate_data_urls(&state, kv, d1, &event.id, &mut pending).await;

    // Enrich with attendee names from Google Sheets
    let attendee_names =
        super::resolve_attendee_names(&state, &event.sheet_id, &event.sheet_name, &pending).await;
    let enriched: Vec<ThbDeposit> = pending
        .into_iter()
        .map(|mut d| {
            d.attendee_name = attendee_names.get(&d.attendee_id).cloned();
            d
        })
        .collect();

    Ok(ApiOk::new(RefundQueueResponse { pending: enriched }))
}

// ---------------------------------------------------------------------------
// GET /api/refund/refunded?event_id=xxx (admin)
// ---------------------------------------------------------------------------

/// Response for the refunded list endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundedListResponse {
    pub refunded: Vec<ThbDeposit>,
}

/// List all refunded THB deposits for an event.
#[worker::send]
pub async fn refunded_list_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<RefundedListResponse>, WorkerError> {
    let kv = state
        .events_kv
        .as_ref()
        .ok_or_else(|| AppError::Internal("EVENTS KV not configured".to_string()))?;
    let d1 = state.d1.as_deref();

    let event =
        crate::handlers::ext::resolve_event_with_access(&state, &claims, query.event_id.as_deref())
            .await?;

    let all_deposits = event_store::list_thb_deposits(kv, &event.id, d1)
        .await
        .map_err(AppError::Internal)?;

    let mut refunded: Vec<ThbDeposit> = all_deposits.into_iter().filter(|d| d.refunded).collect();

    // Migrate any inline base64 slip/refund URLs to R2 (keeps response payload small)
    super::migrate_data_urls(&state, kv, d1, &event.id, &mut refunded).await;

    // Enrich with attendee names from Google Sheets
    let attendee_names =
        super::resolve_attendee_names(&state, &event.sheet_id, &event.sheet_name, &refunded).await;
    let enriched: Vec<ThbDeposit> = refunded
        .into_iter()
        .map(|mut d| {
            d.attendee_name = attendee_names.get(&d.attendee_id).cloned();
            d
        })
        .collect();

    Ok(ApiOk::new(RefundedListResponse { refunded: enriched }))
}

#[cfg(test)]
mod duplicate_slip_tests {
    use super::*;

    fn deposit(attendee_id: &str, hash: Option<&str>) -> ThbDeposit {
        ThbDeposit {
            attendee_id: attendee_id.to_string(),
            event_id: "evt".to_string(),
            amount_thb: 500,
            slip_url: Some("/api/storage/slips/evt/x".to_string()),
            verified: false,
            verified_by: None,
            verified_at: None,
            uploaded_at: "2026-09-22T00:00:00Z".to_string(),
            refunded: false,
            refunded_at: None,
            held_as_credit: false,
            held_as_credit_at: None,
            attendee_name: None,
            bank_account: None,
            bank_name: None,
            account_name: None,
            refund_proof_url: None,
            slip_blake3: hash.map(str::to_string),
            deposit_source: None,
        }
    }

    /// The case the organizer currently solves by recognising faces.
    #[test]
    fn two_attendees_sharing_one_image_are_reported() {
        let rows = [
            deposit("alice", Some("aaa")),
            deposit("bob", Some("aaa")),
            deposit("carol", Some("bbb")),
        ];
        assert_eq!(duplicate_slip_hashes(&rows), vec!["aaa".to_string()]);
    }

    /// An attendee whose slip was rejected re-uploads the same image. One
    /// person, two rows, one hash — not a duplicate, and flagging it would
    /// train the organizer to ignore the flag.
    #[test]
    fn one_attendee_with_two_rows_is_not_a_duplicate() {
        let rows = [deposit("alice", Some("aaa")), deposit("alice", Some("aaa"))];
        assert!(duplicate_slip_hashes(&rows).is_empty());
    }

    /// Every deposit uploaded before migration 0046 has no hash. If absent were
    /// treated as equal, the first admin page load after deploying this would
    /// report the entire backlog as one giant collision — the flag would be
    /// useless on the day it shipped.
    #[test]
    fn rows_without_a_hash_are_never_duplicates_of_each_other() {
        let rows = [
            deposit("alice", None),
            deposit("bob", None),
            deposit("carol", Some("")),
            deposit("dave", Some("")),
        ];
        assert!(
            duplicate_slip_hashes(&rows).is_empty(),
            "absent and empty fingerprints must not match each other"
        );
    }

    /// A mixed table: the pre-migration rows must not drag the real collision
    /// out of the result, nor add themselves to it.
    #[test]
    fn unhashed_rows_do_not_mask_a_real_collision() {
        let rows = [
            deposit("alice", None),
            deposit("bob", Some("aaa")),
            deposit("carol", Some("aaa")),
            deposit("dave", None),
        ];
        assert_eq!(duplicate_slip_hashes(&rows), vec!["aaa".to_string()]);
    }

    #[test]
    fn an_empty_event_reports_nothing() {
        assert!(duplicate_slip_hashes(&[]).is_empty());
    }
}

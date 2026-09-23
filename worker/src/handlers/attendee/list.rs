//! `GET /api/attendees` — list attendees with pagination + statistics.

use axum::{
    Extension,
    extract::{Query, State},
};
use serde_json::json;

use crate::error::ApiOk;
use event_checkin_domain::models::api::{AttendeeListItem, RecentCheckIn, StatsResponse};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::handlers::ext::{AttendeesQuery, resolve_event_with_access, resolve_kv};
use crate::sheets;
use crate::state::AppState;

// Walk-in attendees are now stored in D1 directly, so no KV→Attendee conversion is needed.

/// GET /api/attendees
/// List attendees with cursor-based pagination and statistics.
///
/// Stats are computed over ALL attendees regardless of pagination.
/// Attendees are sorted by `row_index` ascending for deterministic pagination.
/// Use `cursor` (row_index of last item) and `limit` (page size) for pagination.
#[worker::send]
pub async fn list_attendees(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<AttendeesQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(
        staff_fingerprint = %state.log_fingerprint(&claims.email),
        "listing attendees",
    );

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;
    tracing::info!(event_id = %event.id, "STEP 1: event resolved");

    let kv = resolve_kv(&state);
    tracing::info!(
        has_kv = kv.is_some(),
        has_d1 = state.d1.is_some(),
        "STEP 2: bindings"
    );

    // 1. Fetch attendees. NOTE the direction: `get_attendees_for_event` is
    //    D1-FIRST and falls back to Sheets only when D1 returns nothing or
    //    errors. This comment used to say the opposite ("D1 fallback when
    //    Sheets is unavailable"), which mattered — it is why a wrong
    //    `ticket_name` coming out of D1 looked like it could only affect an
    //    outage path, when in fact it is what every organizer sees on this
    //    screen every day (.issues/136). The `match` below is the fallback
    //    for when BOTH are unavailable.
    tracing::info!("STEP 3: before get_attendees_for_event");
    let attendees = match sheets::get_attendees_for_event(
        &state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
        &event.id,
    )
    .await
    {
        Ok(a) => a,
        Err(sheets_err) => {
            tracing::warn!(error = %sheets_err, "Sheets fetch failed, trying D1 fallback");
            match &state.d1 {
                Some(db) => crate::db::attendees::get_attendees_by_event(db, &event.id)
                    .await
                    .map_err(|d1_err| {
                        tracing::error!(error = %d1_err, "D1 fallback also failed");
                        AppError::Internal(format!("Sheets: {sheets_err} | D1: {d1_err}"))
                    })?,
                None => {
                    return Err(AppError::Internal(format!(
                        "failed to fetch attendees: {sheets_err}"
                    ))
                    .into());
                }
            }
        }
    };
    tracing::info!(count = attendees.len(), "STEP 4: attendees fetched");

    // Walk-in attendees are now stored directly in D1 alongside pre-registered
    // attendees (participation_type='walkin'), so no separate KV merge is needed.

    // Compute statistics over ALL attendees (not paginated)
    let total_approved: usize = attendees.iter().filter(|a| a.is_approved()).count();

    let total_checked_in: usize = attendees.iter().filter(|a| a.is_checked_in()).count();

    let total_remaining: usize = total_approved.saturating_sub(total_checked_in);

    let check_in_percentage: f64 = if total_approved > 0 {
        (total_checked_in as f64 / total_approved as f64) * 100.0
    } else {
        0.0
    };

    let recent_check_ins: Vec<RecentCheckIn> = attendees
        .iter()
        .filter(|a| a.is_checked_in())
        .filter_map(|a| {
            a.checked_in_at.as_ref().map(|ts| RecentCheckIn {
                api_id: a.api_id.clone(),
                name: a.display_name().to_string(),
                checked_in_at: ts.clone(),
                checked_in_by: a.checked_in_by.clone(),
            })
        })
        .collect();

    let stats = StatsResponse {
        total_approved,
        total_checked_in,
        total_remaining,
        check_in_percentage: (check_in_percentage * 100.0).round() / 100.0,
        recent_check_ins,
    };

    // Cursor-based pagination: sort approved attendees by row_index,
    // filter by cursor, then take up to `page_limit`.
    let page_limit = query.limit.unwrap_or(200).min(200);

    let mut approved: Vec<_> = attendees.iter().filter(|a| a.is_approved()).collect();
    approved.sort_by_key(|a| a.row_index);

    let filtered: Vec<_> = match query.cursor {
        Some(cursor) => approved
            .into_iter()
            .filter(|a| a.row_index > cursor)
            .collect(),
        None => approved,
    };

    let has_more = filtered.len() > page_limit;
    let page: Vec<_> = filtered.into_iter().take(page_limit).collect();

    let next_cursor = if has_more {
        page.last().map(|a| a.row_index)
    } else {
        None
    };

    let mut attendee_responses: Vec<AttendeeListItem> = page
        .iter()
        .map(|a| AttendeeListItem::from_attendee(a))
        .collect();

    // Annotate each row from the credit ledger (two batch queries, best-effort):
    //  - credit_thb: remaining rolling credit (powers the Apply-Credit action + badge)
    //  - used_credit: whether the attendee got in by spending credit AT this event
    //    (powers the "Credit ✓" badge / who-used-credit view)
    if let Some(db) = state.d1.as_deref() {
        if let Ok(balances) =
            crate::db::credit_ledger::thb_balances_by_email(db, &event.organization_id).await
        {
            for item in attendee_responses.iter_mut() {
                if let Some(&bal) = balances.get(&item.email.to_lowercase()) {
                    item.credit_thb = bal.max(0);
                }
            }
        }
        if let Ok(applied) = crate::db::credit_ledger::emails_applied_credit(db, &event.id).await {
            for item in attendee_responses.iter_mut() {
                if applied.contains(&item.email.to_lowercase()) {
                    item.used_credit = true;
                }
            }
        }

        //  - thb_source / thb_verified / thb_refunded: how the THB deposit was
        //    settled. Without this the roster is blind to the entire THB flow —
        //    it reads `attendees.deposit_status` and the USDC amount columns,
        //    and nothing writes either (`save_deposit_status_to_d1` is dead
        //    code). A staff comp and a credit-covered registration both looked
        //    like an unpaid attendee and the door screen said "Deposit
        //    pending". See `.issues/137`.
        //
        //    Best-effort like the two above: a failure here must degrade the
        //    badge, never the roster. An organizer at the door needs the list of
        //    names far more than they need the badge on it.
        if let Ok(settlements) =
            crate::db::thb_deposits::settlement_by_attendee(db, &event.id).await
        {
            for item in attendee_responses.iter_mut() {
                if let Some(s) = settlements.get(&item.api_id) {
                    item.thb_source = Some(s.source.as_str().to_string());
                    item.thb_verified = s.verified;
                    item.thb_refunded = s.refunded;
                }
            }
        }
    }

    let data = json!({
        "attendees": attendee_responses,
        "stats": stats,
        "next_cursor": next_cursor,
        "has_more": has_more,
    });
    Ok(ApiOk::new(data))
}

//! Attendee reads: D1 first, Google Sheets fallback.

use std::collections::HashMap;

use event_checkin_domain::models::attendee::{Attendee, AttendeeRow};
use event_checkin_domain::models::event::EventConfig;

use worker::KvStore;

use crate::http::{ValueRange, fetch_sheet_range};
use crate::state::AppState;

use super::a1;
use super::columns::get_column_mapping;
use super::token::get_cached_access_token;

// ---------------------------------------------------------------------------
// Sheet range fetch with automatic fallback for narrow sheets
// ---------------------------------------------------------------------------

/// Fetch a sheet range, retrying with progressively smaller column ranges
/// if the initial range exceeds the actual sheet width.
///
/// Google Sheets API returns `Unable to parse range` when the requested
/// column range (e.g. `A2:AF`) exceeds the sheet's actual column count.
/// This helper retries with smaller fallback ranges: Z (26), Q (17).
async fn fetch_sheet_range_with_retry(
    sheet_id: &str,
    sheet_name: &str,
    initial_range: &str,
    access_token: &str,
) -> Result<ValueRange, String> {
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
        urlencoding::encode(initial_range)
    );

    match fetch_sheet_range(&url, access_token).await {
        Ok(vr) => Ok(vr),
        Err(e) if e.contains("Unable to parse range") => {
            tracing::warn!(
                error = %e,
                range = %initial_range,
                "sheet range too wide, retrying with A2:Z"
            );
            let sheet_ref = a1::sheet_ref(sheet_name);
            // Fallback 1: A2:Z (26 columns)
            let fallback_range = format!("{sheet_ref}!A2:Z");
            let fb_url = format!(
                "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
                urlencoding::encode(&fallback_range)
            );
            match fetch_sheet_range(&fb_url, access_token).await {
                Ok(vr) => Ok(vr),
                Err(e2) if e2.contains("Unable to parse range") => {
                    tracing::warn!(
                        error = %e2,
                        "A2:Z also failed, retrying with A2:Q"
                    );
                    // Fallback 2: A2:Q (17 columns — covers through checked_in_by)
                    let fb2_range = format!("{sheet_ref}!A2:Q");
                    let fb2_url = format!(
                        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
                        urlencoding::encode(&fb2_range)
                    );
                    fetch_sheet_range(&fb2_url, access_token).await
                }
                Err(e2) => Err(e2),
            }
        }
        Err(e) => Err(e),
    }
}

// ---------------------------------------------------------------------------
// Attendee queries
// ---------------------------------------------------------------------------

/// Fetch all attendees from the Google Sheet.
/// Returns a list of typed Attendee structs parsed from sheet rows.
///
/// Phase 2d: D1-first, Sheets fallback. No KV cache.
pub async fn get_attendees(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<Vec<Attendee>, String> {
    get_attendees_inner(state, sheet_id, sheet_name, kv, None).await
}

/// Fetch all attendees for an event, trying D1 first when available.
///
/// Phase 2b: Queries D1 directly by `event_id`, avoiding the Google Sheets API
/// entirely on D1 hit. Falls back to Sheets on D1 miss/error.
pub async fn get_attendees_for_event(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
    event_id: &str,
) -> Result<Vec<Attendee>, String> {
    get_attendees_inner(state, sheet_id, sheet_name, kv, Some(event_id)).await
}

/// Inner implementation shared between `get_attendees` and `get_attendees_for_event`.
///
/// When `event_id` is provided and D1 is configured, tries D1 first.
/// Otherwise (or on D1 miss/error), falls through to Google Sheets.
///
/// Phase 2d: KV attendee cache removed — D1 is the primary store.
async fn get_attendees_inner(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
    event_id: Option<&str>,
) -> Result<Vec<Attendee>, String> {
    // Phase 2b: D1-first when event_id is available
    if let (Some(d1), Some(eid)) = (&state.d1, event_id) {
        tracing::info!(event_id = %eid, "D1 path: querying attendees");
        match crate::db::attendees::get_attendees_by_event(d1, eid).await {
            Ok(attendees) if !attendees.is_empty() => {
                tracing::info!(
                    count = attendees.len(),
                    event_id = %eid,
                    "D1 hit: attendees for event"
                );
                return Ok(attendees);
            }
            Ok(_) => {
                tracing::debug!(event_id = %eid, "D1 empty: no attendees for event, falling back to Sheets");
            }
            Err(e) => {
                tracing::warn!(event_id = %eid, error = %e, "D1 error: attendees for event, falling back to Sheets");
            }
        }
    }

    // Sheets fallback (no KV cache — Phase 2d)
    let access_token = get_cached_access_token(state, kv).await?;

    // Resolve column mapping from headers
    let mapping = get_column_mapping(state, sheet_id, sheet_name, kv).await?;

    let last_col = mapping.last_column_letter();
    let sheet_ref = a1::sheet_ref(sheet_name);
    let range = format!("{sheet_ref}!A2:{last_col}");

    let value_range: ValueRange =
        fetch_sheet_range_with_retry(sheet_id, sheet_name, &range, &access_token).await?;

    let attendees: Vec<Attendee> = value_range
        .values
        .iter()
        .enumerate()
        .filter(|(_, row)| !row.is_empty())
        .filter(|(_, row)| row.first().is_some_and(|v| !v.trim().is_empty()))
        .filter_map(|(idx, _)| {
            // row_index is 1-based in the sheet, +2 because row 1 is header and idx is 0-based
            let row_index = idx + 2;
            AttendeeRow::from_sheet_values(&value_range.values, row_index, &mapping)
        })
        .map(|row| row.to_attendee())
        .collect();

    tracing::info!(
        count = attendees.len(),
        "fetched attendees from google sheets"
    );

    Ok(attendees)
}

// ---------------------------------------------------------------------------
// HashMap helpers for O(1) lookups
// ---------------------------------------------------------------------------

/// Build a HashMap of attendees keyed by `api_id`.
/// Internally calls `get_attendees()`.
pub async fn get_attendees_map(
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<HashMap<String, Attendee>, String> {
    let attendees: Vec<Attendee> = get_attendees(state, sheet_id, sheet_name, kv).await?;
    Ok(attendees
        .into_iter()
        .map(|a| (a.api_id.clone(), a))
        .collect())
}

/// Get a single attendee of `event` by their api_id.
///
/// Phase 2b: tries D1 first (O(1) by primary key), falls back to Sheets on miss.
/// Both halves are scoped to `event`: `attendees.id` is global, and callers
/// authorize the event, not the id (Issue 153).
pub async fn get_attendee_by_id(
    api_id: &str,
    state: &AppState,
    event: &EventConfig,
    kv: Option<&KvStore>,
) -> Result<Option<Attendee>, String> {
    if let Some(attendee) = get_attendee_by_id_from_d1(api_id, &event.id, state).await {
        return Ok(Some(attendee));
    }
    get_attendee_by_id_from_sheets(api_id, state, &event.sheet_id, &event.sheet_name, kv).await
}

/// The D1 half of [`get_attendee_by_id`]: `None` on a miss (including an
/// attendee of another event), a D1 error or no D1 binding. Public callers use
/// the halves separately so they can gate the Sheets read (plan 028 W7).
pub async fn get_attendee_by_id_from_d1(
    api_id: &str,
    event_id: &str,
    state: &AppState,
) -> Option<Attendee> {
    let d1 = state.d1.as_ref()?;
    match crate::db::attendees::get_attendee_by_id(d1, event_id, api_id).await {
        Ok(Some(attendee)) => {
            tracing::debug!(attendee_id = %api_id, "D1 hit: attendee by id");
            Some(attendee)
        }
        Ok(None) => {
            tracing::debug!(attendee_id = %api_id, "D1 miss: attendee by id, falling back to Sheets");
            None
        }
        Err(e) => {
            tracing::warn!(attendee_id = %api_id, error = %e, "D1 error: attendee by id, falling back to Sheets");
            None
        }
    }
}

/// The Sheets half of [`get_attendee_by_id`]: reads the whole sheet, so every
/// call costs a Google API request.
pub async fn get_attendee_by_id_from_sheets(
    api_id: &str,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<Option<Attendee>, String> {
    let map = get_attendees_map(state, sheet_id, sheet_name, kv).await?;
    Ok(map.get(api_id).cloned())
}

/// Find an attendee by their claim token.
///
/// Phase 2d: tries D1 first (indexed on claim_token), falls back to Sheets on miss.
/// No KV cache — claim map cache removed.
pub async fn get_attendee_by_claim_token(
    claim_token: &str,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<Option<Attendee>, String> {
    // D1-first: try by claim_token index
    if let Some(ref d1) = state.d1 {
        match crate::db::attendees::get_attendee_by_claim_token(
            d1,
            claim_token,
            state.claim_token_policy(),
        )
        .await
        {
            Ok(Some(attendee)) => {
                tracing::debug!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(claim_token), "D1 hit: attendee by claim_token");
                return Ok(Some(attendee));
            }
            Ok(None) => {
                tracing::debug!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(claim_token), "D1 miss: attendee by claim_token, falling back to Sheets");
            }
            Err(e) => {
                tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(claim_token), error = %e, "D1 error: attendee by claim_token, falling back to Sheets");
            }
        }
    }

    get_attendee_by_claim_token_from_sheets(claim_token, state, sheet_id, sheet_name, kv).await
}

/// The Sheets half of [`get_attendee_by_claim_token`], for callers that have
/// already read D1 by claim token (plan 028 W6).
pub(crate) async fn get_attendee_by_claim_token_from_sheets(
    claim_token: &str,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
) -> Result<Option<Attendee>, String> {
    // Sheets fallback (no KV cache — Phase 2d). The replay window applies here
    // too: D1 is primary, but a D1 outage must not quietly restore an unbounded
    // token lifetime (Issue 071).
    let attendees = get_attendees(state, sheet_id, sheet_name, kv).await?;
    let policy = state.claim_token_policy();
    let attendee = attendees
        .into_iter()
        .find(|a| a.claim_token.as_deref() == Some(claim_token))
        .filter(|a| !policy.is_expired(a.checked_in_at.as_deref()));
    Ok(attendee)
}

/// Look up an attendee by claim token and return claim counts.
///
/// Phase 2b: tries D1 first (single query by event_id), falls back to Sheets on miss.
/// Returns `(attendee, total_checked_in, total_claimed)`.
pub async fn get_attendee_with_claim_counts(
    claim_token: &str,
    state: &AppState,
    sheet_id: &str,
    sheet_name: &str,
    kv: Option<&KvStore>,
    event_id: Option<&str>,
) -> Result<(Option<Attendee>, usize, usize), String> {
    // D1-first: single query by event_id
    if let (Some(d1), Some(eid)) = (&state.d1, event_id) {
        match crate::db::attendees::get_attendee_with_claim_counts(
            d1,
            claim_token,
            eid,
            state.claim_token_policy(),
        )
        .await
        {
            Ok((Some(attendee), checked_in, claimed)) => {
                tracing::debug!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(claim_token), "D1 hit: attendee with claim counts");
                return Ok((Some(attendee), checked_in, claimed));
            }
            Ok((None, _checked_in, _claimed)) => {
                tracing::debug!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(claim_token), "D1 miss: attendee with claim counts, falling back to Sheets");
                // D1 returned counts but no attendee — still use counts from Sheets fallback
            }
            Err(e) => {
                tracing::warn!(claim_token_fingerprint = %crate::crypto::claim_token_fingerprint(claim_token), error = %e, "D1 error: attendee with claim counts, falling back to Sheets");
            }
        }
    }

    // Sheets fallback
    let attendees: Vec<Attendee> = get_attendees(state, sheet_id, sheet_name, kv).await?;
    let total_checked_in = attendees
        .iter()
        .filter(|a| a.checked_in_at.is_some())
        .count();
    let total_claimed = attendees.iter().filter(|a| a.claimed_at.is_some()).count();

    // O(1) lookup by claim token
    let claim_map: HashMap<String, Attendee> = attendees
        .into_iter()
        .filter_map(|a| {
            let token = a.claim_token.clone()?;
            Some((token, a))
        })
        .collect();
    // The replay window applies to the Sheets fallback too. An expired token
    // makes the D1 branch above return `None`, which falls through to here — so
    // without this filter the fallback would hand back the very attendee the
    // window just withheld (Issue 071).
    let policy = state.claim_token_policy();
    let attendee = claim_map
        .get(claim_token)
        .cloned()
        .filter(|a| !policy.is_expired(a.checked_in_at.as_deref()));

    Ok((attendee, total_checked_in, total_claimed))
}

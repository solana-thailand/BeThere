//! Contacts API handlers — master contacts list management.
//!
//! Provides endpoints for listing, syncing, and querying the deduplicated
//! master contacts sheet that tracks all attendees across all events.
//! Also manages the Events tab (event registry in the same sheet).
//!
//! Plan 008 §3.5 adds `GET /api/contacts/{email}/history`, which reads from
//! the `attendees` table (source of truth) rather than the deprecated
//! `contacts.events_joined` CSV column.

use axum::Extension;
use axum::extract::{Path, Query, State};
use serde::Serialize;

use crate::db::contacts::{AudienceRow, ContactEventRow, audience_aggregate, list_contact_events};
use crate::error::{ApiOk, WorkerError};
use crate::sheets;
use crate::sheets::contacts::{self, ContactUpsert};
use crate::sheets::events_tab;
use crate::state::AppState;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

// ---------------------------------------------------------------------------
// GET /api/contacts — list all deduplicated contacts
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct ContactsListResponse {
    pub contacts: Vec<contacts::Contact>,
    pub total: usize,
}

#[worker::send]
pub async fn list_contacts_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<ContactsListResponse>, WorkerError> {
    // Cross-org platform view — super-admin only (was ungated: any staff could
    // dump every org's contacts). See admin security review S1.
    crate::auth::require_super_admin(&claims.email, &state, "view all contacts").await?;
    let config = &state.config.sheets;
    if config.contacts_sheet_id.is_empty() {
        return Ok(ApiOk::new(ContactsListResponse {
            contacts: vec![],
            total: 0,
        }));
    }

    let kv = state.events_kv.as_ref();
    let contacts = contacts::list_contacts(
        &state,
        &config.contacts_sheet_id,
        &config.contacts_sheet_name,
        kv,
    )
    .await
    .map_err(AppError::Internal)?;

    let total = contacts.len();
    Ok(ApiOk::new(ContactsListResponse { contacts, total }))
}

// ---------------------------------------------------------------------------
// GET /api/contacts/events — list events from the Events tab
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct EventsTabListResponse {
    pub events: Vec<events_tab::EventTabRow>,
    pub total: usize,
}

#[worker::send]
pub async fn list_events_tab_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<EventsTabListResponse>, WorkerError> {
    crate::auth::require_super_admin(&claims.email, &state, "view the events tab").await?;
    let config = &state.config.sheets;
    if config.contacts_sheet_id.is_empty() {
        return Ok(ApiOk::new(EventsTabListResponse {
            events: vec![],
            total: 0,
        }));
    }

    let kv = state.events_kv.as_ref();
    let events = events_tab::list_events_tab(
        &state,
        &config.contacts_sheet_id,
        &config.events_sheet_name,
        kv,
    )
    .await
    .map_err(AppError::Internal)?;

    let total = events.len();
    Ok(ApiOk::new(EventsTabListResponse { events, total }))
}

// ---------------------------------------------------------------------------
// GET /api/contacts/stats — contact statistics
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct ContactsStatsResponse {
    /// Total unique emails across all events.
    pub total_contacts: usize,
    /// Contacts who joined more than 1 event.
    pub repeat_attendees: usize,
    /// Top events by contact count.
    pub events: Vec<EventContactCount>,
}

#[derive(Serialize)]
pub struct EventContactCount {
    pub event_id: String,
    pub count: usize,
}

/// Tally how many distinct contacts registered for each event.
///
/// Input is [`AudienceRow::event_ids`], which is
/// `GROUP_CONCAT(DISTINCT a.event_id)` — computed fresh by the audience JOIN,
/// **not** the deprecated `contacts.events_joined` stored column. Extracted so
/// that distinction stays under test.
///
/// Ties are broken by event id so the output is deterministic; a `HashMap`
/// iteration order otherwise makes the response shuffle between identical calls.
fn tally_contacts_per_event(rows: &[AudienceRow]) -> Vec<EventContactCount> {
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for row in rows {
        for event_id in row.event_ids.split(',') {
            let id = event_id.trim();
            if !id.is_empty() {
                *counts.entry(id).or_insert(0) += 1;
            }
        }
    }
    let mut events: Vec<EventContactCount> = counts
        .into_iter()
        .map(|(event_id, count)| EventContactCount {
            event_id: event_id.to_string(),
            count,
        })
        .collect();
    events.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.event_id.cmp(&b.event_id)));
    events
}

/// Source of truth: the `attendees` table, via [`audience_aggregate`].
///
/// Previously this read the master contacts **Google Sheet** and then split the
/// denormalized `contacts.events_joined` CSV column — both of the stale read
/// paths that plan 008 §3.5 deprecated. The sheet lags live registrations and
/// the CSV column is overwritten on every upsert, so the numbers this endpoint
/// reported drifted from reality in two independent ways at once.
///
/// The per-event tally below still splits a comma-separated string, but that
/// string is `GROUP_CONCAT(DISTINCT a.event_id)` computed fresh by the JOIN —
/// not the stored column. Deriving it in SQL and splitting the result is the
/// sanctioned path; reading `contacts.events_joined` is not.
#[worker::send]
pub async fn contacts_stats_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<ContactsStatsResponse>, WorkerError> {
    crate::auth::require_super_admin(&claims.email, &state, "view contact stats").await?;

    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    // `None` ⇒ aggregate across every event.
    let rows = audience_aggregate(db, None)
        .await
        .map_err(AppError::Internal)?;

    // One row per distinct lowercased email, so the row count IS the contact count.
    let total_contacts = rows.len();
    let repeat_attendees = rows.iter().filter(|r| r.events_joined > 1).count();

    let events = tally_contacts_per_event(&rows);

    Ok(ApiOk::new(ContactsStatsResponse {
        total_contacts,
        repeat_attendees,
        events,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/contacts/sync — backfill from all event sheets
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct ContactsSyncResponse {
    /// Total contacts processed.
    pub synced: usize,
    /// New contacts added (not previously in master sheet).
    pub added: usize,
    /// Existing contacts updated (new event appended).
    pub updated: usize,
    /// Errors encountered (non-fatal).
    pub errors: usize,
}

#[worker::send]
pub async fn sync_contacts_handler(
    State(state): State<AppState>,
) -> Result<ApiOk<ContactsSyncResponse>, WorkerError> {
    let contacts_config = &state.config.sheets;
    if contacts_config.contacts_sheet_id.is_empty() {
        return Err(AppError::Validation("CONTACTS_SHEET_ID not configured".to_string()).into());
    }

    let kv = state.events_kv.as_ref();
    let d1 = state.d1.as_deref();

    // Load all events (KV → D1 fallback)
    let event_metas: Vec<event_checkin_domain::models::event::EventMeta> = if let Some(kv_ref) = kv
    {
        let index = crate::event_store::get_event_index(kv_ref)
            .await
            .map_err(AppError::Internal)?;
        index.events
    } else if let Some(db) = d1 {
        tracing::info!("no KV, loading event list from D1 for contacts sync");
        crate::db::events::list_events_as_meta(db)
            .await
            .map_err(AppError::Internal)?
    } else {
        return Err(AppError::Internal("neither KV nor D1 configured".to_string()).into());
    };

    let mut synced = 0usize;
    let mut added = 0usize;
    let mut updated = 0usize;
    let mut errors = 0usize;

    // Load existing contacts for dedup check
    let existing = contacts::list_contacts(
        &state,
        &contacts_config.contacts_sheet_id,
        &contacts_config.contacts_sheet_name,
        kv,
    )
    .await
    .unwrap_or_default();

    let mut existing_set: std::collections::HashSet<String> =
        existing.iter().map(|c| c.email.to_lowercase()).collect();

    // For each event, load attendees and upsert
    for event_meta in &event_metas {
        let config =
            match crate::event_store::get_event_config_with_fallback(kv, d1, &event_meta.id)
                .await
                .map_err(AppError::Internal)?
            {
                Some(c) => c,
                None => continue,
            };

        // Resolve the contacts sheet from the event's organization
        let resolved = if let Some(db) = d1 {
            crate::org_store::resolve_contacts_sheet(db, &config, contacts_config).await
        } else {
            event_checkin_domain::models::org::ResolvedContactsSheet {
                sheet_id: contacts_config.contacts_sheet_id.clone(),
                contacts_sheet_name: contacts_config.contacts_sheet_name.clone(),
                events_sheet_name: contacts_config.events_sheet_name.clone(),
            }
        };

        if resolved.sheet_id.is_empty() {
            continue;
        }

        let attendees = sheets::get_attendees(&state, &config.sheet_id, &config.sheet_name, kv)
            .await
            .unwrap_or_default();

        // Sync event to Events tab (non-fatal)
        if let Err(e) = events_tab::upsert_event_tab(
            &config,
            attendees.len(),
            &state,
            &resolved.sheet_id,
            &resolved.events_sheet_name,
            kv,
        )
        .await
        {
            tracing::warn!(
                event_id = %config.id,
                error = %e,
                "failed to sync event to Events tab"
            );
        }

        for attendee in &attendees {
            let email = attendee.email.trim().to_lowercase();
            if email.is_empty() {
                continue;
            }

            let is_new = !existing_set.contains(&email);

            let upsert = ContactUpsert {
                email: &email,
                name: &attendee.name,
                event_id: &event_meta.id,
                contact_channel: attendee.contact_channel.as_deref(),
                contact_handle: attendee.contact_handle.as_deref(),
            };

            match contacts::upsert_contact(
                &upsert,
                &state,
                &resolved.sheet_id,
                &resolved.contacts_sheet_name,
                kv,
            )
            .await
            {
                Ok(()) => {
                    if is_new {
                        added += 1;
                        existing_set.insert(email);
                    } else {
                        updated += 1;
                    }
                    synced += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        %email,
                        event_id = %event_meta.id,
                        error = %e,
                        "failed to sync contact"
                    );
                    errors += 1;
                }
            }
        }
    }

    tracing::info!(synced, added, updated, errors, "contacts sync completed");

    Ok(ApiOk::new(ContactsSyncResponse {
        synced,
        added,
        updated,
        errors,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/contacts/audience — cross-event audience aggregation
// ---------------------------------------------------------------------------

/// Query params for `GET /api/contacts/audience`.
#[derive(Debug, serde::Deserialize)]
pub struct AudienceQuery {
    /// Comma-separated event IDs to scope the aggregation.
    /// Omit (or empty) to aggregate across ALL events.
    pub event_ids: Option<String>,
    /// Output format: `"csv"` to attach a CSV payload for download; anything
    /// else (or omitted) returns JSON rows only.
    pub format: Option<String>,
}

/// Response for `GET /api/contacts/audience`.
///
/// `rows` is always populated. `csv` / `filename` are attached only when
/// `format=csv` is requested — the frontend can use them directly for a
/// download, or build its own CSV from `rows` like the existing admin export.
#[derive(Serialize)]
pub struct AudienceResponse {
    /// Number of distinct emails in the result.
    pub total: usize,
    /// Per-email aggregate rows.
    pub rows: Vec<AudienceRow>,
    /// Present only when `format=csv`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub csv: Option<String>,
    /// Present only when `format=csv`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    /// Event IDs referenced by audience rows but absent from the events
    /// registry ("orphan" events). Attendees from these events appear in this
    /// cross-event view but can't be selected in the per-event admin dashboard.
    /// Empty (and omitted from JSON) when every referenced event is registered
    /// or when the registry itself couldn't be read (non-fatal).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unregistered_event_ids: Vec<String>,
}

/// `GET /api/contacts/audience?event_ids=a,b&format=csv`
///
/// Cross-event audience aggregation. Queries the `attendees` table directly
/// (source of truth), dedupes by `LOWER(email)`, and returns per-email
/// participation metrics enriched with `developer_profiles`.
///
/// - `event_ids` omitted/empty ⇒ ALL events.
/// - `format=csv` ⇒ response also includes a CSV string + filename for download.
///
/// This intentionally bypasses the denormalized `contacts.events_joined` CSV
/// column (which drifts); the `GROUP BY` here is computed fresh from real
/// registration rows.
#[worker::send]
pub async fn audience_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<AudienceQuery>,
) -> Result<ApiOk<AudienceResponse>, WorkerError> {
    // Cross-event/cross-org aggregation with CSV export of every email/wallet —
    // super-admin only (was ungated: any staff could dump the whole platform's
    // audience). See admin security review S1.
    crate::auth::require_super_admin(&claims.email, &state, "export the audience list").await?;

    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    // Parse `event_ids=a,b , c` → ["a","b","c"]. Empty/missing ⇒ all events.
    let parsed_ids: Option<Vec<String>> = query.event_ids.as_deref().map(|s| {
        s.split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect::<Vec<String>>()
    });
    let event_ids: Option<&[String]> = match &parsed_ids {
        Some(v) if !v.is_empty() => Some(v.as_slice()),
        _ => None,
    };

    let rows = audience_aggregate(db, event_ids)
        .await
        .map_err(|e| AppError::Internal(format!("D1 audience aggregate failed: {e}")))?;

    let total = rows.len();

    let want_csv = match query.format.as_deref() {
        Some(f) => f.eq_ignore_ascii_case("csv"),
        None => false,
    };

    let (csv, filename) = match want_csv {
        true => (
            Some(build_audience_csv(&rows)),
            Some(audience_csv_filename(event_ids)),
        ),
        false => (None, None),
    };

    // ── Orphan event_id detection (data hygiene) ──
    //
    // Each row's `event_ids` is a `GROUP_CONCAT(DISTINCT event_id)` CSV string.
    // Collect every referenced event_id across all rows, diff against the
    // events registry, and surface any that are NOT registered. Attendees from
    // unregistered events appear here (cross-event view) but can't be selected
    // in the per-event admin dashboard — the frontend warns the operator so
    // those rows aren't presumed "missing".
    let referenced: std::collections::HashSet<String> = rows
        .iter()
        .flat_map(|r| r.event_ids.split(','))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let registered = registered_event_ids(&state).await;
    let mut unregistered_event_ids: Vec<String> =
        referenced.difference(&registered).cloned().collect();
    unregistered_event_ids.sort();

    tracing::info!(
        total,
        scoped = event_ids.is_some(),
        want_csv,
        unregistered = unregistered_event_ids.len(),
        staff = %claims.email,
        "audience aggregation exported"
    );

    Ok(ApiOk::new(AudienceResponse {
        total,
        rows,
        csv,
        filename,
        unregistered_event_ids,
    }))
}

// ---------------------------------------------------------------------------
// Orphan event_id detection — data hygiene helper
// ---------------------------------------------------------------------------

/// Resolve the set of registered event IDs from the events registry.
///
/// Mirrors the resolution in `list_events` / `sync_contacts_handler`:
/// KV (`event_store::get_event_index`) first, then D1
/// (`db::events::list_events_as_meta`) fallback. Non-fatal — returns an empty
/// set if neither source is available, so the audience endpoint still serves
/// rows when the registry can't be read (the orphan warning simply won't fire).
async fn registered_event_ids(state: &AppState) -> std::collections::HashSet<String> {
    if let Some(kv_ref) = state.events_kv.as_ref()
        && let Ok(index) = crate::event_store::get_event_index(kv_ref).await
        && !index.events.is_empty()
    {
        return index.events.into_iter().map(|e| e.id).collect();
    }
    if let Some(db) = state.d1.as_deref()
        && let Ok(metas) = crate::db::events::list_events_as_meta(db).await
    {
        return metas.into_iter().map(|e| e.id).collect();
    }
    Default::default()
}

/// Build a CSV string from audience rows.
///
/// Column order mirrors `AudienceRow` field order, with header names chosen for
/// spreadsheet readability (spaces, not snake_case).
fn build_audience_csv(rows: &[AudienceRow]) -> String {
    let mut csv = String::from(
        "Email,Name,Events Joined,Checked In,Approved,In-Person,Online,\
         First Registered,Last Registered,Event IDs,Display Name,Experience,\
         Role,Location,Wallet,Consent Outreach\n",
    );
    for r in rows {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            escape_csv(&r.email),
            escape_csv(&r.name),
            r.events_joined,
            r.checked_in_count,
            r.approved_count,
            r.in_person_count,
            r.online_count,
            escape_csv(r.first_registered.as_deref().unwrap_or("")),
            escape_csv(r.last_registered.as_deref().unwrap_or("")),
            escape_csv(&r.event_ids),
            escape_csv(r.display_name.as_deref().unwrap_or("")),
            escape_csv(r.experience_level.as_deref().unwrap_or("")),
            escape_csv(r.primary_role.as_deref().unwrap_or("")),
            escape_csv(r.location_city.as_deref().unwrap_or("")),
            escape_csv(r.wallet_address.as_deref().unwrap_or("")),
            r.consent_outreach,
        ));
    }
    csv
}

/// Deterministic filename for the audience CSV export.
///
/// No timestamp: the export is computed fresh on each call, and a stable name
/// avoids `chrono`/`Utc::now()` edge cases on the wasm32 Workers runtime.
fn audience_csv_filename(event_ids: Option<&[String]>) -> String {
    match event_ids {
        Some(ids) => format!("audience-{}events.csv", ids.len()),
        None => "audience-all.csv".to_string(),
    }
}

/// Escape a CSV field containing commas, quotes, or newlines.
///
/// Mirrors the `escape_csv` helper in `walkin.rs` — kept local rather than
/// promoted to a shared util to match the established per-handler convention.
fn escape_csv(s: &str) -> String {
    match s.contains(',') || s.contains('"') || s.contains('\n') {
        true => format!("\"{}\"", s.replace('"', "\"\"")),
        false => s.to_string(),
    }
}

// ---------------------------------------------------------------------------
// GET /api/contacts/{email}/history — per-contact event history (Plan 008 §3.5)
// ---------------------------------------------------------------------------

/// Response for `GET /api/contacts/{email}/history`.
#[derive(Serialize)]
pub struct ContactHistoryResponse {
    /// The email the history was requested for (lowercased for lookup).
    pub email: String,
    /// Events this contact has attended, newest first.
    /// Source of truth: the `attendees` table — NOT `contacts.events_joined`.
    pub events: Vec<ContactEventRow>,
    /// Convenience: `events.len()`, surfaced for quick UI badge counts.
    pub total: usize,
}

/// `GET /api/contacts/{email}/history` — list every event a contact attended.
///
/// Reads from the `attendees` table JOINed to `events` (source of truth),
/// NOT the deprecated `contacts.events_joined` CSV column — see
/// [`crate::db::contacts::list_contact_events`] for the deprecation rationale.
///
/// Super-admin only: returns any contact's cross-event history (PII) with no
/// per-event scoping, so it must not be exposed to per-event staff (review S1).
/// The email is bound as a positional SQL parameter, never interpolated, to
/// guard against SQL injection.
#[worker::send]
pub async fn contact_history_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(email): Path<String>,
) -> Result<ApiOk<ContactHistoryResponse>, WorkerError> {
    crate::auth::require_super_admin(&claims.email, &state, "view contact history").await?;
    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    // Normalize: trim whitespace + lowercase to match the `contacts.email` PK
    // convention. The SQL query also LOWER()s both sides, so this is belt-and-
    // suspenders, but it makes the returned `email` field canonical.
    let email = email.trim().to_lowercase();

    let events = list_contact_events(db, &email)
        .await
        .map_err(AppError::Internal)?;

    let total = events.len();
    Ok(ApiOk::new(ContactHistoryResponse {
        email,
        events,
        total,
    }))
}


#[cfg(test)]
mod r9_tests {
    use super::*;

    fn row(email: &str, events_joined: i64, event_ids: &str) -> AudienceRow {
        AudienceRow {
            email: email.to_string(),
            events_joined,
            event_ids: event_ids.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn tallies_contacts_per_event() {
        let rows = vec![
            row("a@x", 2, "evt1,evt2"),
            row("b@x", 1, "evt1"),
            row("c@x", 1, "evt3"),
        ];
        let out = tally_contacts_per_event(&rows);
        assert_eq!(out[0].event_id, "evt1");
        assert_eq!(out[0].count, 2);
        assert_eq!(out.len(), 3);
    }

    /// `GROUP_CONCAT` can emit stray spaces, and an email with no registrations
    /// yields an empty string — neither may become a phantom event bucket.
    #[test]
    fn ignores_blank_and_whitespace_ids() {
        let rows = vec![row("a@x", 0, ""), row("b@x", 1, " evt1 , , evt2")];
        let out = tally_contacts_per_event(&rows);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|e| !e.event_id.is_empty()));
        assert!(out.iter().any(|e| e.event_id == "evt1"));
    }

    /// Equal counts must not shuffle between calls — HashMap order is random.
    #[test]
    fn ties_are_broken_deterministically() {
        let rows = vec![row("a@x", 3, "zeta,alpha,mid")];
        let first = tally_contacts_per_event(&rows);
        let second = tally_contacts_per_event(&rows);
        let ids: Vec<&str> = first.iter().map(|e| e.event_id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "mid", "zeta"]);
        assert_eq!(ids, second.iter().map(|e| e.event_id.as_str()).collect::<Vec<_>>());
    }

    #[test]
    fn empty_input_yields_no_events() {
        assert!(tally_contacts_per_event(&[]).is_empty());
    }

    /// The repeat-attendee predicate reads the JOIN's `COUNT(DISTINCT event_id)`,
    /// not the drifting stored CSV.
    #[test]
    fn repeat_attendees_use_the_joined_count() {
        let rows = vec![row("a@x", 2, "e1,e2"), row("b@x", 1, "e1"), row("c@x", 5, "e1")];
        assert_eq!(rows.iter().filter(|r| r.events_joined > 1).count(), 2);
    }
}

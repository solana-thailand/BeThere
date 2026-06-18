//! Contacts API handlers — master contacts list management.
//!
//! Provides endpoints for listing, syncing, and querying the deduplicated
//! master contacts sheet that tracks all attendees across all events.
//! Also manages the Events tab (event registry in the same sheet).

use axum::extract::{Query, State};
use axum::Extension;
use serde::Serialize;

use crate::db::contacts::{AudienceRow, audience_aggregate};
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
) -> Result<ApiOk<ContactsListResponse>, WorkerError> {
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
) -> Result<ApiOk<EventsTabListResponse>, WorkerError> {
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

#[worker::send]
pub async fn contacts_stats_handler(
    State(state): State<AppState>,
) -> Result<ApiOk<ContactsStatsResponse>, WorkerError> {
    let config = &state.config.sheets;
    if config.contacts_sheet_id.is_empty() {
        return Ok(ApiOk::new(ContactsStatsResponse {
            total_contacts: 0,
            repeat_attendees: 0,
            events: vec![],
        }));
    }

    let kv = state.events_kv.as_ref();
    let all_contacts = contacts::list_contacts(
        &state,
        &config.contacts_sheet_id,
        &config.contacts_sheet_name,
        kv,
    )
    .await
    .map_err(AppError::Internal)?;

    let total_contacts = all_contacts.len();
    let repeat_attendees = all_contacts.iter().filter(|c| c.event_count > 1).count();

    // Count contacts per event
    let mut event_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for contact in &all_contacts {
        for event_id in contact.events_joined.split(',') {
            let id = event_id.trim();
            if !id.is_empty() {
                *event_counts.entry(id.to_string()).or_insert(0) += 1;
            }
        }
    }

    let mut events: Vec<EventContactCount> = event_counts
        .into_iter()
        .map(|(event_id, count)| EventContactCount { event_id, count })
        .collect();
    events.sort_by_key(|e| std::cmp::Reverse(e.count));

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

//! Contacts API handlers — master contacts list management.
//!
//! Provides endpoints for listing, syncing, and querying the deduplicated
//! master contacts sheet that tracks all attendees across all events.
//! Also manages the Events tab (event registry in the same sheet).

use axum::extract::State;
use serde::Serialize;

use crate::error::{ApiOk, WorkerError};
use crate::sheets;
use crate::sheets::contacts::{self, ContactUpsert};
use crate::sheets::events_tab;
use crate::state::AppState;
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
        let resolved = match kv {
            Some(kv_ref) => {
                crate::org_store::resolve_contacts_sheet(kv_ref, &config, contacts_config).await
            }
            None => {
                // No KV: fall back to global sheets config (same as resolve_contacts_sheet fallback)
                event_checkin_domain::models::org::ResolvedContactsSheet {
                    sheet_id: contacts_config.contacts_sheet_id.clone(),
                    contacts_sheet_name: contacts_config.contacts_sheet_name.clone(),
                    events_sheet_name: contacts_config.events_sheet_name.clone(),
                }
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

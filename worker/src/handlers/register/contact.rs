//! Contacts-sheet upsert + developer-profile write helpers.

use crate::state::AppState;

use super::types::DeveloperData;

/// Non-fatal upsert to the master contacts sheet after successful registration.
/// Errors are logged but never block the registration response.
#[allow(clippy::too_many_arguments)]
pub(super) async fn upsert_contact_after_registration(
    email: &str,
    name: &str,
    event_id: &str,
    contact_channel: Option<&str>,
    contact_handle: Option<&str>,
    state: &AppState,
    event_config: &event_checkin_domain::models::event::EventConfig,
    kv: Option<&worker::KvStore>,
) {
    // Resolve the contacts sheet from the event's organization
    let resolved = if let Some(db) = state.d1.as_deref() {
        crate::org_store::resolve_contacts_sheet(db, event_config, &state.config.sheets).await
    } else {
        let global = &state.config.sheets;
        event_checkin_domain::models::org::ResolvedContactsSheet {
            sheet_id: global.contacts_sheet_id.clone(),
            contacts_sheet_name: global.contacts_sheet_name.clone(),
            events_sheet_name: global.events_sheet_name.clone(),
        }
    };

    if resolved.sheet_id.is_empty() {
        return; // Not configured — skip silently
    }

    let upsert = crate::sheets::contacts::ContactUpsert {
        email,
        name,
        event_id,
        contact_channel,
        contact_handle,
    };

    if let Err(e) = crate::sheets::contacts::upsert_contact(
        &upsert,
        state,
        &resolved.sheet_id,
        &resolved.contacts_sheet_name,
        kv,
    )
    .await
    {
        tracing::warn!(
            %email,
            %event_id,
            error = %e,
            "failed to upsert contact to master sheet (non-fatal)"
        );
    }
}

/// Write developer profile + registration responses to D1 (Issue #049 Phase 2).
///
/// Best-effort: each write is individually wrapped in warn-on-error.
pub(super) async fn write_developer_data(data: &DeveloperData<'_>) {
    let DeveloperData {
        d1,
        email,
        name,
        event_id,
        contact_channel,
        contact_handle,
        participation_type,
        consent_given,
        photo_consent_given,
        consent_marketing,
        profile_fields: _,
    } = data;

    // 1. Upsert developer profile (display_name + consent_outreach)
    if let Err(e) =
        crate::db::developers::upsert_developer_field(d1, email, "display_name", name).await
    {
        tracing::warn!(%email, error = %e, "D1 developer display_name upsert failed (non-fatal)");
    }

    // 1b. Upsert consent_outreach from marketing consent
    let consent_val = if *consent_marketing { "1" } else { "0" };
    if let Err(e) =
        crate::db::developers::upsert_developer_field(d1, email, "consent_outreach", consent_val)
            .await
    {
        tracing::warn!(%email, error = %e, "D1 developer consent_outreach upsert failed (non-fatal)");
    }

    // 1c. Upsert dynamic profile fields
    for (key, value) in &data.profile_fields {
        if !value.is_empty()
            && let Err(e) = crate::db::developers::upsert_developer_field(
                d1,
                email,
                key.as_str(),
                value.as_str(),
            )
            .await
        {
            tracing::warn!(%email, key, error = %e, "D1 developer field upsert failed (non-fatal)");
        }
    }

    // 2. Store registration responses (single batch INSERT — 1 D1 call instead of N)
    let mut responses: Vec<(&str, &str, bool)> = vec![
        ("participation_type", participation_type, false),
        ("contact_channel", contact_channel, false),
        ("contact_handle", contact_handle, false),
        (
            "consent_given",
            if *consent_given { "true" } else { "false" },
            false,
        ),
        (
            "photo_consent_given",
            if *photo_consent_given {
                "true"
            } else {
                "false"
            },
            false,
        ),
        (
            "consent_marketing",
            if *consent_marketing { "true" } else { "false" },
            false,
        ),
    ];

    // Profile-enriching fields
    for (key, value) in &data.profile_fields {
        if !value.is_empty() {
            responses.push((key.as_str(), value.as_str(), true));
        }
    }

    if let Err(e) =
        crate::db::developers::batch_insert_registration_responses(d1, event_id, email, &responses)
            .await
    {
        tracing::warn!(
            %email,
            %event_id,
            error = %e,
            "D1 batch registration responses failed (non-fatal)"
        );
    }
}

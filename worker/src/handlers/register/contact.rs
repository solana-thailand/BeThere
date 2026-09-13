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
            attendee_fingerprint = %state.log_fingerprint(email),
            %event_id,
            error = %e,
            "failed to upsert contact to master sheet (non-fatal)"
        );
    }
}

/// Most dynamic answers a single submission may carry.
///
/// `profile_fields` arrives as a free-form map on two *public* endpoints
/// (`register` and `register-post-event`), so its size is attacker-chosen. The
/// cap is generous against real use — a registration form plus a post-event
/// question set — and exists so one request cannot turn into an unbounded
/// number of rows.
const MAX_PROFILE_FIELDS: usize = 40;

/// Longest accepted key. Long enough for `post.satisfaction.overall`.
const MAX_PROFILE_KEY_LEN: usize = 64;

/// Longest accepted answer. Long enough for a free-text survey comment.
const MAX_PROFILE_VALUE_LEN: usize = 2_000;

/// The answers from a submission that are safe to persist.
///
/// Both registration entry points funnel through `write_developer_data`, so
/// applying the bound here covers them together — a guard on one caller would
/// leave the sibling path unbounded, which is how this class of bug survives.
/// Empty values are dropped (an unanswered optional field is not an answer).
fn accepted_profile_fields(fields: &[(String, String)]) -> Vec<(&str, &str)> {
    fields
        .iter()
        .filter(|(key, value)| {
            !value.is_empty()
                && !key.is_empty()
                && key.len() <= MAX_PROFILE_KEY_LEN
                && value.len() <= MAX_PROFILE_VALUE_LEN
        })
        .take(MAX_PROFILE_FIELDS)
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect()
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
        redactor,
    } = data;
    let attendee_fingerprint = redactor.fingerprint(email);

    // 1. Upsert developer profile (display_name + consent_outreach)
    if let Err(e) =
        crate::db::developers::upsert_developer_field(d1, email, "display_name", name).await
    {
        tracing::warn!(attendee_fingerprint = %attendee_fingerprint, error = %e, "D1 developer display_name upsert failed (non-fatal)");
    }

    // 1b. Upsert consent_outreach from marketing consent
    let consent_val = if *consent_marketing { "1" } else { "0" };
    if let Err(e) =
        crate::db::developers::upsert_developer_field(d1, email, "consent_outreach", consent_val)
            .await
    {
        tracing::warn!(attendee_fingerprint = %attendee_fingerprint, error = %e, "D1 developer consent_outreach upsert failed (non-fatal)");
    }

    // 1c. Upsert dynamic profile fields.
    //
    // `post.`-namespaced keys are answers about the event, not about the
    // person, so they are stored in `registration_responses` and nowhere else.
    // Sending them to the profile upsert would fail the allowlist and log once
    // per answer per respondent — noise that scales with the size of the
    // question set (DevRel phase-2 item 2). A non-namespaced key that fails to
    // resolve is still a real form-config mistake and still warns.
    for (key, value) in accepted_profile_fields(&data.profile_fields) {
        if crate::db::developers::is_event_scoped_field(key) {
            continue;
        }
        if let Err(e) = crate::db::developers::upsert_developer_field(d1, email, key, value).await {
            tracing::warn!(attendee_fingerprint = %attendee_fingerprint, key, error = %e, "D1 developer field upsert failed (non-fatal)");
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

    // Dynamic answers. `is_profile_field` records whether the answer *also*
    // updated `developer_profiles` — so an event-scoped `post.` answer, which
    // deliberately does not, must not claim it did. DevRel read this column to
    // work out where the existing 2,103 rows came from; it has to stay honest.
    for (key, value) in accepted_profile_fields(&data.profile_fields) {
        let is_profile_field = !crate::db::developers::is_event_scoped_field(key);
        responses.push((key, value, is_profile_field));
    }

    if let Err(e) =
        crate::db::developers::batch_insert_registration_responses(d1, event_id, email, &responses)
            .await
    {
        tracing::warn!(
            attendee_fingerprint = %attendee_fingerprint,
            %event_id,
            error = %e,
            "D1 batch registration responses failed (non-fatal)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_PROFILE_FIELDS, MAX_PROFILE_KEY_LEN, MAX_PROFILE_VALUE_LEN, accepted_profile_fields,
    };

    fn pair(key: &str, value: &str) -> (String, String) {
        (key.to_string(), value.to_string())
    }

    #[test]
    fn answers_survive_unchanged() {
        let fields = vec![pair("experience_level", "Senior"), pair("post.nps", "9")];
        assert_eq!(
            accepted_profile_fields(&fields),
            vec![("experience_level", "Senior"), ("post.nps", "9")]
        );
    }

    /// An untouched optional field is not an answer and must not become a row.
    #[test]
    fn empty_values_and_keys_are_dropped() {
        let fields = vec![
            pair("experience_level", ""),
            pair("", "orphan"),
            pair("interests", "DeFi"),
        ];
        assert_eq!(
            accepted_profile_fields(&fields),
            vec![("interests", "DeFi")]
        );
    }

    /// The caps are a policy choice, so they are pinned against literals rather
    /// than against themselves. The test below asserts the `take` is applied by
    /// reading `MAX_PROFILE_FIELDS`, which means it cannot notice the constant
    /// being raised to something useless — this one can.
    #[test]
    fn the_caps_stay_within_a_defensible_range() {
        assert!(
            (10..=100).contains(&MAX_PROFILE_FIELDS),
            "MAX_PROFILE_FIELDS = {MAX_PROFILE_FIELDS}: under 10 breaks a real              question set, over 100 stops being a bound on a public endpoint"
        );
        assert!((32..=256).contains(&MAX_PROFILE_KEY_LEN));
        assert!((256..=8_192).contains(&MAX_PROFILE_VALUE_LEN));
    }

    /// `profile_fields` is a free-form map on two public endpoints, so its size
    /// is chosen by the caller.
    #[test]
    fn an_oversized_submission_is_bounded() {
        let fields: Vec<(String, String)> = (0..MAX_PROFILE_FIELDS * 5)
            .map(|i| pair(&format!("post.q{i}"), "answer"))
            .collect();
        assert_eq!(accepted_profile_fields(&fields).len(), MAX_PROFILE_FIELDS);
    }

    #[test]
    fn overlong_keys_and_values_are_rejected_not_truncated() {
        let long_key = "p".repeat(MAX_PROFILE_KEY_LEN + 1);
        let long_value = "v".repeat(MAX_PROFILE_VALUE_LEN + 1);
        let fields = vec![
            pair(&long_key, "ok"),
            pair("post.comment", &long_value),
            pair("post.nps", "9"),
        ];
        // Truncating would store a corrupted answer; dropping is the honest
        // outcome and leaves the rest of the submission intact.
        assert_eq!(accepted_profile_fields(&fields), vec![("post.nps", "9")]);
    }

    /// The boundary itself, so an off-by-one in the comparison is caught.
    #[test]
    fn keys_and_values_at_the_limit_are_accepted() {
        let key = format!("post.{}", "k".repeat(MAX_PROFILE_KEY_LEN - "post.".len()));
        let value = "v".repeat(MAX_PROFILE_VALUE_LEN);
        assert_eq!(key.len(), MAX_PROFILE_KEY_LEN);
        let fields = vec![(key.clone(), value.clone())];
        assert_eq!(
            accepted_profile_fields(&fields),
            vec![(key.as_str(), value.as_str())]
        );
    }
}

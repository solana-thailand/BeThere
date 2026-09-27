//! Deposit-deadline and in-person-capacity gates.

use chrono::Utc;
use event_checkin_domain::models::event::EventConfig;

use crate::state::AppState;
use event_checkin_domain::models::attendee::SheetRow;

/// Check if the deposit deadline has passed and auto-switch participation_type.
/// Returns `true` if the deadline expired and the switch was performed.
pub(crate) async fn check_and_switch_deadline(
    state: &AppState,
    kv: Option<&worker::KvStore>,
    event: &EventConfig,
    attendee: &event_checkin_domain::models::attendee::Attendee,
    registration_date_str: &str,
) -> bool {
    let Some(deadline_hours) = event.deposit_deadline_hours else {
        return false;
    };

    // Parse registration_date (ISO 8601)
    let reg_time = match chrono::DateTime::parse_from_rfc3339(registration_date_str) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(e) => {
            tracing::warn!(
                attendee_id = %attendee.api_id,
                error = %e,
                raw = registration_date_str,
                "deposit deadline: failed to parse registration_date"
            );
            return false;
        }
    };

    let deadline = reg_time + chrono::Duration::hours(i64::from(deadline_hours));
    let now = Utc::now();

    if now <= deadline {
        return false; // Still within deadline
    }

    // Deadline passed — auto-switch participation_type in the sheet
    tracing::info!(
        attendee_id = %attendee.api_id,
        event_id = %event.id,
        deadline = %deadline.to_rfc3339(),
        "deposit deadline expired: auto-switching participation_type to Online"
    );

    // Get column mapping for the sheet
    let mapping = match crate::sheets::get_column_mapping(
        state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
    )
    .await
    {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "deadline switch: failed to get column mapping");
            return true; // Deadline expired even if we can't switch
        }
    };

    if let Some(ctx) = &state.worker_ctx {
        ctx.wait_until(crate::sheets::bg_sync::update_participation_type(
            state.clone(),
            SheetRow::of(attendee.api_id.clone()),
            "Online".to_string(),
            mapping,
            event.sheet_id.clone(),
            event.sheet_name.clone(),
            kv.cloned(),
        ));
        tracing::info!(
            attendee_id = %attendee.api_id,
            "deposit deadline: participation_type switched to Online (bg)"
        );
    } else {
        match crate::sheets::write::update_participation_type(
            SheetRow::of(attendee.api_id.clone()),
            "Online",
            &mapping,
            state,
            &event.sheet_id,
            &event.sheet_name,
            kv,
        )
        .await
        {
            Ok(()) => tracing::info!(
                attendee_id = %attendee.api_id,
                "deposit deadline: participation_type switched to Online"
            ),
            Err(e) => tracing::warn!(
                attendee_id = %attendee.api_id,
                error = %e,
                "deposit deadline: failed to update sheet participation_type"
            ),
        }
    }

    true
}

/// Check if in-person capacity is still available for the event.
/// Returns `true` if spots are available (or unlimited), `false` if full.
pub(crate) async fn check_in_person_capacity(
    state: &AppState,
    event: &EventConfig,
    kv: Option<&worker::KvStore>,
) -> bool {
    // No capacity limit = always available (handled by has_in_person_capacity)
    if event.in_person_capacity.is_none() {
        return true;
    }

    // Walk-ins included (.issues/157). Assume full when the count is unknown.
    match crate::handlers::capacity::count_tracks(state, event, kv).await {
        Ok(counts) => event.has_in_person_capacity(counts.in_person),
        Err(e) => {
            tracing::warn!(error = %e, "reclaim capacity: failed to count attendees");
            false
        }
    }
}

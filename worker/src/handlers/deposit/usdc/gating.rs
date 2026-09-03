//! Deposit-deadline and in-person-capacity gates.

use chrono::Utc;
use event_checkin_domain::models::event::EventConfig;

use crate::state::AppState;

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
            attendee.row_index,
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
            attendee.row_index,
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

    // Count in-person attendees from sheet
    let attendees = match crate::sheets::get_attendees_for_event(
        state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
        &event.id,
    )
    .await
    {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(error = %e, "reclaim capacity: failed to get attendees");
            return false; // Assume full on error
        }
    };

    let in_person_count = attendees.iter().filter(|a| a.is_in_person()).count() as u32;

    // Count walk-in attendees from D1
    let mut walkin_count: u32 = 0;
    if let Some(db) = state.d1.as_deref() {
        match crate::db::attendees::count_walkin_attendees(db, &event.id).await {
            Ok(count) => walkin_count = count,
            Err(e) => {
                tracing::warn!(error = %e, "reclaim capacity: failed to count D1 walkins");
            }
        }
    }

    let total = in_person_count + walkin_count;
    event.has_in_person_capacity(total)
}

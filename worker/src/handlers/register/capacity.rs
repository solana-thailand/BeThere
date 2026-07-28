//! Capacity enforcement helper for registration.

use event_checkin_domain::models::attendee::ParticipationType;
use event_checkin_domain::models::error::AppError;

use crate::state::AppState;

/// Enforce capacity limits before registration.
/// Returns an error if the selected track is full or not available.
pub(super) async fn enforce_capacity(
    state: &AppState,
    config: &event_checkin_domain::models::event::EventConfig,
    participation_type: &str,
    kv: Option<&worker::kv::KvStore>,
) -> Result<(), AppError> {
    use event_checkin_domain::models::event::OnlineOpenMode;

    // Judge the registering attendee with the SAME canonical enum used for
    // existing attendees (Attendee::is_in_person) below — keeps both checks
    // consistent and covers the `in_person`/`physical` variants the old inline
    // matcher missed.
    let is_in_person = matches!(
        ParticipationType::parse(participation_type),
        ParticipationType::InPerson
    );

    // Count current attendees from sheet
    let attendees = crate::sheets::get_attendees_for_event(
        state,
        &config.sheet_id,
        &config.sheet_name,
        kv,
        &config.id,
    )
    .await
    .map_err(|e| AppError::Internal(format!("failed to check capacity: {e}")))?;

    let mut in_person_count: u32 = 0;
    let mut online_count: u32 = 0;
    for a in &attendees {
        if a.is_in_person() {
            in_person_count += 1;
        } else {
            online_count += 1;
        }
    }

    // Count walk-in attendees from D1
    if let Some(db) = state.d1.as_deref() {
        match crate::db::attendees::count_walkin_attendees(db, &config.id).await {
            Ok(count) => {
                in_person_count += count;
            }
            Err(e) => {
                tracing::warn!(error = %e, "D1 walkin count for capacity failed, skipping");
            }
        }
    }

    tracing::info!(
        event_id = %config.id,
        participation_type = %participation_type,
        is_in_person = is_in_person,
        in_person_count = in_person_count,
        online_count = online_count,
        in_person_capacity = ?config.in_person_capacity,
        online_capacity = ?config.online_capacity,
        "capacity check"
    );

    if is_in_person {
        // Check in-person capacity
        if !config.has_in_person_capacity(in_person_count) {
            return Err(AppError::Validation(
                "In-person spots are full. Please register for the online track instead."
                    .to_string(),
            ));
        }
    } else {
        // Check online capacity
        if !config.has_online_capacity(online_count) {
            return Err(AppError::Validation(
                "Online spots are full. Registration is closed.".to_string(),
            ));
        }

        // Check online registration gating
        let in_person_available = config.has_in_person_capacity(in_person_count);

        let online_open = match config.online_open_mode {
            OnlineOpenMode::Always => true,
            OnlineOpenMode::AutoOnFull => !in_person_available,
            OnlineOpenMode::Manual => config.online_registration_open,
        };

        if !online_open {
            return Err(AppError::Validation(
                "Online registration is not open yet. Please check back later or register for the in-person track.".to_string(),
            ));
        }
    }

    Ok(())
}

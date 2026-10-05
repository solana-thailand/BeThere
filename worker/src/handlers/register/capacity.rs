//! Capacity enforcement helper for registration.

use event_checkin_domain::models::attendee::{ParticipationType, TrackCounts};
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

    // One count for both tracks, walk-ins included. Fails closed — see
    // `handlers::capacity`.
    let TrackCounts {
        in_person: in_person_count,
        online: online_count,
    } = crate::handlers::capacity::count_tracks_for_cap(state, config, kv).await?;

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

    // The in-person track closes when the event ends; online stays open.
    let in_person_closed =
        config.in_person_registration_closed(chrono::Utc::now().timestamp_millis());

    if is_in_person {
        if in_person_closed {
            let msg = match config.event_format.has_online() {
                true => {
                    "In-person registration closed when the event ended. You can still register for the online track."
                }
                false => "Registration closed when the event ended.",
            };
            return Err(AppError::Validation(msg.to_string()));
        }
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
        let in_person_available =
            !in_person_closed && config.has_in_person_capacity(in_person_count);

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

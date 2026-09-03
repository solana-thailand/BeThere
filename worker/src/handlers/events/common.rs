//! Helpers shared by the per-event handler modules (`summary`, `recap`,
//! `pr_pack`, `post_event_registration`).
//!
//! These four modules each carried a byte-identical private `load_event` /
//! `enforce_organizer` pair, with a comment deferring the extraction until a
//! fourth consumer appeared. It has.
//!
//! The extraction also fixes a defect the four copies shared: they matched on
//! `Ok(Some(_))` and let every other arm fall through to
//! `AppError::NotFound`, so a KV or D1 outage told the organizer their event
//! did not exist. That is the same masking `resolve_event_by_slug` carried
//! before `734aa4b`; the split is `NotFound` (the store answered, the row is
//! absent) vs `Internal` (the store did not answer).

use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::EventConfig;

use crate::state::AppState;

/// Load an event by id, KV first then D1 fallback.
///
/// A KV read failure is non-fatal — KV is a cache and D1 is the source of
/// truth, so we log and fall through. A D1 read failure is fatal: at that
/// point no store has answered and "not found" would be a lie.
pub async fn load_event(
    state: &AppState,
    kv: Option<&worker::KvStore>,
    id: &str,
) -> Result<EventConfig, AppError> {
    let mut kv_failed = false;

    if let Some(kv_ref) = kv {
        match crate::event_store::get_event(kv_ref, id).await {
            Ok(Some(config)) => return Ok(config),
            Ok(None) => {}
            Err(e) => {
                kv_failed = true;
                tracing::warn!(
                    event_id = %id,
                    error = %e,
                    "KV event read failed — falling back to D1"
                );
            }
        }
    }

    let Some(db) = state.d1.as_deref() else {
        return match kv_failed {
            true => Err(AppError::Internal(
                "could not load the event — please try again".into(),
            )),
            false => Err(AppError::NotFound(format!("event '{id}' not found"))),
        };
    };

    match crate::db::events::get_event(db, id).await {
        Ok(Some(row)) => Ok(row.to_event_config()),
        Ok(None) => Err(AppError::NotFound(format!("event '{id}' not found"))),
        Err(e) => {
            tracing::error!(event_id = %id, error = %e, "D1 event read failed");
            Err(AppError::Internal(
                "could not load the event — please try again".into(),
            ))
        }
    }
}

/// Reject anyone below Organizer for this event (Staff included).
///
/// `action` completes the sentence "only super admins or organizers can
/// {action}" — e.g. `"view event summaries"`.
pub async fn enforce_organizer(
    claims: &Claims,
    state: &AppState,
    event: &EventConfig,
    action: &str,
) -> Result<(), AppError> {
    let role = crate::auth::resolve_user_role(&claims.email, state, Some(event)).await;
    match role < crate::auth::UserRole::Organizer {
        true => Err(AppError::Forbidden(format!(
            "only super admins or organizers can {action}"
        ))),
        false => Ok(()),
    }
}

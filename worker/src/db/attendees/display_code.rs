//! The short booking display code (`.issues/178`, migration 0058).
//!
//! Codes are assigned **after** the row exists, by a conditional `UPDATE … WHERE
//! display_code IS NULL`, rather than inside each writer's `INSERT`. That keeps
//! one retry loop instead of six, and a failed assignment never fails the
//! registration, walk-in or sync that created the row: the row simply keeps a
//! NULL code until the next call here (the ticket read calls it), which is the
//! same state the migration's last-resort pass may leave behind.
//!
//! Every statement filters on `event_id`: `attendees.id` is global
//! (`.issues/153`) and a code is unique only within one event.

use event_checkin_domain::models::attendee::{DISPLAY_CODE_RANDOM_BYTES, DisplayCode};
use worker::D1Database;
use worker::d1::D1Type;

/// How many fresh codes to try before giving up on a unique-index collision.
/// With 31^6 codes per event a single collision is already ~1e-6 at our
/// sizes; five in a row means something other than chance is wrong.
pub(crate) const DISPLAY_CODE_MAX_ATTEMPTS: usize = 5;

/// Whether a D1 error is the per-event unique index rejecting a duplicate code.
pub(crate) fn is_display_code_collision(message: &str) -> bool {
    message.contains("UNIQUE constraint failed") && message.contains("display_code")
}

/// A fresh random code from the runtime CSPRNG.
pub(crate) fn draw_display_code() -> Result<DisplayCode, String> {
    // `None` only when almost every byte was rejected (odds < 1e-12); draw again.
    for _ in 0..4 {
        let bytes = crate::crypto::random_bytes::<DISPLAY_CODE_RANDOM_BYTES>()?;
        if let Some(code) = DisplayCode::from_random_bytes(&bytes) {
            return Ok(code);
        }
    }
    Err("display code: CSPRNG returned unusable bytes repeatedly".to_string())
}

/// The stored code of one attendee of `event_id`, if the row has one.
///
/// `Ok(None)` for a missing row, another event's id, or a NULL code.
pub(crate) async fn get_display_code(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
) -> Result<Option<String>, String> {
    // `IS NOT NULL` in SQL, so the column read never meets a JS `null`
    // (`d1_safe` explains why that matters on worker 0.8).
    db.prepare(
        "SELECT display_code FROM attendees \
         WHERE id = ?1 AND event_id = ?2 AND display_code IS NOT NULL",
    )
    .bind_refs(&[D1Type::Text(attendee_id), D1Type::Text(event_id)])
    .map_err(|e| format!("D1 get_display_code bind: {e:?}"))?
    .first::<String>(Some("display_code"))
    .await
    .map_err(|e| format!("D1 get_display_code: {e:?}"))
}

/// Give the attendee a code if they have none, and return their code.
///
/// Idempotent: a row that already has a code keeps it (the `UPDATE` matches
/// nothing, and the stored code is read back). Retries a unique-index
/// collision up to [`DISPLAY_CODE_MAX_ATTEMPTS`] times with a fresh code.
/// `Ok(None)` when no row `(attendee_id, event_id)` exists.
pub(crate) async fn assign_display_code(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
) -> Result<Option<String>, String> {
    for attempt in 1..=DISPLAY_CODE_MAX_ATTEMPTS {
        let code = draw_display_code()?;
        let result = db
            .prepare(
                "UPDATE attendees SET display_code = ?1 \
                 WHERE id = ?2 AND event_id = ?3 AND display_code IS NULL",
            )
            .bind_refs(&[
                D1Type::Text(code.as_str()),
                D1Type::Text(attendee_id),
                D1Type::Text(event_id),
            ])
            .map_err(|e| format!("D1 assign_display_code bind: {e:?}"))?
            .run()
            .await;
        match result {
            Ok(done) => {
                let changes = done
                    .meta()
                    .map_err(|e| format!("D1 assign_display_code meta: {e:?}"))?
                    .and_then(|m| m.changes)
                    .unwrap_or(0);
                return match changes {
                    0 => get_display_code(db, event_id, attendee_id).await,
                    _ => Ok(Some(code.as_str().to_string())),
                };
            }
            Err(e) => {
                let message = format!("{e:?}");
                if !is_display_code_collision(&message) {
                    return Err(format!("D1 assign_display_code: {message}"));
                }
                tracing::info!(attempt, "display code collision in event, drawing again");
            }
        }
    }
    Err(format!(
        "D1 assign_display_code: {DISPLAY_CODE_MAX_ATTEMPTS} collisions in a row"
    ))
}

/// The code to show on a ticket: the stored one, or a newly assigned one for a
/// row that predates its writer setting it (rows inserted by old code between
/// migration 0058 and the deploy, or left NULL by the migration's last pass).
pub(crate) async fn ensure_display_code(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
) -> Result<Option<String>, String> {
    match get_display_code(db, event_id, attendee_id).await? {
        Some(code) => Ok(Some(code)),
        None => assign_display_code(db, event_id, attendee_id).await,
    }
}

/// Best-effort assignment right after a writer inserted the row. A failure is
/// logged, not returned: the ticket read assigns it later.
pub(crate) async fn assign_display_code_best_effort(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
) {
    if let Err(e) = assign_display_code(db, event_id, attendee_id).await {
        tracing::warn!(error = %e, "display code assignment deferred to first ticket read");
    }
}

/// Staff lookup: the attendee id holding `code` in `event_id`, if any.
///
/// The caller must already have authorized the event. A code from another
/// event matches nothing here, by construction of the `WHERE`.
pub(crate) async fn find_attendee_id_by_display_code(
    db: &D1Database,
    event_id: &str,
    code: &DisplayCode,
) -> Result<Option<String>, String> {
    db.prepare("SELECT id FROM attendees WHERE event_id = ?1 AND display_code = ?2")
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(code.as_str())])
        .map_err(|e| format!("D1 find_attendee_id_by_display_code bind: {e:?}"))?
        .first::<String>(Some("id"))
        .await
        .map_err(|e| format!("D1 find_attendee_id_by_display_code: {e:?}"))
}

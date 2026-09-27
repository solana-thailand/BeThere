//! The attendee list registration checks read (duplicate email, capacity).
//!
//! `get_attendees_for_event` is D1-first but falls back to the event's sheet
//! when D1 has no rows, which is every brand-new event. An unreachable sheet
//! then failed the very first registration with a 500. D1 is the source of
//! truth, so when the sheet read fails this re-reads D1 and trusts its answer,
//! including an empty one. Only a D1 failure on top fails the request.

use event_checkin_domain::models::attendee::Attendee;
use event_checkin_domain::models::event::EventConfig;
use worker::KvStore;

use crate::state::AppState;

pub(super) async fn registration_attendees(
    state: &AppState,
    config: &EventConfig,
    kv: Option<&KvStore>,
) -> Result<Vec<Attendee>, String> {
    let sheet_err = match crate::sheets::get_attendees_for_event(
        state,
        &config.sheet_id,
        &config.sheet_name,
        kv,
        &config.id,
    )
    .await
    {
        Ok(attendees) => return Ok(attendees),
        Err(e) => e,
    };
    let Some(d1) = state.d1.as_deref() else {
        return Err(sheet_err);
    };
    let attendees = crate::db::attendees::get_attendees_by_event(d1, &config.id)
        .await
        .map_err(|d1_err| format!("sheet: {sheet_err} | D1: {d1_err}"))?;
    tracing::warn!(
        event_id = %config.id,
        count = attendees.len(),
        error = %sheet_err,
        "sheet unreadable — registration checks use D1"
    );
    Ok(attendees)
}

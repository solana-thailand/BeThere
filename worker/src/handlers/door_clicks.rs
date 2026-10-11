//! `POST /api/public/click` (.plans/045 R4.10): `{ page, door }` from the
//! site's doors via `navigator.sendBeacon`, which sends text/plain, so the
//! body is read as bytes and parsed here. Unknown names are refused; nothing
//! about the caller is read or stored. The answer is always 204.

use axum::{body::Bytes, extract::State, http::StatusCode};
use serde::Deserialize;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct Click {
    page: String,
    door: String,
}

#[worker::send]
pub async fn click(State(state): State<AppState>, body: Bytes) -> StatusCode {
    let Ok(click) = serde_json::from_slice::<Click>(&body) else {
        return StatusCode::NO_CONTENT;
    };
    if !crate::door_clicks::is_valid(&click.page, &click.door) {
        return StatusCode::NO_CONTENT;
    }
    if let Some(db) = state.d1.as_deref()
        && let Err(e) = crate::door_clicks::count(db, &click.page, &click.door).await
    {
        tracing::warn!(error = %e, "door click not counted");
    }
    StatusCode::NO_CONTENT
}

//! `GET /robots.txt`, `/sitemap.xml`, `/llms.txt` (`crate::crawl` renders
//! them). The open events are the same list `GET /api/public/events` serves.

use axum::extract::State;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, FixedOffset};
use serde_json::Value;

use crate::crawl::{OpenEvent, llms_txt, robots_txt, sitemap_xml};
use crate::state::AppState;

/// Bangkok, where every event so far has been held.
const UTC_PLUS_7_SECS: i32 = 7 * 3600;

fn text(body: String, content_type: &'static str) -> Response {
    let mut resp = body.into_response();
    resp.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    resp
}

fn starts(event: &Value) -> String {
    if event.get("time_tba").and_then(Value::as_bool) == Some(true) {
        return "date to be announced".to_string();
    }
    let at = event
        .get("event_start_ms")
        .and_then(Value::as_i64)
        .and_then(DateTime::from_timestamp_millis);
    match (at, FixedOffset::east_opt(UTC_PLUS_7_SECS)) {
        (Some(at), Some(tz)) => at
            .with_timezone(&tz)
            .format("%Y-%m-%d %H:%M (UTC+7)")
            .to_string(),
        _ => "date to be announced".to_string(),
    }
}

/// The open events, or none when the store cannot be read: a crawler
/// document never fails over a list.
async fn open_events(state: &AppState) -> Vec<OpenEvent> {
    let events = match super::public_event::upcoming_public_events(state).await {
        Ok(events) => events,
        Err(e) => {
            tracing::warn!(error = %e, "crawl: open events unreadable, listing none");
            return Vec::new();
        }
    };
    let field = |e: &Value, key: &str| {
        e.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    events
        .iter()
        .filter(|e| !field(e, "slug").is_empty())
        .map(|e| OpenEvent {
            name: field(e, "name"),
            slug: field(e, "slug"),
            starts: starts(e),
            location: field(e, "location"),
        })
        .collect()
}

#[worker::send]
pub async fn robots(State(state): State<AppState>) -> Response {
    text(
        robots_txt(&state.config.server.url),
        "text/plain; charset=utf-8",
    )
}

#[worker::send]
pub async fn sitemap(State(state): State<AppState>) -> Response {
    let events = open_events(&state).await;
    text(
        sitemap_xml(&state.config.server.url, &events),
        "application/xml; charset=utf-8",
    )
}

#[worker::send]
pub async fn llms(State(state): State<AppState>) -> Response {
    let events = open_events(&state).await;
    text(
        llms_txt(&state.config.server.url, &events),
        "text/plain; charset=utf-8",
    )
}

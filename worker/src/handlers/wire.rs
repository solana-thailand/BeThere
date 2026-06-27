//! Wire-protocol smoke endpoints (Plan 014 Phase 1.3).
//!
//! These exist to prove the zero-copy wire path end-to-end before any
//! production endpoint opts in. The handler encodes a fixed Pod value via
//! the shared [`event_checkin_domain::wire`] envelope and returns it with
//! `Content-Type: application/x-bethere-bin` when `?fmt=bin` is requested.
//! JSON remains the default so the existing client path is byte-identical.
//!
//! Once the GOAT-gate (Task 1.7: ≥3× decode speedup AND ≥40% payload reduction)
//! clears on a real production endpoint, this module can be removed.

use std::collections::HashMap;

use axum::extract::Query;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use serde_json::json;

use event_checkin_domain::models::adventure::LevelScore;
use event_checkin_domain::wire;

/// `GET /api/wire-sample/level-score[?fmt=bin]`
///
/// Returns a fixed [`LevelScore`] sample. Default format is JSON (canonical);
/// `?fmt=bin` switches to the BLAKE3-committed wire envelope with
/// `Content-Type: application/x-bethere-bin`.
///
/// No auth, no state — this is a read-only probe.
#[worker::send]
pub async fn level_score_sample(Query(params): Query<HashMap<String, String>>) -> Response {
    let sample = LevelScore {
        moves: 7,
        puzzles_solved: 2,
        time_seconds: 45,
        stars: 2,
        _pad: [0; 3],
    };

    let want_bin = params.get("fmt").is_some_and(|s| s == "bin");

    if want_bin {
        let bytes = wire::pack(&sample);
        wire_response(bytes)
    } else {
        // Canonical JSON path — kept byte-identical to existing endpoints.
        Json(json!({
            "moves": sample.moves,
            "puzzles_solved": sample.puzzles_solved,
            "time_seconds": sample.time_seconds,
            "stars": sample.stars,
        }))
        .into_response()
    }
}

/// Wrap a wire-encoded byte buffer in an axum `Response` with the wire
/// content-type. Mirrors the `service_unavailable` pattern in `lib.rs`.
fn wire_response(bytes: Vec<u8>) -> Response {
    let mut resp = Response::new(axum::body::Body::from(bytes));
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(wire::CONTENT_TYPE),
    );
    resp
}

//! Wire-protocol API client (Plan 014 Phase 1.3).
//!
//! Calls the `?fmt=bin` endpoints and decodes the BLAKE3-committed envelope
//! via the shared `event_checkin_domain::wire` module. This is the client-side
//! half of the zero-copy wire path — the worker encodes, we decode.
//!
//! JSON endpoints continue to use the existing `response_json` path; these
//! functions are only for endpoints that opt into `?fmt=bin`.

use event_checkin_domain::models::adventure::LevelScore;
use event_checkin_domain::wire;

use super::fetch::{get_no_cache, response_array_buffer};
use super::types::ApiError;

/// `GET /api/wire-sample/level-score?fmt=bin`
///
/// Smoke-test client for the zero-copy wire path. Fetches the fixed
/// `LevelScore` sample as a binary envelope and decodes it via
/// [`wire::unpack`]. Returns a zero-copy view into the response buffer.
///
/// This is the end-to-end proof that the wire protocol works across the
/// HTTP boundary. Once a production endpoint adopts `?fmt=bin`, this
/// function can be removed.
pub async fn get_wire_sample_level_score() -> Result<LevelScore, ApiError> {
    let response = get_no_cache(
        "/api/wire-sample/level-score?fmt=bin",
        &[("Accept", wire::CONTENT_TYPE)],
    )
    .await?;

    if !response.ok() {
        return Err(ApiError {
            message: format!("wire sample request failed: HTTP {}", response.status()),
            status: response.status(),
        });
    }

    let bytes = response_array_buffer(&response).await?;
    let decoded: &LevelScore = wire::unpack(&bytes).map_err(|e| ApiError {
        message: format!("wire decode failed: {e}"),
        status: 0,
    })?;

    // Copy out of the response buffer into an owned value — the buffer is
    // dropped when this function returns. The zero-copy win is on the decode
    // side (no `serde_json` parse), not on this final 16-byte copy.
    Ok(*decoded)
}

//! PR pack (Plan 008 Phase 4).


use crate::api::types::{ApiError, ApiResponse};
use crate::api::{
    api_get, fetch::response_json,
};


// ===== PR Pack (Plan 008 Phase 4) =====

/// Generated marketing copy for one event. Mirrors `domain::pr_pack::PrPack`.
/// All fields are `String` (or `Vec<String>` for organizers) so they drop
/// straight into copy-to-clipboard cards.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PrPack {
    pub headline: String,
    pub short_blurb: String,
    pub social_post: String,
    pub calendar_text: String,
    pub email_snippet: String,
    pub deposit_terms: String,
    #[serde(default)]
    pub organizers: Vec<String>,
}

/// Wrapper returned by `/events/{id}/pr-pack`.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct PrPackData {
    pub event_id: String,
    pub pack: PrPack,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub source_config_version: String,
}

/// GET /api/events/{id}/pr-pack — generate the PR pack (deterministic).
///
/// Organizer-gated server-side. Returns 403 for Staff, 404 for unknown events.
pub async fn get_pr_pack(id: &str) -> Result<PrPackData, ApiError> {
    let path = format!("/events/{id}/pr-pack");
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Failed to load PR pack".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: 0,
        });
    }

    let wrapper: ApiResponse<PrPackData> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse PR pack response: {e}"),
            status: 0,
        })?;

    wrapper.data.ok_or_else(|| ApiError {
        message: wrapper.error.unwrap_or("No data".to_string()),
        status: 0,
    })
}

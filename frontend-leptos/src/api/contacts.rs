//! Contacts API client — cross-event audience aggregation.
//!
//! The audience endpoints query the `attendees` table directly (source of truth)
//! with `GROUP BY LOWER(email)`, returning per-email participation metrics
//! across the selected events (or ALL events when no filter is supplied).
//!
//! This intentionally bypasses the denormalized `contacts.events_joined` CSV
//! column, which drifts; the aggregation is computed fresh from real
//! registration rows on every call.

use serde::Deserialize;

use super::fetch::response_json;
use super::types::{ApiError, ApiResponse};
use super::api_get;

// ===== Audience types =====

/// One row in the cross-event audience aggregation (one per distinct email).
///
/// Field set mirrors the backend `AudienceRow`
/// (`worker/src/db/contacts.rs`). Numeric aggregates default to 0; enrichment
/// columns from `developer_profiles` are `None` when no profile row exists.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AudienceRow {
    /// Lowercased email (the group key).
    pub email: String,
    #[serde(default)]
    pub name: String,
    /// Distinct events this email registered for.
    #[serde(default)]
    pub events_joined: i64,
    /// Registrations where the attendee actually checked in.
    #[serde(default)]
    pub checked_in_count: i64,
    /// Registrations with `approval_status = 'approved'`.
    #[serde(default)]
    pub approved_count: i64,
    /// Registrations whose participation type is in-person (or empty/legacy).
    #[serde(default)]
    pub in_person_count: i64,
    /// Registrations whose participation type is online/virtual.
    #[serde(default)]
    pub online_count: i64,
    /// Earliest `created_at` across this email's registrations.
    pub first_registered: Option<String>,
    /// Latest `created_at` across this email's registrations.
    pub last_registered: Option<String>,
    /// Comma-separated distinct event IDs this email joined.
    #[serde(default)]
    pub event_ids: String,
    // ── Optional enrichment from `developer_profiles` ──
    pub display_name: Option<String>,
    pub experience_level: Option<String>,
    pub primary_role: Option<String>,
    pub location_city: Option<String>,
    pub wallet_address: Option<String>,
    /// PDPA outreach consent (0/1). Defaults to 0 when no profile row.
    #[serde(default)]
    pub consent_outreach: i64,
}

/// Response from `GET /api/contacts/audience`.
///
/// `rows` is always populated. `csv` / `filename` are present only when
/// `format=csv` is requested — the caller can pass them straight to a download
/// helper, or build its own CSV from `rows`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AudienceResponse {
    /// Number of distinct emails in the result.
    pub total: usize,
    pub rows: Vec<AudienceRow>,
    pub csv: Option<String>,
    pub filename: Option<String>,
}

// ===== Path builder =====

/// Build the `/contacts/audience` path with optional event scoping and format.
///
/// - `event_ids` `None`/empty ⇒ ALL events.
/// - `format_csv` `true` ⇒ attaches `format=csv` (server returns `csv` +
///   `filename` for direct download).
///
/// Event IDs are joined with `,` to match the backend's comma-split parser.
/// IDs are trimmed and empties are dropped so stray commas/spaces are harmless.
fn build_audience_path(event_ids: Option<&[&str]>, format_csv: bool) -> String {
    let mut params: Vec<String> = Vec::new();

    if let Some(ids) = event_ids {
        let joined: String = ids
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<&str>>()
            .join(",");
        if !joined.is_empty() {
            params.push(format!("event_ids={joined}"));
        }
    }

    if format_csv {
        params.push("format=csv".to_string());
    }

    match params.is_empty() {
        true => "/contacts/audience".to_string(),
        false => format!("/contacts/audience?{}", params.join("&")),
    }
}

// ===== Audience API functions =====

/// `GET /api/contacts/audience` — cross-event audience aggregation as JSON rows.
///
/// `event_ids` = `None` or empty ⇒ aggregate across ALL events.
///
/// Results are NOT cached client-side: the aggregation is cross-event and
/// organizer-facing, and a fresh snapshot is cheap and preferable here.
pub async fn get_audience(event_ids: Option<&[&str]>) -> Result<AudienceResponse, ApiError> {
    let path = build_audience_path(event_ids, false);
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Audience request failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let result: ApiResponse<AudienceResponse> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse audience response: {e}"),
            status: 0,
        })?;

    if !result.success {
        return Err(ApiError {
            message: result.error.unwrap_or("Unknown error".to_string()),
            status: 0,
        });
    }

    result.data.ok_or_else(|| ApiError {
        message: "No data in audience response".to_string(),
        status: 0,
    })
}

/// `GET /api/contacts/audience?format=csv` — audience aggregation as a CSV
/// download payload (string + filename).
///
/// `event_ids` = `None` or empty ⇒ aggregate across ALL events.
///
/// Mirrors `export_walkin_csv` — the caller triggers a browser download using
/// the returned `csv` / `filename`. `total` is also returned so the UI can show
/// "Exported N emails" without re-parsing the CSV.
pub async fn export_audience_csv(
    event_ids: Option<&[&str]>,
) -> Result<AudienceResponse, ApiError> {
    let path = build_audience_path(event_ids, true);
    let response = api_get(&path).await?;

    if !response.ok() {
        let body: ApiResponse<()> = response_json(&response).await.unwrap_or(ApiResponse {
            success: false,
            data: None,
            error: Some("Audience CSV export failed".to_string()),
            correlation_id: None,
        });
        return Err(ApiError {
            message: body.error.unwrap_or_default(),
            status: response.status(),
        });
    }

    let result: ApiResponse<AudienceResponse> =
        response_json(&response).await.map_err(|e| ApiError {
            message: format!("Failed to parse audience CSV response: {e}"),
            status: 0,
        })?;

    if !result.success {
        return Err(ApiError {
            message: result.error.unwrap_or("Unknown error".to_string()),
            status: 0,
        });
    }

    result.data.ok_or_else(|| ApiError {
        message: "No data in audience CSV response".to_string(),
        status: 0,
    })
}

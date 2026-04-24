//! Google Sheets API operations for the Cloudflare Worker.
//!
//! Mirrors `src/sheets/client.rs` from the Axum build but uses
//! `worker::Fetch` (via `crate::http`) and SubtleCrypto (via `crate::crypto`)
//! instead of `reqwest` and the `rsa` crate.

use base64::Engine;
use chrono::Utc;

use event_checkin_domain::models::attendee::{Attendee, AttendeeRow};
use event_checkin_domain::models::auth::ServiceAccountClaim;

use crate::crypto;
use crate::http::{
    AccessTokenResponse, BatchUpdateRequest, ValueRange, batch_update_sheet,
    exchange_jwt_assertion, fetch_sheet_range,
};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Access token
// ---------------------------------------------------------------------------

/// Get a Google API access token using service account JWT assertion.
///
/// Builds an RS256-signed JWT, exchanges it for an access token via
/// the Google OAuth2 token endpoint.
pub async fn get_access_token(state: &AppState) -> Result<String, String> {
    let sa = &state.config.service_account;
    let claim = ServiceAccountClaim::new(sa.client_email.clone(), sa.token_uri.clone());

    // Build JWT header + payload (base64url-encoded)
    let header_b64 = base64_url_encode(
        &serde_json::to_vec(&serde_json::json!({"alg": "RS256", "typ": "JWT"}))
            .map_err(|e| format!("failed to encode jwt header: {e}"))?,
    );
    let payload_b64 = base64_url_encode(
        &serde_json::to_vec(&claim).map_err(|e| format!("failed to encode jwt payload: {e}"))?,
    );

    // Sign with RSA-SHA256 via SubtleCrypto
    let jwt_assertion =
        crypto::sign_jwt_assertion(&header_b64, &payload_b64, &sa.private_key).await?;

    // Exchange the signed JWT for an access token
    let token_response: AccessTokenResponse =
        exchange_jwt_assertion(&sa.token_uri, &jwt_assertion).await?;

    tracing::debug!(
        "obtained google api access token, expires in {}s",
        token_response.expires_in
    );

    Ok(token_response.access_token)
}

// ---------------------------------------------------------------------------
// Attendee queries
// ---------------------------------------------------------------------------

/// Fetch all attendees from the Google Sheet.
/// Returns a list of typed Attendee structs parsed from sheet rows.
pub async fn get_attendees(state: &AppState) -> Result<Vec<Attendee>, String> {
    let access_token = get_access_token(state).await?;
    let range = format!("{}!A2:Z", state.config.sheets.sheet_name);
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}",
        state.config.sheets.sheet_id,
        urlencoding::encode(&range)
    );

    let value_range: ValueRange = fetch_sheet_range(&url, &access_token).await?;

    let attendees: Vec<Attendee> = value_range
        .values
        .iter()
        .enumerate()
        .filter(|(_, row)| !row.is_empty())
        .filter(|(_, row)| row.first().is_some_and(|v| !v.trim().is_empty()))
        .filter_map(|(idx, _)| {
            // row_index is 1-based in the sheet, +2 because row 1 is header and idx is 0-based
            let row_index = idx + 2;
            AttendeeRow::from_sheet_values(&value_range.values, row_index)
        })
        .map(|row| row.to_attendee())
        .collect();

    tracing::info!("fetched {} attendees from google sheets", attendees.len());
    Ok(attendees)
}

/// Get a single attendee by their api_id.
pub async fn get_attendee_by_id(
    api_id: &str,
    state: &AppState,
) -> Result<Option<Attendee>, String> {
    let attendees: Vec<Attendee> = get_attendees(state).await?;
    Ok(attendees.into_iter().find(|a| a.api_id == api_id))
}

/// Find an attendee by their claim token (column L).
/// Scans all attendees and returns the first matching claim_token.
/// Returns `None` if no attendee has the given token.
pub async fn get_attendee_by_claim_token(
    claim_token: &str,
    state: &AppState,
) -> Result<Option<Attendee>, String> {
    let attendees: Vec<Attendee> = get_attendees(state).await?;
    Ok(attendees
        .into_iter()
        .find(|a| a.claim_token.as_deref() == Some(claim_token)))
}

// ---------------------------------------------------------------------------
// Staff queries
// ---------------------------------------------------------------------------

/// Fetch staff email addresses from the dedicated "staff" sheet tab.
/// Reads column A starting from row 2 (row 1 is header).
/// Returns lowercased, trimmed, non-empty email strings.
pub async fn get_staff_emails(state: &AppState) -> Result<Vec<String>, String> {
    let access_token = get_access_token(state).await?;
    let range = format!("{}!A2:A", state.config.sheets.staff_sheet_name);
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}",
        state.config.sheets.sheet_id,
        urlencoding::encode(&range)
    );

    let value_range: ValueRange = fetch_sheet_range(&url, &access_token).await?;

    let emails: Vec<String> = value_range
        .values
        .iter()
        .filter_map(|row| row.first().cloned())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    tracing::debug!("fetched {} staff emails from sheet", emails.len());
    Ok(emails)
}

// ---------------------------------------------------------------------------
// Sheet mutations
// ---------------------------------------------------------------------------

/// Mark an attendee as checked in by updating:
/// - Column I: checked_in_at timestamp (ISO 8601)
/// - Column J: checked_in_by staff email
/// - Column R: claim_token (UUID v7 for NFT/refund claim link)
///
/// Uses batch update to write all columns in a single API call.
pub async fn mark_checked_in(
    row_index: usize,
    staff_email: &str,
    claim_token: &str,
    state: &AppState,
) -> Result<String, String> {
    let access_token = get_access_token(state).await?;
    let timestamp = Utc::now().to_rfc3339();
    let sheet_name = &state.config.sheets.sheet_name;

    let data = vec![
        ValueRange {
            range: format!("{sheet_name}!I{row_index}"),
            values: vec![vec![timestamp.clone()]],
        },
        ValueRange {
            range: format!("{sheet_name}!J{row_index}"),
            values: vec![vec![staff_email.to_string()]],
        },
        ValueRange {
            range: format!("{sheet_name}!R{row_index}"),
            values: vec![vec![claim_token.to_string()]],
        },
    ];

    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values:batchUpdate",
        state.config.sheets.sheet_id
    );

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    tracing::info!(
        "marked row {row_index} as checked in at {timestamp} by {staff_email} claim_token={claim_token}"
    );
    Ok(timestamp)
}

/// Mark an attendee as claimed by writing wallet to column P and claimed_at to column S.
/// Called after a successful cNFT mint to persist the claim on the Google Sheet.
pub async fn mark_claimed(
    row_index: usize,
    wallet_address: &str,
    claimed_at: &str,
    state: &AppState,
) -> Result<String, String> {
    let access_token = get_access_token(state).await?;
    let sheet_name = &state.config.sheets.sheet_name;

    let data = vec![
        ValueRange {
            range: format!("{sheet_name}!P{row_index}"),
            values: vec![vec![wallet_address.to_string()]],
        },
        ValueRange {
            range: format!("{sheet_name}!S{row_index}"),
            values: vec![vec![claimed_at.to_string()]],
        },
    ];

    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values:batchUpdate",
        state.config.sheets.sheet_id
    );

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    tracing::info!("marked row {row_index} as claimed at {claimed_at} wallet={wallet_address}");
    Ok(claimed_at.to_string())
}

/// Bulk update QR code URLs for approved attendees.
/// Updates column Q (qr_code_url) for each attendee.
pub async fn update_qr_urls(
    updates: &[(usize, String)],
    state: &AppState,
) -> Result<usize, String> {
    if updates.is_empty() {
        return Ok(0);
    }

    let access_token = get_access_token(state).await?;
    let sheet_name = &state.config.sheets.sheet_name;

    // Build batch update with individual value ranges
    let data: Vec<ValueRange> = updates
        .iter()
        .map(|(row_index, url)| ValueRange {
            range: format!("{sheet_name}!Q{row_index}"),
            values: vec![vec![url.clone()]],
        })
        .collect();

    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values:batchUpdate",
        state.config.sheets.sheet_id
    );

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, &access_token).await?;

    let updated = updates.len();
    tracing::info!("updated {updated} qr code urls in google sheets");
    Ok(updated)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// URL-safe Base64 encoding (no padding).
fn base64_url_encode(data: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_url_encode() {
        let input = b"hello world";
        let encoded = base64_url_encode(input);
        assert_eq!(encoded, "aGVsbG8gd29ybGQ");
    }
}

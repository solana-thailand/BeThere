use base64::Engine;
use chrono::Utc;
use rsa::RsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::sha2::Sha256;
use rsa::signature::Signer;
use serde::{Deserialize, Serialize};

use crate::config::AppState;
use crate::models::attendee::{Attendee, AttendeeRow};
use crate::models::auth::ServiceAccountClaim;

/// Google API access token response from service account assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccessTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
}

/// Google Sheets API batch update request body.
#[derive(Debug, Serialize)]
struct BatchUpdateRequest {
    data: Vec<ValueRange>,
    value_input_option: String,
}

/// Google Sheets API value range for read/write.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ValueRange {
    range: String,
    values: Vec<Vec<String>>,
}

/// Get a Google API access token using service account JWT assertion.
/// This uses the RS256 signed JWT to authenticate without user interaction.
pub async fn get_access_token(state: &AppState) -> Result<String, String> {
    let sa = &state.config.service_account;
    let claim = ServiceAccountClaim::new(sa.client_email.clone(), sa.token_uri.clone());

    // Build the JWT header and payload
    let header = base64_url_encode(
        &serde_json::to_vec(&serde_json::json!({"alg": "RS256", "typ": "JWT"}))
            .map_err(|e| format!("failed to encode jwt header: {e}"))?,
    );
    let payload = base64_url_encode(
        &serde_json::to_vec(&claim).map_err(|e| format!("failed to encode jwt payload: {e}"))?,
    );
    let sign_input = format!("{header}.{payload}");

    // Sign with RSA private key
    let private_key = parse_rsa_private_key(&sa.private_key)?;
    let signing_key = SigningKey::<Sha256>::new(private_key);
    let signature = signing_key.sign(sign_input.as_bytes());
    let sig_bytes: Box<[u8]> = Box::from(signature);
    let jwt = format!("{sign_input}.{}", base64_url_encode(&sig_bytes));

    // Exchange the signed JWT for an access token
    let params = [
        ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
        ("assertion", &jwt),
    ];

    let response = state
        .http_client
        .post(&sa.token_uri)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("failed to request access token: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("access token request failed ({status}): {body}"));
    }

    let token_response: AccessTokenResponse = response
        .json()
        .await
        .map_err(|e| format!("failed to parse access token response: {e}"))?;

    tracing::debug!(
        "obtained google api access token, expires in {}s",
        token_response.expires_in
    );

    Ok(token_response.access_token)
}

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

    let response = state
        .http_client
        .get(&url)
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|e| format!("failed to fetch sheet data: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("sheets api request failed ({status}): {body}"));
    }

    let value_range: ValueRange = response
        .json()
        .await
        .map_err(|e| format!("failed to parse sheet data: {e}"))?;

    let attendees: Vec<Attendee> = value_range
        .values
        .iter()
        .enumerate()
        .filter(|(_, row): &(usize, &Vec<String>)| !row.is_empty())
        .filter(|(_, row): &(usize, &Vec<String>)| {
            row.first().is_some_and(|v: &String| !v.trim().is_empty())
        })
        .filter_map(|(idx, _): (usize, &Vec<String>)| {
            // row_index is 1-based in the sheet, +2 because row 1 is header and idx is 0-based
            let row_index = idx + 2;
            AttendeeRow::from_sheet_values(&value_range.values, row_index)
        })
        .map(|row: AttendeeRow| row.to_attendee())
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
    Ok(attendees
        .into_iter()
        .find(|a: &Attendee| a.api_id == api_id))
}

/// Fetch staff email addresses from the staff sheet tab.
/// Reads column A starting from row 2, trims, lowercases, and filters empty values.
pub async fn get_staff_emails(state: &AppState) -> Result<Vec<String>, String> {
    let access_token = get_access_token(state).await?;
    let range = format!("{}!A2:A", state.config.sheets.staff_sheet_name);
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}",
        state.config.sheets.sheet_id,
        urlencoding::encode(&range)
    );

    let response = state
        .http_client
        .get(&url)
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|e| format!("failed to fetch staff emails: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("staff emails request failed ({status}): {body}"));
    }

    let value_range: ValueRange = response
        .json()
        .await
        .map_err(|e| format!("failed to parse staff emails: {e}"))?;

    let emails: Vec<String> = value_range
        .values
        .into_iter()
        .filter_map(|row| row.into_iter().next())
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .collect();

    tracing::info!("fetched {} staff emails from google sheets", emails.len());
    Ok(emails)
}

/// Mark an attendee as checked in by updating columns I (timestamp) and J (staff email).
/// Uses batch update to write both values in a single request.
pub async fn mark_checked_in(
    row_index: usize,
    staff_email: &str,
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
    ];

    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values:batchUpdate",
        state.config.sheets.sheet_id
    );

    let body = BatchUpdateRequest {
        data,
        value_input_option: "USER_ENTERED".to_string(),
    };

    let response = state
        .http_client
        .post(&url)
        .header("Authorization", format!("Bearer {access_token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("failed to update sheet: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("check-in update failed ({status}): {body}"));
    }

    tracing::info!("marked row {row_index} as checked in at {timestamp} by {staff_email}");
    Ok(timestamp)
}

/// Bulk update QR code URLs for approved attendees.
/// Updates column K (qr_code_url) for each attendee.
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
        .map(|(row_index, url): &(usize, String)| ValueRange {
            range: format!("{sheet_name}!K{row_index}"),
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

    let response = state
        .http_client
        .post(&url)
        .header("Authorization", format!("Bearer {access_token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("failed to batch update qr urls: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("batch update failed ({status}): {body}"));
    }

    let updated = updates.len();
    tracing::info!("updated {updated} qr code urls in google sheets");
    Ok(updated)
}

/// Parse a PEM-encoded RSA private key from the Google service account JSON.
fn parse_rsa_private_key(pem_str: &str) -> Result<RsaPrivateKey, String> {
    // Handle escaped newlines in env var
    let normalized = pem_str.replace("\\n", "\n").replace("\\r", "\r");

    let pem_content = pem::parse(&normalized).map_err(|e| format!("failed to parse pem: {e}"))?;

    rsa::pkcs8::DecodePrivateKey::from_pkcs8_der(pem_content.contents())
        .map_err(|e| format!("failed to parse rsa private key: {e}"))
}

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

    #[test]
    fn test_parse_rsa_private_key_invalid() {
        let result = parse_rsa_private_key("not a valid pem");
        assert!(result.is_err());
    }

    #[test]
    fn test_value_range_serialization() {
        let vr = ValueRange {
            range: "Sheet1!I2".to_string(),
            values: vec![vec!["2025-01-01T00:00:00Z".to_string()]],
        };
        let json = serde_json::to_string(&vr).unwrap();
        assert!(json.contains("\"range\":\"Sheet1!I2\""));
        assert!(json.contains("\"2025-01-01T00:00:00Z\""));
    }

    #[test]
    fn test_batch_update_request_serialization() {
        let req = BatchUpdateRequest {
            data: vec![ValueRange {
                range: "Sheet1!K2".to_string(),
                values: vec![vec!["https://example.com/qr".to_string()]],
            }],
            value_input_option: "USER_ENTERED".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("USER_ENTERED"));
        assert!(json.contains("Sheet1!K2"));
    }
}

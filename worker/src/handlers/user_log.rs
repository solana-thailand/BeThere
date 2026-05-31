//! User sign-in logging to Google Sheets.
//!
//! Records every sign-in to a "users" sheet tab for analytics.
//! Columns: A = email, B = google_id, C = role, D = first_seen, E = last_seen.
//! If the email already exists, updates role and last_seen in place.
//!
//! The "users" tab is auto-created with headers if it doesn't exist.

use crate::http::{ValueRange, batch_update_sheet, fetch_sheet_range, post_json, put_json};
use crate::sheets::get_access_token;
use crate::state::AppState;

/// Sheet tab name for user sign-in logs.
const SHEET_NAME: &str = "users";
/// Header row for the users sheet.
const HEADER_ROW: [&str; 5] = ["email", "google_id", "role", "first_seen", "last_seen"];

/// Upsert a user sign-in record into the "users" Google Sheet tab.
///
/// - If the email is new, appends a full row: `[email, google_id, role, first_seen, last_seen]`.
/// - If the email already exists, updates columns C (role) and E (last_seen) in the matching row.
/// - If the "users" tab doesn't exist, auto-creates it with headers and retries.
///
/// Errors are logged but **do not block** the sign-in flow.
#[worker::send]
pub async fn upsert_user_log(
    email: &str,
    google_id: &str,
    role: &str,
    state: &AppState,
) -> Result<(), String> {
    let access_token = get_access_token(state).await.map_err(|e| {
        tracing::error!(error = %e, "user_log: failed to get access token");
        e
    })?;
    let sheet_id = resolve_platform_sheet_id(state);
    tracing::info!(sheet_id = %sheet_id, "user_log: resolved platform sheet");
    let email_lower = email.trim().to_lowercase();

    // Fetch existing emails from column A (starting at row 2 to skip header)
    let range = format!("{SHEET_NAME}!A2:A");
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
        urlencoding::encode(&range)
    );

    let value_range: ValueRange = match fetch_sheet_range(&url, &access_token).await {
        Ok(vr) => vr,
        Err(e) => {
            // If the "users" tab doesn't exist, Google returns HTTP 400
            // with "Unable to parse range: users!..." — auto-create and retry.
            if e.contains("Unable to parse range") || e.contains("Unable to find sheet") {
                tracing::info!(%sheet_id, "user_log: 'users' tab not found, creating it");
                create_users_tab(&sheet_id, &access_token).await?;

                // Retry fetch after tab creation
                fetch_sheet_range(&url, &access_token).await.map_err(|e2| {
                    tracing::error!(error = %e2, %sheet_id, "user_log: still failed after creating tab");
                    e2
                })?
            } else {
                tracing::error!(error = %e, %sheet_id, "user_log: failed to fetch existing users");
                return Err(e);
            }
        }
    };

    // Find the matching row index (0-based within the fetched range, so row 0 = sheet row 2)
    let matching_row = value_range.values.iter().position(|row| {
        row.first()
            .map(|v| v.trim().to_lowercase() == email_lower)
            .unwrap_or(false)
    });

    let now = chrono::Utc::now().to_rfc3339();

    if let Some(idx) = matching_row {
        // Row exists — update columns C (role) and E (last_seen).
        // Sheet row number = idx + 2 (header is row 1, data starts at row 2).
        let sheet_row = idx + 2;
        update_existing_user(&sheet_id, sheet_row, role, &now, &access_token).await?;
        tracing::info!(
            staff_email = %email_lower,
            sheet_row,
            %role,
            "updated existing user log row"
        );
    } else {
        // New user — append a full row
        append_new_user(
            &sheet_id,
            &email_lower,
            google_id,
            role,
            &now,
            &access_token,
        )
        .await?;
        tracing::info!(
            staff_email = %email_lower,
            %role,
            "appended new user log row"
        );
    }

    Ok(())
}

/// Create the "users" sheet tab with a header row.
async fn create_users_tab(sheet_id: &str, access_token: &str) -> Result<(), String> {
    // Add a new sheet tab via the Sheets API batchUpdate (spreadsheet-level, not values).
    let url = format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}:batchUpdate");

    let body = serde_json::json!({
        "requests": [{
            "addSheet": {
                "properties": {
                    "title": SHEET_NAME
                }
            }
        }]
    });

    post_json::<serde_json::Value>(&url, &body, Some(access_token))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, %sheet_id, "user_log: failed to create 'users' tab");
            format!("failed to create 'users' tab: {e}")
        })?;

    tracing::info!(%sheet_id, "user_log: created 'users' tab");

    // Write header row
    let header_body = ValueRange {
        range: format!("{SHEET_NAME}!A1:E1"),
        values: vec![HEADER_ROW.iter().map(|s| s.to_string()).collect()],
    };

    let header_url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
        urlencoding::encode(&format!("{SHEET_NAME}!A1:E1")),
    );

    put_json(&header_url, &header_body, access_token)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, %sheet_id, "user_log: failed to write header row");
            format!("failed to write header row: {e}")
        })?;

    tracing::info!(%sheet_id, "user_log: wrote header row to 'users' tab");
    Ok(())
}

/// Update role (column C) and last_seen (column E) for an existing row.
async fn update_existing_user(
    sheet_id: &str,
    sheet_row: usize,
    role: &str,
    last_seen: &str,
    access_token: &str,
) -> Result<(), String> {
    use crate::http::BatchUpdateRequest;

    let url =
        format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values:batchUpdate");

    let body = BatchUpdateRequest {
        data: vec![
            ValueRange {
                range: format!("{SHEET_NAME}!C{sheet_row}"),
                values: vec![vec![role.to_string()]],
            },
            ValueRange {
                range: format!("{SHEET_NAME}!E{sheet_row}"),
                values: vec![vec![last_seen.to_string()]],
            },
        ],
        value_input_option: "USER_ENTERED".to_string(),
    };

    batch_update_sheet(&url, &body, access_token).await
}

/// Append a new user row: [email, google_id, role, first_seen, last_seen].
async fn append_new_user(
    sheet_id: &str,
    email: &str,
    google_id: &str,
    role: &str,
    timestamp: &str,
    access_token: &str,
) -> Result<(), String> {
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{SHEET_NAME}!A:E:append?valueInputOption=USER_ENTERED&insertDataOption=INSERT_ROWS"
    );

    let body = ValueRange {
        range: format!("{SHEET_NAME}!A:E"),
        values: vec![vec![
            email.to_string(),
            google_id.to_string(),
            role.to_string(),
            timestamp.to_string(), // first_seen
            timestamp.to_string(), // last_seen
        ]],
    };

    post_json::<serde_json::Value>(&url, &body, Some(access_token))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, %sheet_id, "user_log: failed to append new user");
            e
        })?;

    Ok(())
}

/// Resolve the platform sheet ID, falling back to the primary event sheet
/// if `platform_sheet_id` is not configured.
pub fn resolve_platform_sheet_id(state: &AppState) -> String {
    if state.config.sheets.platform_sheet_id.is_empty() {
        state.config.sheets.sheet_id.clone()
    } else {
        state.config.sheets.platform_sheet_id.clone()
    }
}

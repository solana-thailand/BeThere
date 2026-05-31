//! Waitlist handler for the Cloudflare Worker.
//!
//! POST /api/waitlist — public endpoint to join the waitlist.
//! Saves email + timestamp to a dedicated Google Sheets tab.
//! Deduplicates by checking existing emails before appending.

use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::json;

use crate::error::ApiOk;

use event_checkin_domain::models::error::AppError;

use crate::http::{ValueRange, fetch_sheet_range, post_json};
use crate::sheets::get_access_token;
use crate::state::AppState;

/// Request body for waitlist signup.
#[derive(Debug, Clone, Deserialize)]
pub struct WaitlistRequest {
    pub email: String,
}

/// POST /api/waitlist
/// Public endpoint — add email to the waitlist Google Sheet tab.
///
/// Validates email format, checks for duplicates, then appends a row
/// to the "waitlist" sheet tab with columns: A = email, B = timestamp (ISO 8601).
#[worker::send]
pub async fn join_waitlist(
    State(state): State<AppState>,
    Json(body): Json<WaitlistRequest>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let email = body.email.trim().to_lowercase();

    // Basic email validation
    if email.is_empty() || !email.contains('@') || !email.contains('.') {
        return Err(AppError::Validation("Invalid email address".into()).into());
    }

    // Check email length
    if email.len() > 254 {
        return Err(AppError::Validation("Email too long".into()).into());
    }

    // Duplicate check — fetch existing emails from the sheet
    match get_existing_waitlist_emails(&state).await {
        Ok(existing) => {
            if existing.contains(&email) {
                tracing::info!(staff_email = %email, "waitlist duplicate");
                return Err(
                    AppError::Validation("This email is already on the waitlist".into()).into(),
                );
            }
        }
        Err(e) => {
            // If the 'waitlist' tab doesn't exist, auto-create it and continue
            if e.contains("Unable to parse range") || e.contains("Unable to find sheet") {
                tracing::info!("waitlist: tab not found, auto-creating");
                if let Err(create_err) = create_waitlist_tab(&state).await {
                    tracing::warn!(error = %create_err, "waitlist: failed to auto-create tab");
                }
            } else {
                tracing::warn!(error = ?e, "could not fetch existing waitlist emails for dedup");
            }
        }
    }

    tracing::info!(staff_email = %email, "waitlist signup");

    // Append to Google Sheet
    append_to_waitlist(&email, &state).await.map_err(|e| {
        tracing::error!(staff_email = %email, error = ?e, "waitlist signup failed");
        AppError::Internal(format!("Failed to join waitlist: {e}"))
    })?;

    let data = json!({ "email": email });
    Ok(ApiOk::new(data))
}

/// Fetch all existing emails from the "waitlist" sheet tab (column A).
/// Returns a Vec of lowercased email strings for dedup comparison.
async fn get_existing_waitlist_emails(state: &AppState) -> Result<Vec<String>, String> {
    let access_token = get_access_token(state).await?;
    let sheet_id = crate::handlers::user_log::resolve_platform_sheet_id(state);
    let sheet_name = "waitlist";
    let range = format!("{sheet_name}!A2:A");
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
        urlencoding::encode(&range)
    );

    let value_range: ValueRange = fetch_sheet_range(&url, &access_token).await?;

    let emails: Vec<String> = value_range
        .values
        .iter()
        .filter_map(|row| {
            row.first()
                .map(|v| v.trim().to_lowercase())
                .filter(|v| !v.is_empty())
        })
        .collect();

    tracing::debug!(
        total_fetched = emails.len(),
        "fetched existing waitlist emails for dedup"
    );
    Ok(emails)
}

/// Append an email to the waitlist Google Sheet tab.
///
/// Uses the append API to add a new row at the bottom of the sheet.
/// Columns: A = email, B = signed_up_at (ISO 8601 timestamp).
async fn append_to_waitlist(email: &str, state: &AppState) -> Result<(), String> {
    let access_token = get_access_token(state).await?;
    let timestamp = chrono::Utc::now().to_rfc3339();
    let sheet_id = crate::handlers::user_log::resolve_platform_sheet_id(state);
    let sheet_name = "waitlist"; // Dedicated tab name

    // Use Google Sheets append API
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{sheet_name}!A:B:append?valueInputOption=USER_ENTERED&insertDataOption=INSERT_ROWS"
    );

    let body = ValueRange {
        range: format!("{sheet_name}!A:B"),
        values: vec![vec![email.to_string(), timestamp]],
    };

    // POST as JSON with auth
    post_json::<serde_json::Value>(&url, &body, Some(&access_token)).await?;

    Ok(())
}

/// Create the "waitlist" sheet tab with a header row.
async fn create_waitlist_tab(state: &AppState) -> Result<(), String> {
    use crate::http::put_json;

    let access_token = get_access_token(state).await?;
    let sheet_id = crate::handlers::user_log::resolve_platform_sheet_id(state);

    // Add a new sheet tab via the Sheets API batchUpdate
    let url = format!("https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}:batchUpdate");

    let body = serde_json::json!({
        "requests": [{
            "addSheet": {
                "properties": {
                    "title": "waitlist"
                }
            }
        }]
    });

    post_json::<serde_json::Value>(&url, &body, Some(&access_token)).await?;

    // Write header row
    let header_body = ValueRange {
        range: "waitlist!A1:B1".to_string(),
        values: vec![vec!["email".to_string(), "signed_up_at".to_string()]],
    };

    let header_url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{sheet_id}/values/{}",
        urlencoding::encode("waitlist!A1:B1"),
    );

    put_json(&header_url, &header_body, &access_token).await?;

    tracing::info!(%sheet_id, "waitlist: created tab with header row");
    Ok(())
}

//! Waitlist handler for the Cloudflare Worker.
//!
//! POST /api/waitlist — public endpoint to join the waitlist.
//! Saves email + timestamp to a dedicated Google Sheets tab.
//! Deduplicates by checking existing emails before appending.

use axum::{extract::State, response::Json};
use serde::Deserialize;
use serde_json::json;

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
) -> Json<serde_json::Value> {
    let email = body.email.trim().to_lowercase();

    // Basic email validation
    if email.is_empty() || !email.contains('@') || !email.contains('.') {
        return Json(json!({
            "success": false,
            "error": "Invalid email address",
        }));
    }

    // Check email length
    if email.len() > 254 {
        return Json(json!({
            "success": false,
            "error": "Email too long",
        }));
    }

    // Duplicate check — fetch existing emails from the sheet
    match get_existing_waitlist_emails(&state).await {
        Ok(existing) => {
            if existing.contains(&email) {
                tracing::info!("waitlist duplicate: {email}");
                return Json(json!({
                    "success": false,
                    "error": "This email is already on the waitlist",
                    "code": "duplicate",
                }));
            }
        }
        Err(e) => {
            // Log but don't block — if we can't read, still allow signup
            tracing::warn!("could not fetch existing waitlist emails for dedup: {e}");
        }
    }

    tracing::info!("waitlist signup: {email}");

    // Append to Google Sheet
    match append_to_waitlist(&email, &state).await {
        Ok(()) => Json(json!({
            "success": true,
            "data": { "email": email },
        })),
        Err(e) => {
            tracing::error!("waitlist signup failed for {email}: {e}");
            Json(json!({
                "success": false,
                "error": format!("Failed to join waitlist: {e}"),
            }))
        }
    }
}

/// Fetch all existing emails from the "waitlist" sheet tab (column A).
/// Returns a Vec of lowercased email strings for dedup comparison.
async fn get_existing_waitlist_emails(state: &AppState) -> Result<Vec<String>, String> {
    let access_token = get_access_token(state).await?;
    let sheet_id = &state.config.sheets.sheet_id;
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
        "fetched {} existing waitlist emails for dedup",
        emails.len()
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
    let sheet_id = &state.config.sheets.sheet_id;
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

use axum::{Extension, extract::State, response::Json};
use serde_json::json;

use crate::config::AppState;
use crate::models::api::{GenerateQrResponse, QrGenerationDetail, QrGenerationStatus};
use crate::models::auth::Claims;
use crate::{qr, sheets};

/// POST /api/generate-qrs
/// Bulk generate QR code URLs for approved attendees.
///
/// This endpoint:
/// 1. Fetches all attendees from Google Sheets
/// 2. Generates check-in URLs for approved attendees without existing QR URLs
/// 3. Batch updates the `qr_code_url` column (column K) in Google Sheets
/// 4. Returns a summary of generated/skipped QR codes
///
/// Optional query parameter `force=true` to regenerate all QR URLs.
pub async fn generate_qrs(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Json<serde_json::Value> {
    tracing::info!("QR generation requested by {}", claims.email);

    // Fetch all attendees
    let attendees = match sheets::get_attendees(&state).await {
        Ok(a) => a,
        Err(ref e) => {
            tracing::error!("failed to fetch attendees for QR generation: {e}");
            return Json(json!({
                "success": false,
                "error": format!("failed to fetch attendees: {e}"),
            }));
        }
    };

    let total_approved: usize = attendees.iter().filter(|a| a.is_approved()).count();

    // Generate QR URLs for attendees that don't have one
    let updates = qr::generate_qr_urls(&attendees, &state.config.server_url, false);

    if updates.is_empty() {
        tracing::info!("no QR codes to generate, all approved attendees already have URLs");
        return Json(json!({
            "success": true,
            "data": GenerateQrResponse {
                total: total_approved,
                generated: 0,
                skipped: total_approved,
                details: vec![],
            },
        }));
    }

    // Build details for response before updating
    let details: Vec<QrGenerationDetail> = updates
        .iter()
        .filter_map(|(row_idx, url)| {
            attendees
                .iter()
                .find(|a| a.row_index == *row_idx)
                .map(|a| QrGenerationDetail {
                    api_id: a.api_id.clone(),
                    name: a.display_name().to_string(),
                    qr_code_url: url.clone(),
                    status: QrGenerationStatus::Generated,
                })
        })
        .collect();

    // Batch update the Google Sheet
    match sheets::update_qr_urls(&updates, &state).await {
        Ok(updated) => {
            tracing::info!(
                "QR generation complete: {updated} URLs written to sheet (requested by: {})",
                claims.email
            );

            // Add skipped attendees to details
            let mut all_details = details;

            let updated_rows: Vec<usize> = updates.iter().map(|(row, _)| *row).collect();

            let skipped_details: Vec<QrGenerationDetail> = attendees
                .iter()
                .filter(|a| a.is_approved() && !updated_rows.contains(&a.row_index))
                .map(|a| QrGenerationDetail {
                    api_id: a.api_id.clone(),
                    name: a.display_name().to_string(),
                    qr_code_url: a.qr_code_url.clone().unwrap_or_default(),
                    status: QrGenerationStatus::Skipped,
                })
                .collect();

            all_details.extend(skipped_details);

            let skipped: usize = total_approved.saturating_sub(updated);

            Json(json!({
                "success": true,
                "data": GenerateQrResponse {
                    total: total_approved,
                    generated: updated,
                    skipped,
                    details: all_details,
                },
            }))
        }
        Err(ref e) => {
            tracing::error!("failed to update QR URLs in sheet: {e}");
            Json(json!({
                "success": false,
                "error": format!("failed to write QR URLs to sheet: {e}"),
            }))
        }
    }
}

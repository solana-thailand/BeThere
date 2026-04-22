use axum::{
    Extension,
    extract::{Query, State},
    response::Json,
};
use serde::Deserialize;
use serde_json::json;

use crate::config::AppState;
use crate::models::api::{GenerateQrResponse, QrGenerationDetail, QrGenerationStatus};
use crate::models::auth::Claims;
use crate::{qr, sheets};

#[derive(Debug, Deserialize)]
pub struct GenerateQrQuery {
    /// If true, regenerate QR URLs even for attendees that already have one.
    #[serde(default)]
    pub force: bool,
}

/// POST /api/generate-qrs?force=true
/// Bulk generate QR code URLs for approved attendees.
///
/// This endpoint:
/// 1. Fetches all attendees from Google Sheets
/// 2. Generates check-in URLs for approved attendees without existing QR URLs
/// 3. Batch updates the `qr_code_url` column (column K) in Google Sheets
/// 4. Returns a summary of generated/skipped QR codes with detailed reasons
///
/// Query parameters:
/// - `force`: if true, regenerates QR URLs for all approved attendees (even those with existing URLs)
pub async fn generate_qrs(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<GenerateQrQuery>,
) -> Json<serde_json::Value> {
    let force = query.force;
    tracing::info!(
        "QR generation requested by {} (force={force})",
        claims.email
    );

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

    let total_fetched = attendees.len();

    // Compute approval status distribution for diagnostics
    let mut status_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for a in &attendees {
        let key = a.approval_status.to_string();
        *status_counts.entry(key).or_insert(0) += 1;
    }

    let total_approved: usize = attendees.iter().filter(|a| a.is_approved()).count();

    // Count QR URL states for diagnostics
    let approved_with_qr: Vec<&crate::models::attendee::Attendee> = attendees
        .iter()
        .filter(|a| a.is_approved() && a.qr_code_url.is_some())
        .collect();

    let approved_without_qr: usize = total_approved.saturating_sub(approved_with_qr.len());

    tracing::info!(
        "QR diagnostics: total_fetched={total_fetched}, total_approved={total_approved}, \
         with_existing_qr={}, without_qr={approved_without_qr}, statuses={status_counts:?}",
        approved_with_qr.len()
    );

    // Log sample QR URL values for the first few approved attendees with existing URLs
    for a in approved_with_qr.iter().take(3) {
        match &a.qr_code_url {
            Some(url) => tracing::info!(
                "sample existing QR: api_id={}, row={}, url_len={}, url_preview=\"{}\"",
                a.api_id,
                a.row_index,
                url.len(),
                if url.len() > 80 {
                    format!("{}...", &url[..80])
                } else {
                    url.clone()
                }
            ),
            None => unreachable!(),
        }
    }

    // Log sample attendees without QR
    for a in attendees
        .iter()
        .filter(|a| a.is_approved() && a.qr_code_url.is_none())
        .take(3)
    {
        tracing::info!(
            "sample without QR: api_id={}, row={}, participation_type=\"{}\"",
            a.api_id,
            a.row_index,
            a.participation_type
        );
    }

    // Generate QR URLs using the filter logic
    let updates = qr::generate_qr_urls(&attendees, &state.config.server_url, force);

    tracing::info!(
        "generate_qr_urls: {} updates to write (force={force})",
        updates.len()
    );

    if updates.is_empty() {
        tracing::info!("no QR codes to generate");

        // Build detailed skip reasons for all approved attendees
        let details: Vec<QrGenerationDetail> = attendees
            .iter()
            .filter(|a| a.is_approved())
            .map(|a| {
                let skip_reason = match &a.qr_code_url {
                    Some(url) if !url.is_empty() => {
                        format!("already has QR URL (len={})", url.len())
                    }
                    Some(_) => "has empty QR URL".to_string(),
                    None => "unknown skip reason".to_string(),
                };
                tracing::debug!(
                    "skipped attendee {} (row={}): {skip_reason}",
                    a.api_id,
                    a.row_index
                );
                QrGenerationDetail {
                    api_id: a.api_id.clone(),
                    name: a.display_name().to_string(),
                    qr_code_url: a.qr_code_url.clone().unwrap_or_default(),
                    status: QrGenerationStatus::Skipped,
                }
            })
            .collect();

        return Json(json!({
            "success": true,
            "data": GenerateQrResponse {
                total: total_approved,
                generated: 0,
                skipped: total_approved,
                details,
            },
        }));
    }

    // Build details for attendees that will be generated
    let generated_details: Vec<QrGenerationDetail> = updates
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

            let updated_rows: Vec<usize> = updates.iter().map(|(row, _)| *row).collect();

            // Build skipped details for approved attendees not in the update set
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

            let mut all_details = generated_details;
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

//! QR code generation handler for the Cloudflare Worker.
//!
//! Mirrors `src/handlers/qr.rs` from the Axum build but uses
//! `crate::sheets` (worker::Fetch) and `crate::auth` (SubtleCrypto JWT)
//! instead of `reqwest` + `jsonwebtoken`.

use axum::{
    Extension,
    extract::{Query, State},
};
use serde::Deserialize;

use crate::error::ApiOk;

use event_checkin_domain::models::api::{
    GenerateQrResponse, QrGenerationDetail, QrGenerationStatus,
};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::qr;

use super::ext::{resolve_event_with_access, resolve_kv};
use crate::error::WorkerError;
use crate::sheets;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct GenerateQrQuery {
    /// If true, regenerate QR URLs even for attendees that already have one.
    #[serde(default)]
    pub force: bool,
    /// Optional event ID for multi-event support.
    #[serde(default)]
    pub event_id: Option<String>,
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
#[worker::send]
pub async fn generate_qrs(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<GenerateQrQuery>,
) -> Result<ApiOk<GenerateQrResponse>, WorkerError> {
    let force = query.force;

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;

    tracing::info!(
        staff_email = %claims.email,
        force = force,
        "QR generation requested"
    );

    // Fetch all attendees
    let kv = resolve_kv(&state);
    let attendees = sheets::get_attendees(&state, &event.sheet_id, &event.sheet_name, kv)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "failed to fetch attendees for QR generation");
            AppError::Internal(format!("failed to fetch attendees: {e}"))
        })?;

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
    let approved_with_qr: Vec<_> = attendees
        .iter()
        .filter(|a| a.is_approved() && a.qr_code_url.is_some())
        .collect();

    let approved_without_qr: usize = total_approved.saturating_sub(approved_with_qr.len());

    tracing::info!(
        total_fetched = total_fetched,
        total_approved = total_approved,
        with_existing_qr = approved_with_qr.len(),
        without_qr = approved_without_qr,
        statuses = ?status_counts,
        "QR diagnostics"
    );

    // Log sample QR URL values for the first few approved attendees with existing URLs
    for a in approved_with_qr.iter().take(3) {
        match &a.qr_code_url {
            Some(url) => tracing::info!(
                attendee_id = %a.api_id,
                row_index = a.row_index,
                url_len = url.len(),
                url_preview = %if url.len() > 80 {
                    format!("{}...", &url[..80])
                } else {
                    url.clone()
                },
                "sample existing QR"
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
            attendee_id = %a.api_id,
            row_index = a.row_index,
            participation_type = %a.participation_type,
            "sample without QR"
        );
    }

    // Generate QR URLs using the filter logic
    let updates = qr::generate_qr_urls(&attendees, &state.config.server.url, force);

    tracing::info!(
        total_updates = updates.len(),
        force = force,
        "generate_qr_urls: updates to write"
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
                    attendee_id = %a.api_id,
                    row_index = a.row_index,
                    skip_reason = %skip_reason,
                    "skipped attendee"
                );
                QrGenerationDetail {
                    api_id: a.api_id.clone(),
                    name: a.display_name().to_string(),
                    qr_code_url: a.qr_code_url.clone().unwrap_or_default(),
                    status: QrGenerationStatus::Skipped,
                }
            })
            .collect();

        return Ok(ApiOk::new(GenerateQrResponse {
            total: total_approved,
            generated: 0,
            skipped: total_approved,
            details,
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
    let updated = sheets::update_qr_urls(&updates, &state, &event.sheet_id, &event.sheet_name, kv)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "failed to update QR URLs in sheet");
            AppError::Internal(format!("failed to write QR URLs to sheet: {e}"))
        })?;

    tracing::info!(
        total_updated = updated,
        staff_email = %claims.email,
        "QR generation complete"
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

    Ok(ApiOk::new(GenerateQrResponse {
        total: total_approved,
        generated: updated,
        skipped,
        details: all_details,
    }))
}

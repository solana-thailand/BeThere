mod hold_admin;
mod hold_credit;
mod hold_refund_request;
mod refund;
mod slip_admin_upload;
mod slip_list;
mod slip_upload;
mod slip_verify;

pub use hold_admin::{
    admin_apply_credit_handler, admin_hold_deposit_handler, credit_liability_handler,
    held_list_handler,
};
pub use hold_credit::{credit_balance_handler, hold_deposit_handler};
pub use hold_refund_request::{
    clear_credit_refund_request_handler, credit_refund_request_status_handler,
    credit_refund_requests_handler, request_credit_refund_handler,
};
pub use refund::{batch_thb_refund_handler, mark_manual_refund_handler, mark_refund_handler};
pub use slip_admin_upload::admin_upload_thb_slip_handler;
pub use slip_list::{
    credit_used_handler, pending_thb_slips_handler, refund_queue_handler, refunded_list_handler,
};
pub use slip_upload::upload_thb_slip_handler;
pub use slip_verify::verify_thb_slip_handler;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Resolve attendee display names from Google Sheets for a list of deposits.
/// Returns a map of `attendee_id → display_name`.
/// Silently skips attendees not found — the frontend falls back to showing the raw ID.
pub(crate) async fn resolve_attendee_names(
    state: &crate::state::AppState,
    sheet_id: &str,
    sheet_name: &str,
    deposits: &[event_checkin_domain::models::deposit::ThbDeposit],
) -> std::collections::HashMap<String, String> {
    use crate::handlers::ext::resolve_kv;
    use crate::sheets;

    if deposits.is_empty() {
        return std::collections::HashMap::new();
    }

    let kv = resolve_kv(state);
    match sheets::get_attendees_map(state, sheet_id, sheet_name, kv).await {
        Ok(map) => deposits
            .iter()
            .filter_map(|d| {
                map.get(&d.attendee_id)
                    .map(|a| (d.attendee_id.clone(), a.display_name().to_string()))
            })
            .collect(),
        Err(e) => {
            tracing::warn!(error = %e, "failed to resolve attendee names for deposits");
            std::collections::HashMap::new()
        }
    }
}

/// Migrate any inline base64 `data:` URLs (slip_url, refund_proof_url) to R2
/// and persist the serving path back to D1.
///
/// Legacy rows inserted before R2 integration may still hold multi-MB base64
/// strings in `slip_url` / `refund_proof_url`. When these appear in a list
/// response the JSON payload balloons (e.g. 1.5 MB for a single slip). This
/// helper uploads those images to R2 on first sight and replaces the field
/// with a compact serving path (`/api/storage/slips/...`).
///
/// Idempotent: rows whose URL is already an R2 path are skipped. R2 upload
/// failures are non-fatal — the original `data:` URL is kept in the response.
pub(super) async fn migrate_data_urls(
    state: &crate::state::AppState,
    kv: &worker::KvStore,
    d1: Option<&worker::D1Database>,
    event_id: &str,
    deposits: &mut [event_checkin_domain::models::deposit::ThbDeposit],
) {
    for deposit in deposits.iter_mut() {
        let mut changed = false;

        if let Some(url) = deposit.slip_url.as_ref()
            && url.starts_with("data:")
        {
            let migrated = maybe_upload_to_r2(
                state,
                event_id,
                &deposit.attendee_id,
                url,
                crate::storage::PREFIX_SLIPS,
            )
            .await;
            if !migrated.starts_with("data:") {
                tracing::info!(
                    attendee_id = %deposit.attendee_id,
                    "migrated slip_url data URL to R2"
                );
                deposit.slip_url = Some(migrated);
                changed = true;
            }
        }

        if let Some(url) = deposit.refund_proof_url.as_ref()
            && url.starts_with("data:")
        {
            let migrated = maybe_upload_to_r2(
                state,
                event_id,
                &deposit.attendee_id,
                url,
                crate::storage::PREFIX_REFUNDS,
            )
            .await;
            if !migrated.starts_with("data:") {
                tracing::info!(
                    attendee_id = %deposit.attendee_id,
                    "migrated refund_proof_url data URL to R2"
                );
                deposit.refund_proof_url = Some(migrated);
                changed = true;
            }
        }

        if changed && let Err(e) = crate::event_store::save_thb_deposit(kv, deposit, d1).await {
            tracing::warn!(
                attendee_id = %deposit.attendee_id,
                error = %e,
                "failed to persist data URL migration — will retry on next list call"
            );
        }
    }
}

/// Upload a data URL to R2 if available. Returns the R2 key on success,
/// or the original URL if R2 is not available or it's not a data URL.
pub(super) async fn maybe_upload_to_r2(
    state: &crate::state::AppState,
    event_id: &str,
    attendee_id: &str,
    url: &str,
    prefix: &str,
) -> String {
    // Only process data URLs
    if !url.starts_with("data:") {
        return url.to_string();
    }

    let Some(bucket) = state.r2.as_ref() else {
        tracing::debug!("R2 bucket not available, storing slip URL as-is");
        return url.to_string();
    };

    // Parse data URL: data:<mime>;base64,<data>
    let rest = url.strip_prefix("data:").unwrap_or("");
    let Some((header, data)) = rest.split_once(',') else {
        return url.to_string();
    };

    // Decode base64
    use base64::Engine;
    let bytes = match base64::engine::general_purpose::STANDARD.decode(data.trim()) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "failed to decode base64 data URL, storing as-is");
            return url.to_string();
        }
    };

    // Determine extension from MIME type
    let mime = header.split(';').next().unwrap_or("image/jpeg");
    let ext = match mime {
        "image/png" => "png",
        "image/webp" => "webp",
        _ => "jpg",
    };

    // Upload to R2
    let key = format!("{prefix}{event_id}/{attendee_id}.{ext}");
    match crate::storage::put_bytes(bucket, &key, bytes, mime).await {
        Ok(_) => {
            tracing::info!(key = %key, "uploaded slip image to R2");
            // Return the serving URL path — route format depends on prefix
            match prefix {
                crate::storage::PREFIX_SLIPS => {
                    // /api/storage/slips/{event_id}/{attendee_id} (ext stripped for route)
                    format!("/api/storage/slips/{event_id}/{attendee_id}")
                }
                crate::storage::PREFIX_REFUNDS => {
                    format!("/api/storage/refunds/{event_id}/{attendee_id}")
                }
                _ => format!("/api/storage/{key}"),
            }
        }
        Err(e) => {
            tracing::warn!(key = %key, error = %e, "R2 upload failed, storing data URL as-is");
            url.to_string()
        }
    }
}

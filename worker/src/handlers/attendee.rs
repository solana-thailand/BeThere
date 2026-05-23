//! Attendee handlers for the Cloudflare Worker.
//!
//! Mirrors `src/handlers/attendee.rs` from the Axum build but uses
//! `crate::sheets` (worker::Fetch) and `crate::auth` (SubtleCrypto JWT)
//! instead of `reqwest` + `jsonwebtoken`.

use axum::{
    Extension,
    extract::{Path, Query, State},
};
use serde_json::json;

use worker::KvStore;

use crate::error::ApiOk;
use event_checkin_domain::models::api::{
    AttendeeListItem, AttendeeResponse, RecentCheckIn, StatsResponse,
};
use event_checkin_domain::models::attendee::{Attendee, CheckInStatus, WalkinAttendee};
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use super::ext::{
    AttendeesQuery, EventIdQuery, resolve_event, resolve_event_with_access, resolve_kv,
};
use crate::sheets;
use crate::state::AppState;

/// Convert a walk-in attendee (from KV) into an Attendee (sheet-compatible)
/// so it can be merged into the unified attendee list.
fn walkin_to_attendee(w: &WalkinAttendee, row_index: usize) -> Attendee {
    Attendee {
        api_id: format!("walkin:{}", w.email),
        first_name: String::new(),
        last_name: String::new(),
        name: w.name.clone(),
        email: w.email.clone(),
        ticket_name: "Walk-in".to_string(),
        approval_status: CheckInStatus::CheckedIn,
        participation_type: "In-Person".to_string(),
        registration_date: None,
        phone: w.phone.clone(),
        contact_channel: None,
        contact_handle: None,
        deposit_agreed: None,
        deposit_method: None,
        deposit_amount: None,
        deposit_tx_signature: None,
        deposit_verified: None,
        checked_in_at: Some(w.checked_in_at.clone()),
        checked_in_by: Some(w.checked_in_by.clone()),
        solana_address: w.wallet_address.clone(),
        qr_code_url: None,
        claim_token: Some(w.claim_token.clone()),
        claimed_at: w.claimed_at.clone(),
        bank_account: None,
        bank_name: None,
        account_name: None,
        refund_status: None,
        send_email_status: None,
        row_index,
    }
}

/// GET /api/attendees
/// List attendees with cursor-based pagination and statistics.
///
/// Stats are computed over ALL attendees regardless of pagination.
/// Attendees are sorted by `row_index` ascending for deterministic pagination.
/// Use `cursor` (row_index of last item) and `limit` (page size) for pagination.
#[worker::send]
pub async fn list_attendees(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<AttendeesQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!("listing attendees (requested by: {})", claims.email);

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;

    let kv = resolve_kv(&state);

    // 1. Fetch sheet-based attendees
    let mut attendees = sheets::get_attendees(&state, &event.sheet_id, &event.sheet_name, kv)
        .await
        .map_err(|e| {
            tracing::error!("failed to fetch attendees: {e}");
            AppError::Internal(format!("failed to fetch attendees: {e}"))
        })?;

    // 2. Merge walk-in attendees from KV (only for this event)
    let sheet_len = attendees.len();
    let walkin_attendees = match kv {
        Some(k) => super::walkin::list_walkin_attendees(k, &event.id)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(event_id = %event.id, error = %e, "failed to load walk-in attendees, skipping");
                Vec::new()
            }),
        None => Vec::new(),
    };

    // Deduplicate: skip walk-ins whose email already exists in the sheet
    // (they may have been synced already)
    let sheet_emails: std::collections::HashSet<String> =
        attendees.iter().map(|a| a.email.to_lowercase()).collect();

    for (i, w) in walkin_attendees.iter().enumerate() {
        if sheet_emails.contains(&w.email.to_lowercase()) {
            continue;
        }
        // Assign row_index beyond sheet rows (sheet rows are 1-based)
        let row_index = sheet_len + i + 1;
        attendees.push(walkin_to_attendee(w, row_index));
    }

    let walkin_merged = attendees.len().saturating_sub(sheet_len);
    if walkin_merged > 0 {
        tracing::info!(
            event_id = %event.id,
            sheet_attendees = sheet_len,
            walkin_merged,
            total = attendees.len(),
            "merged walk-in attendees into unified list"
        );
    }

    // Compute statistics over ALL attendees (not paginated)
    let total_approved: usize = attendees.iter().filter(|a| a.is_approved()).count();

    let total_checked_in: usize = attendees.iter().filter(|a| a.is_checked_in()).count();

    let total_remaining: usize = total_approved.saturating_sub(total_checked_in);

    let check_in_percentage: f64 = if total_approved > 0 {
        (total_checked_in as f64 / total_approved as f64) * 100.0
    } else {
        0.0
    };

    let recent_check_ins: Vec<RecentCheckIn> = attendees
        .iter()
        .filter(|a| a.is_checked_in())
        .filter_map(|a| {
            a.checked_in_at.as_ref().map(|ts| RecentCheckIn {
                api_id: a.api_id.clone(),
                name: a.display_name().to_string(),
                checked_in_at: ts.clone(),
                checked_in_by: a.checked_in_by.clone(),
            })
        })
        .collect();

    let stats = StatsResponse {
        total_approved,
        total_checked_in,
        total_remaining,
        check_in_percentage: (check_in_percentage * 100.0).round() / 100.0,
        recent_check_ins,
    };

    // Cursor-based pagination: sort approved attendees by row_index,
    // filter by cursor, then take up to `page_limit`.
    let page_limit = query.limit.unwrap_or(200).min(200);

    let mut approved: Vec<_> = attendees.iter().filter(|a| a.is_approved()).collect();
    approved.sort_by_key(|a| a.row_index);

    let filtered: Vec<_> = match query.cursor {
        Some(cursor) => approved
            .into_iter()
            .filter(|a| a.row_index > cursor)
            .collect(),
        None => approved,
    };

    let has_more = filtered.len() > page_limit;
    let page: Vec<_> = filtered.into_iter().take(page_limit).collect();

    let next_cursor = if has_more {
        page.last().map(|a| a.row_index)
    } else {
        None
    };

    let attendee_responses: Vec<AttendeeListItem> = page
        .iter()
        .map(|a| AttendeeListItem::from_attendee(a))
        .collect();

    let data = json!({
        "attendees": attendee_responses,
        "stats": stats,
        "next_cursor": next_cursor,
        "has_more": has_more,
    });
    Ok(ApiOk::new(data))
}

/// GET /api/attendee/:id
/// Get a single attendee by their api_id.
/// Returns full attendee details including check-in status and QR code URL.
#[worker::send]
pub async fn get_attendee(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!("fetching attendee {id} (requested by: {})", claims.email);

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;

    let kv = resolve_kv(&state);
    let attendee = sheets::get_attendee_by_id(&id, &state, &event.sheet_id, &event.sheet_name, kv)
        .await
        .map_err(|e| {
            tracing::error!("failed to fetch attendee {id}: {e}");
            AppError::Internal(format!("failed to fetch attendee: {e}"))
        })?
        .ok_or_else(|| AppError::NotFound(format!("attendee with id '{id}' not found")))?;

    let response = AttendeeResponse::from_attendee(&attendee);

    // Generate a QR code image (cached in KV) if the attendee has a QR URL
    let qr_image = match attendee.qr_code_url.as_ref() {
        Some(url) => get_cached_qr_image(kv, &attendee.api_id, url).await,
        None => None,
    };

    // Read finalized claim lock KV for claimed attendees to retrieve asset_id / cluster.
    let (claimed_asset_id, cluster) = if attendee.claimed_at.is_some() {
        if let Some(token) = attendee.claim_token.as_ref() {
            let lock_key = crate::claim::claim_lock_key(&event.id, token);
            let lock_data: Option<String> = if let Some(kv_ref) = kv {
                kv_ref.get(&lock_key).text().await.ok().flatten()
            } else {
                None
            };
            if let Some(json_str) = lock_data {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    let cluster_val = if state.config.solana.rpc_url.contains("mainnet") {
                        "mainnet-beta".to_string()
                    } else {
                        "devnet".to_string()
                    };
                    (
                        val.get("asset_id")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        Some(cluster_val),
                    )
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let data = json!({
        "attendee": response,
        "qr_image": qr_image,
        "is_checked_in": attendee.is_checked_in(),
        "is_approved": attendee.is_approved(),
        "is_in_person": attendee.is_in_person(),
        "participation_type": attendee.participation_type,
        "claimed": attendee.claimed_at.is_some(),
        "claimed_asset_id": claimed_asset_id,
        "cluster": cluster,
    });
    Ok(ApiOk::new(data))
}

/// GET /api/public/ticket/:id
/// Public — no auth required. Returns attendee ticket data with QR image.
/// Masks email for privacy (e.g. "j***@example.com").
#[worker::send]
pub async fn get_public_ticket(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(attendee_id = %id, "public ticket requested");

    let event = resolve_event(&state, query.event_id.as_deref()).await?;

    let kv = resolve_kv(&state);
    let attendee = sheets::get_attendee_by_id(&id, &state, &event.sheet_id, &event.sheet_name, kv)
        .await
        .map_err(|e| {
            tracing::error!(attendee_id = %id, error = %e, "failed to fetch attendee for public ticket");
            AppError::Internal(format!("failed to fetch attendee: {e}"))
        })?
        .ok_or_else(|| AppError::NotFound(format!("attendee with id '{id}' not found")))?;

    let mut response = AttendeeResponse::from_attendee(&attendee);

    // Mask email for privacy: "john@example.com" → "j***@example.com"
    response.email = mask_email(&response.email);

    // Generate QR code image (cached in KV) if the attendee has a QR URL
    let qr_image = match attendee.qr_code_url.as_ref() {
        Some(url) => get_cached_qr_image(kv, &attendee.api_id, url).await,
        None => None,
    };

    // Fetch deposit status for context (pending verification, etc.)
    let deposit_status = if let Some(kv) = kv {
        crate::event_store::get_deposit_status(kv, &event.id, &attendee.api_id)
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let deposit_info = deposit_status.as_ref().map(|d| {
        serde_json::json!({
            "method": match d.method {
                event_checkin_domain::models::deposit::DepositMethod::Usdc => "usdc",
                event_checkin_domain::models::deposit::DepositMethod::Thb => "thb",
                event_checkin_domain::models::deposit::DepositMethod::CreditThb => "credit_thb",
                event_checkin_domain::models::deposit::DepositMethod::CreditUsdc => "credit_usdc",
            },
            "verified": d.verified,
            "currency": d.currency,
        })
    });

    // Read finalized claim lock KV for claimed attendees to retrieve asset_id / cluster.
    let (claimed_asset_id, cluster) = if attendee.claimed_at.is_some() {
        if let Some(token) = attendee.claim_token.as_ref() {
            let lock_key = crate::claim::claim_lock_key(&event.id, token);
            let lock_data: Option<String> = if let Some(kv_ref) = kv {
                kv_ref.get(&lock_key).text().await.ok().flatten()
            } else {
                None
            };
            if let Some(json_str) = lock_data {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    let cluster_val = if state.config.solana.rpc_url.contains("mainnet") {
                        "mainnet-beta".to_string()
                    } else {
                        "devnet".to_string()
                    };
                    (
                        val.get("asset_id")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        Some(cluster_val),
                    )
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    let data = json!({
        "attendee": response,
        "qr_image": qr_image,
        "is_checked_in": attendee.is_checked_in(),
        "is_approved": attendee.is_approved(),
        "is_in_person": attendee.is_in_person(),
        "participation_type": attendee.participation_type,
        "deposit_info": deposit_info,
        "event_end_ms": event.event_end_ms,
        "event_name": event.name,
        "event_start_ms": event.event_start_ms,
        "event_format": event.event_format.as_str(),
        "video_url": event.video_url,
        "claimed": attendee.claimed_at.is_some(),
        "claimed_asset_id": claimed_asset_id,
        "cluster": cluster,
    });
    Ok(ApiOk::new(data))
}

/// Mask an email address for privacy: "john@example.com" → "j***@example.com".
fn mask_email(email: &str) -> String {
    let Some((local, domain)) = email.split_once('@') else {
        return "***".to_string();
    };
    if local.is_empty() {
        return format!("***@{domain}");
    }
    let first = local.chars().next().unwrap_or('*');
    format!("{first}***@{domain}")
}

/// QR image cache TTL in seconds (1 hour).
const QR_IMAGE_CACHE_TTL_SECS: u64 = 3600;

/// Generate a QR base64 image, cached in KV.
///
/// Key: `qr:{api_id}`, TTL: 1 hour.
/// Falls back to uncached generation if KV is unavailable.
#[allow(clippy::collapsible_if)]
async fn get_cached_qr_image(
    kv: Option<&KvStore>,
    api_id: &str,
    qr_code_url: &str,
) -> Option<String> {
    let cache_key = format!("qr:{api_id}");

    // Try KV cache first
    if let Some(kv) = kv
        && let Ok(Some(cached)) = kv.get(&cache_key).text().await
    {
        return Some(cached);
    }

    // Generate fresh
    let image = event_checkin_domain::qr::generate_qr_base64(qr_code_url).ok()?;

    // Store in KV (best-effort, don't block on failure)
    if let Some(kv) = kv
        && let Ok(builder) = kv.put(&cache_key, image.clone())
        && let Err(e) = builder
            .expiration_ttl(QR_IMAGE_CACHE_TTL_SECS)
            .execute()
            .await
    {
        tracing::debug!(key = %cache_key, error = %e, "failed to cache QR image in KV");
    }

    Some(image)
}

/// POST /api/admin/flush-cache
/// Flush all server-side caches (attendee list + column mapping) for an event.
/// Use after changing sheet structure or headers.
#[worker::send]
pub async fn flush_cache(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!("flushing caches (requested by: {})", claims.email);

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;
    let kv = resolve_kv(&state);

    sheets::flush_caches(&state, &event.sheet_id, &event.sheet_name, kv).await;

    Ok(ApiOk::new(json!({
        "flushed": true,
        "event_id": event.id,
        "sheet_id": event.sheet_id,
    })))
}

// ---------------------------------------------------------------------------
// DELETE /api/attendee/{id} — delete attendee from system + sheet
// ---------------------------------------------------------------------------

/// Delete attendee request (supports both sheet-based and walk-in attendees).
///
/// Cleans up:
/// - Google Sheet row (regular attendees)
/// - Walk-in KV records (walkin:*, claim_walkin:*, walkin_synced:*)
/// - Deposit status, THB deposit data
/// - Claim locks
/// - QR image cache
/// - Attendee list cache
#[worker::send]
pub async fn delete_attendee(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(attendee_id = %id, staff_email = %claims.email, "delete attendee request");

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;
    let kv = resolve_kv(&state);

    let mut deleted_keys = Vec::new();
    let source;

    // 1. Try to find as walk-in attendee in KV
    let walkin = if let Some(kv) = kv {
        // Walk-in keys are indexed by email, not api_id.
        // We need to scan walkin:{event_id}:* to find by claim_token or name.
        crate::handlers::walkin::find_walkin_by_any(kv, &event.id, &id).await
    } else {
        None
    };

    if let Some(walkin_attendee) = walkin {
        // Walk-in attendee found in KV
        source = "walk-in".to_string();
        let email_lower = walkin_attendee.email.to_lowercase();

        // Delete walk-in record
        let key = format!("walkin:{}:{}", event.id, email_lower);
        if let Some(kv) = kv {
            let _ = kv.delete(&key).await;
            deleted_keys.push(key);
        }

        // Delete reverse mapping
        let rkey = format!("claim_walkin:{}", walkin_attendee.claim_token);
        if let Some(kv) = kv {
            let _ = kv.delete(&rkey).await;
            deleted_keys.push(rkey);
        }

        // Delete sync marker
        let skey = format!("walkin_synced:{}:{}", event.id, email_lower);
        if let Some(kv) = kv {
            let _ = kv.delete(&skey).await;
            deleted_keys.push(skey);
        }

        // Delete claim lock
        let lkey = crate::claim::claim_lock_key(&event.id, &walkin_attendee.claim_token);
        if let Some(kv) = kv {
            let _ = kv.delete(&lkey).await;
            deleted_keys.push(lkey);
        }

        tracing::info!(
            event_id = %event.id,
            email = %email_lower,
            name = %walkin_attendee.name,
            "walk-in attendee deleted from KV"
        );
    } else {
        // 2. Try to find as regular attendee in Google Sheet
        let attendee =
            sheets::get_attendee_by_id(&id, &state, &event.sheet_id, &event.sheet_name, kv)
                .await
                .map_err(|e| AppError::Internal(format!("failed to look up attendee: {e}")))?;

        match attendee {
            Some(attendee) => {
                source = "sheet".to_string();

                // Delete the row from Google Sheet
                let mapping =
                    sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, kv)
                        .await
                        .unwrap_or_else(|_| {
                            event_checkin_domain::models::attendee::ColumnMapping::hardcoded()
                        });

                crate::sheets::write::delete_sheet_row(
                    attendee.row_index,
                    &mapping,
                    &state,
                    &event.sheet_id,
                    &event.sheet_name,
                    kv,
                )
                .await
                .map_err(|e| AppError::Internal(format!("failed to delete sheet row: {e}")))?;

                // Clean up KV keys for this attendee
                if let Some(kv) = kv {
                    // Deposit status
                    let dkey = crate::event_store::deposit_status_key(&event.id, &attendee.api_id);
                    let _ = kv.delete(&dkey).await;
                    deleted_keys.push(dkey);

                    // THB deposit
                    let tkey = crate::event_store::thb_deposit_key(&event.id, &attendee.api_id);
                    let _ = kv.delete(&tkey).await;
                    deleted_keys.push(tkey);

                    // Claim lock (if has claim_token)
                    if let Some(ref token) = attendee.claim_token
                        && !token.is_empty()
                    {
                        let lkey = crate::claim::claim_lock_key(&event.id, token);
                        let _ = kv.delete(&lkey).await;
                        deleted_keys.push(lkey);
                    }

                    // QR image cache
                    let qrkey = format!("qr:{}", attendee.api_id);
                    let _ = kv.delete(&qrkey).await;
                    deleted_keys.push(qrkey);

                    // Flush attendee cache so list refreshes
                    sheets::flush_caches(&state, &event.sheet_id, &event.sheet_name, Some(kv))
                        .await;
                }

                tracing::info!(
                    event_id = %event.id,
                    attendee_id = %attendee.api_id,
                    name = %attendee.display_name(),
                    row_index = attendee.row_index,
                    "sheet attendee deleted"
                );
            }
            None => {
                return Err(AppError::NotFound(format!(
                    "attendee '{id}' not found in walk-in records or event sheet"
                ))
                .into());
            }
        }
    }

    // Audit log
    if let Some(kv) = &state.events_kv {
        let action = if source == "walk-in" {
            crate::audit_store::AuditAction::WalkinDeleted
        } else {
            crate::audit_store::AuditAction::AttendeeDeleted
        };
        let _ = crate::audit_store::append_event_audit(
            kv,
            &event.id,
            crate::audit_store::create_entry(
                &claims.email,
                action,
                &id,
                &format!("attendee deleted (source={source})"),
            ),
        )
        .await;
    }

    Ok(ApiOk::new(json!({
        "deleted": true,
        "attendee_id": id,
        "event_id": event.id,
        "source": source,
        "kv_keys_removed": deleted_keys.len(),
    })))
}

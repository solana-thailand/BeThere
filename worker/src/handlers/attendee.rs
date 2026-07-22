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
use event_checkin_domain::models::attendee::ParticipationType;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use super::ext::{
    AttendeesQuery, EventIdQuery, resolve_event, resolve_event_with_access, resolve_kv,
};
use crate::sheets;
use crate::state::AppState;

// Walk-in attendees are now stored in D1 directly, so no KV→Attendee conversion is needed.

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
    tracing::info!(event_id = %event.id, "STEP 1: event resolved");

    let kv = resolve_kv(&state);
    tracing::info!(
        has_kv = kv.is_some(),
        has_d1 = state.d1.is_some(),
        "STEP 2: bindings"
    );

    // 1. Fetch sheet-based attendees (D1 fallback when Sheets is unavailable/rate-limited)
    tracing::info!("STEP 3: before get_attendees_for_event");
    let attendees = match sheets::get_attendees_for_event(
        &state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
        &event.id,
    )
    .await
    {
        Ok(a) => a,
        Err(sheets_err) => {
            tracing::warn!(error = %sheets_err, "Sheets fetch failed, trying D1 fallback");
            match &state.d1 {
                Some(db) => crate::db::attendees::get_attendees_by_event(db, &event.id)
                    .await
                    .map_err(|d1_err| {
                        tracing::error!(error = %d1_err, "D1 fallback also failed");
                        AppError::Internal(format!("Sheets: {sheets_err} | D1: {d1_err}"))
                    })?,
                None => {
                    return Err(AppError::Internal(format!(
                        "failed to fetch attendees: {sheets_err}"
                    ))
                    .into());
                }
            }
        }
    };
    tracing::info!(count = attendees.len(), "STEP 4: attendees fetched");

    // Walk-in attendees are now stored directly in D1 alongside pre-registered
    // attendees (participation_type='walkin'), so no separate KV merge is needed.

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
    let attendee = match sheets::get_attendee_by_id(
        &id,
        &state,
        &event.sheet_id,
        &event.sheet_name,
        kv,
    )
    .await
    {
        Ok(Some(a)) => a,
        Ok(None) => {
            return Err(AppError::NotFound(format!("attendee with id '{id}' not found")).into());
        }
        Err(e) => {
            // Sheets errors (404, 429, etc.) — attendee can't be found
            tracing::warn!(attendee_id = %id, error = %e, "failed to fetch attendee for public ticket");
            return Err(AppError::NotFound(format!("attendee with id '{id}' not found")).into());
        }
    };

    let mut attendee = attendee;

    let mut response = AttendeeResponse::from_attendee(&attendee);

    // Mask email for privacy: "john@example.com" → "j***@example.com"
    response.email = mask_email(&response.email);

    // Fetch USDC deposit status and THB deposit concurrently (Plan 014 Phase
    // 4.3.2). Both reads depend only on (event.id, attendee.api_id) and are
    // independent of each other — collapsing two sequential D1/KV round-trips
    // into one concurrent step. The plan's original "3 sequential reads" claim
    // was overstated: event→attendee is a dependency chain (attendee needs
    // event.sheet_id), so only these two post-attendee reads are parallelizable.
    let (usdc_status_res, thb_deposit_res) = futures_util::join!(
        crate::event_store::get_deposit_status_with_fallback(
            kv,
            state.d1.as_deref(),
            &event.id,
            &attendee.api_id,
        ),
        crate::event_store::get_thb_deposit_with_fallback(
            kv,
            state.d1.as_deref(),
            &event.id,
            &attendee.api_id,
        ),
    );
    let deposit_status = usdc_status_res.ok().flatten();
    let thb_deposit = thb_deposit_res.ok().flatten();

    // --- Read-path self-heal (USDC only) ---
    // Recover a missing tx_signature from on-chain PDA history and verify the
    // deposit via signer cross-check. This makes the ticket page self-healing:
    // every 10s poll while `!verified` either discovers the signature or
    // verifies it, until `verified == true` and the ticket tier-upgrades to
    // AwaitingCheckIn. Idempotent: returns immediately when already verified.
    // THB deposits are excluded (admin verifies slips manually).
    let deposit_status = match deposit_status {
        Some(s) if s.method == event_checkin_domain::models::deposit::DepositMethod::Usdc => Some(
            crate::handlers::deposit::usdc::recover_and_verify_deposit(&state, &event, s).await,
        ),
        other => other,
    };

    // Lazy QR backfill: ensure the attendee has a valid QR URL pointing at the
    // current deployment. Triggers when:
    //   - qr_url is missing/empty (verified before the D1 QR-write fix), OR
    //   - qr_url points at a different host (e.g. the fallback default
    //     `event-checkin.workers.dev` from a previous deploy where SERVER_URL
    //     wasn't bound).
    // Self-heals affected attendees on first ticket view without requiring a
    // manual re-verify or sheet→D1 sync.
    let expected_qr_url = format!(
        "{}/staff/?scan={}",
        state.config.server.url.trim_end_matches('/'),
        attendee.api_id
    );
    let needs_qr_backfill = attendee.is_approved()
        && attendee.is_in_person()
        && deposit_status.as_ref().is_some_and(|d| d.verified)
        && attendee.qr_code_url.as_deref() != Some(expected_qr_url.as_str());

    if needs_qr_backfill {
        tracing::info!(
            attendee_id = %attendee.api_id,
            old_qr_url = ?attendee.qr_code_url,
            "lazy QR backfill: (re)generating qr_url for verified attendee"
        );
        if let Some(ref d1) = state.d1
            && let Err(e) =
                crate::db::attendees::set_qr_url(d1, &attendee.api_id, &expected_qr_url).await
        {
            tracing::warn!(
                attendee_id = %attendee.api_id,
                error = %e,
                "lazy QR backfill: D1 write failed (non-fatal)"
            );
        }
        attendee.qr_code_url = Some(expected_qr_url.clone());
        response.qr_code_url = Some(expected_qr_url);
    }

    // Generate QR code image (cached in KV) if the attendee has a QR URL
    let qr_image = match attendee.qr_code_url.as_ref() {
        Some(url) => get_cached_qr_image(kv, &attendee.api_id, url).await,
        None => None,
    };

    // thb_deposit was fetched concurrently with deposit_status above (join!).
    let deposit_info = deposit_status.as_ref().map(|d| {
        serde_json::json!({
            // SSOT: domain Display produces the snake_case wire form.
            // Hand-mapping removed (Plan 014 Phase 2.2 R2).
            "method": d.method.to_string(),
            "verified": d.verified,
            "currency": d.currency,
            "refunded": thb_deposit.as_ref().is_some_and(|t| t.refunded),
            "refund_proof_url": thb_deposit.as_ref().and_then(|t| t.refund_proof_url.clone()),
            // Whether the attendee converted this deposit to rolling credit
            // (distinct from `refunded`). Drives the frontend HoldDepositCard's
            // already-held state and prevents re-showing the CTA on reload.
            "held_as_credit": thb_deposit.as_ref().is_some_and(|t| t.held_as_credit),
        })
    });

    // Deposit deadline check for ticket page notice
    let mut deadline_expired = false;
    let mut in_person_available: Option<bool> = None;

    if deposit_status.is_none()
        && event.deposit_deadline_hours.is_some()
        && attendee.is_in_person()
        && let Some(reg_str) = &attendee.registration_date
        && let Ok(reg_time) = chrono::DateTime::parse_from_rfc3339(reg_str)
    {
        let deadline = reg_time.with_timezone(&chrono::Utc)
            + chrono::Duration::hours(i64::from(event.deposit_deadline_hours.unwrap_or(0)));
        if chrono::Utc::now() > deadline {
            deadline_expired = true;
            // Check capacity
            let cap = event.in_person_capacity;
            let available = if let Some(cap) = cap {
                let count = sheets::get_attendees_for_event(
                    &state,
                    &event.sheet_id,
                    &event.sheet_name,
                    kv,
                    &event.id,
                )
                .await
                .map(|a| a.iter().filter(|a| a.is_in_person()).count() as u32)
                .unwrap_or(u32::MAX);
                count < cap
            } else {
                true
            };
            in_person_available = Some(available);
        }
    }

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

    // Rollover target event: find an upcoming event by the same organizer where the
    // attendee can roll over their verified USDC deposit from this past event.
    let now_ms = chrono::Utc::now().timestamp_millis();
    let rollover_target_event = if let Some(kv_ref) = kv {
        let ds = deposit_status.as_ref();
        let is_usdc_verified = ds.is_some_and(|d| {
            d.verified && d.method == event_checkin_domain::models::deposit::DepositMethod::Usdc
        });
        let is_not_refunded = ds.is_some_and(|_| {
            // For USDC, refund is tracked on-chain (escrow indexer). The deposit_status
            // itself doesn't carry a `refunded` flag, so we check thb_deposit as a
            // fallback — USDC deposits won't have a thb_deposit record, so this is None.
            thb_deposit.is_none() || !thb_deposit.as_ref().is_some_and(|t| t.refunded)
        });

        if is_usdc_verified
            && attendee.is_checked_in()
            && is_not_refunded
            && event.event_end_ms < now_ms
        {
            find_rollover_target(kv_ref, &event, &attendee.api_id, now_ms).await
        } else {
            None
        }
    } else {
        None
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
        "event_link": event.link,
        "event_location": event.location,
        "event_tagline": event.tagline,
        "nft_image_url": event.nft_image_url,
        "claimed": attendee.claimed_at.is_some(),
        "claimed_asset_id": claimed_asset_id,
        "cluster": cluster,
        "deposit_enabled": event.deposit_enabled,
        "deposit_deadline_hours": event.deposit_deadline_hours,
        "deposit_amount_usdc": event.deposit_amount_usdc,
        "deposit_amount_thb": event.deposit_amount_thb,
        "deadline_expired": deadline_expired,
        "in_person_available": in_person_available,
        "event_slug": event.slug,
        "event_id": event.id,
        "refund_link": attendee.refund_link,
        "escrow_status": format!("{}", event.escrow_status),
        "rollover_target_event": rollover_target_event,
        "quiz_enabled": event.quiz_enabled,
        "community_links": event.community_links,
        "calendar_subscribe_url": event.calendar_subscribe_url,
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

/// Find an upcoming event by the same organizer that qualifies as a rollover target.
///
/// Criteria:
/// - Same organizer (matched via `organization_id` or `organizer_emails` overlap)
/// - Deposits enabled with escrow initialized/active
/// - Event is upcoming (`event_end_ms > now_ms`)
/// - Same `deposit_amount_usdc`
/// - Attendee does NOT already have a deposit on that event
async fn find_rollover_target(
    kv: &KvStore,
    source_event: &event_checkin_domain::models::event::EventConfig,
    attendee_id: &str,
    now_ms: i64,
) -> Option<serde_json::Value> {
    let all_events = crate::event_store::list_events(kv).await.ok()?;

    for meta in &all_events {
        // Skip same event
        if meta.id == source_event.id {
            continue;
        }

        // Must be upcoming
        if meta.event_end_ms <= now_ms {
            continue;
        }

        // Deposits enabled
        if !meta.deposit_enabled {
            continue;
        }

        // Escrow initialized or active
        if !matches!(
            meta.escrow_status,
            event_checkin_domain::models::event::EscrowStatus::Initialized
        ) {
            continue;
        }

        // Same organizer: match via organization_id or overlapping organizer_emails
        let same_org = !source_event.organization_id.is_empty()
            && source_event.organization_id == meta.organization_id;
        let same_emails = !source_event.organizer_emails.is_empty()
            && source_event
                .organizer_emails
                .iter()
                .any(|e| meta.organizer_emails.contains(e));
        if !same_org && !same_emails {
            continue;
        }

        // Load full config to check deposit_amount_usdc
        let target_config = crate::event_store::get_event_config(kv, &meta.id)
            .await
            .ok()??;

        // Same deposit amount
        if target_config.deposit_amount_usdc != source_event.deposit_amount_usdc {
            continue;
        }

        // Attendee does NOT already have a deposit on this event
        let existing = crate::event_store::get_deposit_status(kv, &meta.id, attendee_id, None)
            .await
            .ok()
            .flatten();
        if existing.is_some() {
            continue;
        }

        // Found a match
        return Some(serde_json::json!({
            "event_id": meta.id,
            "event_name": meta.name,
            "event_slug": meta.slug,
            "deposit_amount_usdc": target_config.deposit_amount_usdc,
        }));
    }

    None
}

/// POST /api/admin/flush-cache
/// Flush server-side column mapping cache for an event.
/// Use after changing sheet structure or headers.
///
/// Phase 2d: attendee list cache removed — only column map cache remains.
#[worker::send]
pub async fn flush_cache(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<EventIdQuery>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!("flushing column map cache (requested by: {})", claims.email);

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;
    let kv = resolve_kv(&state);

    sheets::invalidate_column_map_cache(kv, &event.sheet_id, &event.sheet_name).await;

    Ok(ApiOk::new(json!({
        "flushed": true,
        "event_id": event.id,
        "sheet_id": event.sheet_id,
    })))
}

// ---------------------------------------------------------------------------
// POST /api/admin/repair-claim-tokens — backfill empty D1 claim_tokens
// ---------------------------------------------------------------------------

/// Repair D1 attendees with empty or NULL claim_tokens.
///
/// Generates a UUID v7 for each affected row. Idempotent — rows with valid
/// tokens are untouched. SuperAdmin only.
#[worker::send]
pub async fn repair_claim_tokens(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    tracing::info!(staff_email = %claims.email, "claim token repair requested");

    let role = crate::auth::resolve_user_role(&claims.email, &state, None).await;
    if role != crate::auth::UserRole::SuperAdmin {
        return Err(AppError::Forbidden("only super admins can repair claim tokens".into()).into());
    }

    let db = state
        .d1
        .as_deref()
        .ok_or_else(|| AppError::Internal("D1 database not available".to_string()))?;

    let repaired = crate::db::attendees::repair_empty_claim_tokens(db)
        .await
        .map_err(AppError::Internal)?;

    tracing::info!(repaired, "claim token repair complete");

    Ok(ApiOk::new(json!({
        "repaired": repaired,
    })))
}

// ---------------------------------------------------------------------------
// DELETE /api/attendee/{id} — delete attendee from system + sheet
// ---------------------------------------------------------------------------

/// Delete attendee request (supports both sheet-based and walk-in attendees).
///
/// Cleans up:
/// - D1 attendee record (primary store)
/// - Google Sheet row (async, best-effort)
/// - Claim locks
/// - QR image cache
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

    // 1. Try to find as walk-in attendee in D1 (primary store)
    //    Walk-ins have participation_type='walkin' in D1.
    //    The `id` param may be a claim_token, email, or name.
    let mut walkin: Option<event_checkin_domain::models::attendee::WalkinAttendee> = None;
    if let Some(db) = state.d1.as_deref() {
        // Try claim_token lookup first
        if let Ok(Some(a)) = crate::db::attendees::get_attendee_by_claim_token(db, &id).await
            && a.participation_type == "walkin"
        {
            walkin = Some(event_checkin_domain::models::attendee::WalkinAttendee {
                event_id: event.id.clone(),
                name: a.name.clone(),
                email: a.email.clone(),
                phone: a.phone.clone(),
                claim_token: a.claim_token.clone().unwrap_or_default(),
                checked_in_at: a.checked_in_at.clone().unwrap_or_default(),
                checked_in_by: a.checked_in_by.clone().unwrap_or_default(),
                wallet_address: None,
                claimed_at: a.claimed_at.clone(),
            });
        }
    }

    if let Some(walkin_attendee) = walkin {
        source = "walk-in".to_string();
        let email_lower = walkin_attendee.email.to_lowercase();
        let claim_token = walkin_attendee.claim_token.clone();

        // Delete from D1 (primary)
        if let Some(db) = state.d1.as_deref()
            && let Err(e) = crate::db::attendees::delete_attendee(db, &event.id, &email_lower).await
        {
            tracing::warn!(
                event_id = %event.id,
                email = %email_lower,
                error = %e,
                "D1 walk-in delete failed"
            );
        }

        // Clean up claim lock
        if let Some(kv) = kv {
            let lkey = crate::claim::claim_lock_key(&event.id, &claim_token);
            let _ = kv.delete(&lkey).await;
            deleted_keys.push(lkey);
        }

        tracing::info!(
            event_id = %event.id,
            email = %email_lower,
            name = %walkin_attendee.name,
            "walk-in attendee deleted"
        );
    } else {
        // 2. Try to find as regular attendee.
        //
        //    Lookup order (matches the dashboard's data source so that what the
        //    admin sees is what gets deleted):
        //      a. D1 by api_id  — authoritative record; gives us email,
        //         claim_token, and a (possibly stale) sheet_row_index.
        //      b. Google Sheet   — read fresh to resolve the CURRENT row_index.
        //         Match by api_id first; fall back to email (case-insensitive)
        //         because some sheets don't populate the api_id column, which
        //         would otherwise cause the sheet row to be missed.
        //
        //    Earlier this code only consulted the Sheet (get_attendees_map with
        //    no event_id skips D1). When the Sheet lookup missed (empty api_id
        //    column, shifted rows, transient Sheets API error), the handler
        //    fell into the "d1-orphan" branch and ONLY deleted from D1 — so the
        //    Google Sheet row survived and the attendee reappeared after any
        //    re-sync. We now delete from BOTH stores whenever a row can be
        //    located in either, preferring the Sheet's fresh row_index over
        //    D1's possibly-stale sheet_row_index.

        // (a) D1 authoritative lookup
        let d1_attendee = if let Some(db) = state.d1.as_deref() {
            crate::db::attendees::get_attendee_by_id(db, &id)
                .await
                .map_err(|e| AppError::Internal(format!("D1 lookup for delete: {e}")))?
        } else {
            None
        };

        // (b) Sheet lookup — try api_id, then email
        let sheet_map = sheets::get_attendees_map(&state, &event.sheet_id, &event.sheet_name, kv)
            .await
            .map_err(|e| AppError::Internal(format!("failed to read attendees sheet: {e}")))?;
        let sheet_attendee = sheet_map.get(&id).cloned().or_else(|| {
            // api_id miss — retry by email (lowercased). Email is reliably
            // populated by registration forms even when api_id is blank.
            let email = d1_attendee.as_ref().map(|a| a.email.to_lowercase())?;
            sheet_map
                .values()
                .find(|a| a.email.eq_ignore_ascii_case(&email))
                .cloned()
        });

        if sheet_attendee.is_none() && d1_attendee.is_none() {
            // Truly unknown — nothing to delete anywhere.
            return Err(AppError::NotFound(format!(
                "attendee '{id}' not found in walk-in records, event sheet, or D1"
            ))
            .into());
        }

        // Reconcile into one view for the cleanup below.
        // Prefer the Sheet row (it carries the live row_index + the email as
        // stored in the sheet); fall back to the D1 record.
        let attendee = sheet_attendee.clone().or(d1_attendee.clone());
        let attendee = match attendee {
            Some(a) => a,
            None => {
                return Err(
                    AppError::NotFound(format!("attendee '{id}' could not be resolved")).into(),
                );
            }
        };

        // Choose the sheet row_index to delete:
        //   1. Sheet's live row_index (authoritative) when available.
        //   2. D1's sheet_row_index as a last resort — but only if non-zero,
        //      since 0 is the "unset" sentinel and would delete the header row.
        let sheet_row_to_delete: Option<usize> = sheet_attendee
            .as_ref()
            .map(|a| a.row_index)
            .filter(|&r| r >= 2) // row 1 is the header; data starts at row 2
            .or_else(|| {
                let r = d1_attendee.as_ref().map(|a| a.row_index).unwrap_or(0);
                if r >= 2 {
                    tracing::warn!(
                        event_id = %event.id,
                        attendee_id = %id,
                        row_index = r,
                        "sheet lookup missed; deleting by D1 sheet_row_index (may be stale)"
                    );
                    Some(r)
                } else {
                    None
                }
            });

        source = if sheet_attendee.is_some() {
            "sheet"
        } else {
            "d1-orphan"
        }
        .to_string();

        // Delete the Google Sheet row (best-effort). If we couldn't resolve a
        // row_index at all, skip the sheet delete but still clean D1/KV so the
        // attendee leaves the dashboard.
        if let Some(row_index) = sheet_row_to_delete {
            let mapping =
                sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, kv)
                    .await
                    .unwrap_or_else(|_| {
                        event_checkin_domain::models::attendee::ColumnMapping::hardcoded()
                    });

            if let Some(ctx) = &state.worker_ctx {
                ctx.wait_until(crate::sheets::bg_sync::delete_sheet_row(
                    state.clone(),
                    row_index,
                    mapping.clone(),
                    event.sheet_id.clone(),
                    event.sheet_name.clone(),
                    kv.cloned(),
                ));
            } else {
                crate::sheets::write::delete_sheet_row(
                    row_index,
                    &mapping,
                    &state,
                    &event.sheet_id,
                    &event.sheet_name,
                    kv,
                )
                .await
                .map_err(|e| AppError::Internal(format!("failed to delete sheet row: {e}")))?;
            };
        } else {
            tracing::warn!(
                event_id = %event.id,
                attendee_id = %id,
                "no resolvable sheet row_index; skipping sheet delete (D1/KV still cleaned)"
            );
        }

        // Delete from D1 (primary store). Use email when we have it (covers
        // duplicate-id edge cases), else delete by id.
        let email_lower = attendee.email.to_lowercase();
        if !email_lower.is_empty() {
            if let Some(db) = state.d1.as_deref()
                && let Err(e) =
                    crate::db::attendees::delete_attendee(db, &event.id, &email_lower).await
            {
                tracing::warn!(
                    event_id = %event.id,
                    email = %email_lower,
                    error = %e,
                    "D1 attendee delete (by email) failed"
                );
            }
        } else if let Some(db) = state.d1.as_deref()
            && let Err(e) = crate::db::attendees::delete_attendee_by_id(db, &id).await
        {
            tracing::warn!(
                event_id = %event.id,
                attendee_id = %id,
                error = %e,
                "D1 attendee delete (by id) failed"
            );
        }

        // Clean up KV keys for this attendee
        if let Some(kv) = kv {
            // Deposit status
            let dkey = crate::event_store::deposit_status_key(&event.id, &attendee.api_id);
            let _ = kv.delete(&dkey).await;
            deleted_keys.push(dkey);

            // THB deposit (KV cleanup — D1 also cleaned below)
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
        }

        if let Some(db) = state.d1.as_deref()
            && let Err(e) =
                crate::db::thb_deposits::delete_thb_deposit(db, &event.id, &attendee.api_id).await
        {
            tracing::warn!(
                event_id = %event.id,
                attendee_id = %attendee.api_id,
                error = %e,
                "D1 THB deposit delete failed"
            );
        }

        // Delete deposit status from D1
        if let Some(db) = state.d1.as_deref()
            && let Err(e) =
                crate::db::deposit_statuses::delete_deposit_status(db, &event.id, &attendee.api_id)
                    .await
        {
            tracing::warn!(
                event_id = %event.id,
                attendee_id = %attendee.api_id,
                error = %e,
                "D1 deposit status delete failed"
            );
        }

        tracing::info!(
            event_id = %event.id,
            attendee_id = %attendee.api_id,
            name = %attendee.display_name(),
            row_index = ?sheet_row_to_delete,
            source = %source,
            "attendee deleted"
        );
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
            state.d1.as_deref(),
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

/// Normalize a manual participation_type override into its canonical form.
///
/// Accepts both display-case (`In-Person`/`Online`) and canonical
/// (`in_person`/`online`) input — the frontend currently sends display-case.
/// Rejects `Other` (covers `walkin`, `test`, junk) so this endpoint can't be
/// used to clobber the `walkin` sentinel or set garbage.
///
/// Returns `(canonical_for_d1, display_for_sheet)`.
fn normalize_override(raw: &str) -> Result<ParticipationType, AppError> {
    // Reject empty explicitly — the parse() default-to-in-person behavior is for
    // legacy reads, not for an explicit manual override (empty is a no-op/error).
    if raw.trim().is_empty() {
        return Err(AppError::Validation(
            "participation_type must not be empty".to_string(),
        ));
    }
    let parsed = ParticipationType::parse(raw);
    if parsed == ParticipationType::Other {
        return Err(AppError::Validation(format!(
            "participation_type must be in-person or online, got '{raw}'"
        )));
    }
    Ok(parsed)
}

/// Request body for `PATCH /attendee/:id/participation-type`.
#[derive(serde::Deserialize)]
pub(crate) struct UpdateParticipationTypeBody {
    participation_type: String,
}

/// PATCH /api/attendee/{id}/participation-type
///
/// Manually override an attendee's participation_type (In-Person ⇄ Online).
///
/// Fills the gap left by the deposit-deadline auto-switch: when an attendee
/// chose the deposit flow, never came back to the system, and later confirms
/// out-of-band (phone/Telegram/email) that they will attend online, the admin
/// can flip them here instead of editing the Google Sheet cell by hand.
///
/// Writes **both** the Google Sheet (column I, via detached `wait_until`)
/// **and** D1 — unlike the auto-switch, which only writes the Sheet. Deposit
/// records and slips are intentionally left intact; this is purely a
/// participation-mode change, not a payment-state change.
#[worker::send]
pub async fn update_participation_type(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Query(query): Query<EventIdQuery>,
    axum::Json(body): axum::Json<UpdateParticipationTypeBody>,
) -> Result<ApiOk<serde_json::Value>, crate::error::WorkerError> {
    let resolved = normalize_override(body.participation_type.trim())?;
    // Canonical form for D1 storage; display form for the organizer-facing Sheet.
    let new_value = resolved.as_str().to_string();
    let new_value_display = resolved.display().to_string();

    tracing::info!(
        attendee_id = %id,
        new_participation_type = %new_value,
        staff_email = %claims.email,
        "manual participation_type override"
    );

    let event = resolve_event_with_access(&state, &claims, query.event_id.as_deref()).await?;
    let kv = resolve_kv(&state);

    // Look up the attendee to get the correct Sheet row_index (mirrors delete_attendee).
    let attendee = {
        let map = sheets::get_attendees_map(&state, &event.sheet_id, &event.sheet_name, kv)
            .await
            .map_err(|e| AppError::Internal(format!("failed to look up attendee: {e}")))?;
        map.get(&id).cloned()
    };
    let row_index = match attendee.as_ref().map(|a| a.row_index) {
        Some(idx) => Some(idx),
        None => {
            // No sheet row — likely a walk-in or D1-only record. Not an error:
            // we still update D1 below.
            tracing::warn!(
                attendee_id = %id,
                event_id = %event.id,
                "participation_type override: attendee not found in sheet (will update D1 only if present)"
            );
            None
        }
    };

    // 1. Update the Google Sheet cell (detached via wait_until when possible).
    if let Some(row_index) = row_index {
        let mapping = sheets::get_column_mapping(&state, &event.sheet_id, &event.sheet_name, kv)
            .await
            .unwrap_or_else(|_| event_checkin_domain::models::attendee::ColumnMapping::hardcoded());

        if let Some(ctx) = &state.worker_ctx {
            ctx.wait_until(crate::sheets::bg_sync::update_participation_type(
                state.clone(),
                row_index,
                new_value_display.clone(),
                mapping,
                event.sheet_id.clone(),
                event.sheet_name.clone(),
                kv.cloned(),
            ));
        } else {
            // Fallback: blocking Sheets write when worker_ctx unavailable (tests)
            crate::sheets::write::update_participation_type(
                row_index,
                &new_value_display,
                &mapping,
                &state,
                &event.sheet_id,
                &event.sheet_name,
                kv,
            )
            .await
            .map_err(|e| {
                AppError::Internal(format!("failed to update sheet participation_type: {e}"))
            })?;
        }
    }

    // 2. Update D1 (primary store for walk-ins; keeps ticket page + admin list
    //    in sync with the Sheet for regular attendees too).
    let d1_updated = if let Some(db) = state.d1.as_deref() {
        match crate::db::attendees::set_participation_type(db, &id, &new_value).await {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(
                    attendee_id = %id,
                    error = %e,
                    "participation_type override: D1 update failed (sheet was still updated)"
                );
                false
            }
        }
    } else {
        false
    };

    // 3. Audit log.
    if let Some(kv) = &state.events_kv {
        let _ = crate::audit_store::append_event_audit(
            kv,
            &event.id,
            crate::audit_store::create_entry_with_meta(
                &claims.email,
                crate::audit_store::AuditAction::ParticipationTypeChanged,
                &id,
                &format!("participation_type → {new_value}"),
                serde_json::json!({
                    "old_value": attendee.as_ref().map(|a| a.participation_type.clone()).unwrap_or_default(),
                    "new_value": new_value,
                    "sheet_row_updated": row_index.is_some(),
                    "d1_updated": d1_updated,
                }),
            ),
            state.d1.as_deref(),
        )
        .await;
    }

    // Note: there is no server-side attendee-list cache to invalidate
    // (removed in Phase 2d — the list is read directly from D1/sheet each
    // request). The client-side cache is invalidated by the frontend caller
    // after a successful PATCH.

    tracing::info!(
        attendee_id = %id,
        event_id = %event.id,
        new_participation_type = %new_value,
        sheet_row_updated = row_index.is_some(),
        d1_updated,
        "participation_type override complete"
    );

    Ok(ApiOk::new(json!({
        "attendee_id": id,
        "event_id": event.id,
        "participation_type": new_value,
        "sheet_row_updated": row_index.is_some(),
        "d1_updated": d1_updated,
    })))
}

#[cfg(test)]
mod tests {
    use super::normalize_override;
    use event_checkin_domain::models::attendee::ParticipationType;
    use event_checkin_domain::models::error::AppError;

    #[test]
    fn normalize_override_accepts_display_case() {
        // Frontend currently sends display-case — must keep working.
        assert_eq!(
            normalize_override("In-Person").unwrap(),
            ParticipationType::InPerson
        );
        assert_eq!(
            normalize_override("Online").unwrap(),
            ParticipationType::Online
        );
    }

    #[test]
    fn normalize_override_accepts_canonical() {
        // New canonical form (post-Tier-B) also accepted.
        assert_eq!(
            normalize_override("in_person").unwrap(),
            ParticipationType::InPerson
        );
        assert_eq!(
            normalize_override("online").unwrap(),
            ParticipationType::Online
        );
    }

    #[test]
    fn normalize_override_accepts_case_variants_and_synonyms() {
        assert_eq!(
            normalize_override("IN-PERSON").unwrap(),
            ParticipationType::InPerson
        );
        assert_eq!(
            normalize_override("physical").unwrap(),
            ParticipationType::InPerson
        );
        assert_eq!(
            normalize_override("Virtual").unwrap(),
            ParticipationType::Online
        );
    }

    #[test]
    fn normalize_override_rejects_walkin_sentinel() {
        // `walkin` is a status sentinel, not a participation mode — this endpoint
        // must never set it (would corrupt walk-in detection).
        assert!(matches!(
            normalize_override("walkin"),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn normalize_override_rejects_junk_and_empty() {
        assert!(matches!(
            normalize_override("test"),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            normalize_override(""),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            normalize_override("TBD"),
            Err(AppError::Validation(_))
        ));
    }
}

//! Attendee read endpoints — `GET /api/attendee/:id` and the public ticket
//! endpoint `GET /api/public/ticket/:id`, plus their shared QR/masking helpers.

use axum::{
    Extension,
    extract::{Path, Query, State},
};
use serde_json::json;

use worker::KvStore;

use crate::error::ApiOk;
use event_checkin_domain::models::api::AttendeeResponse;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;

use crate::handlers::ext::{EventIdQuery, resolve_event, resolve_event_with_access, resolve_kv};
use crate::sheets;
use crate::state::AppState;

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

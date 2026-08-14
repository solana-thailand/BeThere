//! `register_attendee` — the public self-registration handler.

use axum::{
    Extension, Json,
    extract::State,
};
use uuid::Uuid;

use event_checkin_domain::models::attendee::ParticipationType;
use event_checkin_domain::models::auth::Claims;
use event_checkin_domain::models::error::AppError;
use event_checkin_domain::models::event::{EventFormat, EventStatus};

use crate::error::ApiOk;
use crate::sheets;
use crate::state::AppState;

use super::capacity::enforce_capacity;
use super::contact::{upsert_contact_after_registration, write_developer_data};
use super::my_registration::{build_next_step, is_online_participation};
use super::types::{DeveloperData, NextStep, RegisterRequest, RegisterResponse};

/// POST /api/public/register
///
/// Self-registration endpoint — requires JWT identity (verified email).
/// Email is taken from JWT claims, not the request body.
/// Flow: validate → resolve event → check status → dedup email → append to sheet → return next step.
#[worker::send]
pub async fn register_attendee(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<RegisterRequest>,
) -> Result<ApiOk<RegisterResponse>, crate::error::WorkerError> {
    // 1. Validate input
    let name = body.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::Validation("name is required (max 100 chars)".to_string()).into());
    }

    // Identity resolution (Plan 017 — wallet↔email convergence).
    // Google sessions: email comes from the verified JWT. Wallet-only sessions
    // have a synthetic `wallet:<address>` identity and MUST supply a real email
    // in the body to reserve — the reservation is filed under that email, and
    // (only if the email is brand-new) the proven wallet is bound to it.
    let jwt_email = claims.email.trim().to_lowercase();
    let is_wallet_session = jwt_email.starts_with("wallet:");
    let email = if is_wallet_session {
        let typed = body.email.trim().to_lowercase();
        if !is_plausible_email(&typed) {
            return Err(AppError::Validation(
                "please enter a valid email to reserve your spot".to_string(),
            )
            .into());
        }
        typed
    } else {
        jwt_email.clone()
    };
    // Wallet address for wallet sessions (JWT `sub` = the base58 address).
    let session_wallet = if is_wallet_session {
        Some(claims.sub.trim().to_string())
    } else {
        None
    };
    tracing::info!(%email, is_wallet_session, "registration identity resolved");

    // For wallet sessions, decide up-front whether this email is brand-new.
    // Must be evaluated BEFORE we upsert the contact/attendee below (which would
    // otherwise make every email look "existing"). On DB error, fail safe by
    // treating the email as existing so we never auto-bind on uncertain state.
    let email_is_new = if is_wallet_session {
        match state.d1.as_deref() {
            Some(db) => !crate::db::contacts::email_has_account(db, &email)
                .await
                .unwrap_or(true),
            None => false,
        }
    } else {
        false
    };

    // SECURITY: rolling deposit credit is stored value tied to an email. Only
    // spend it when the caller has PROVEN ownership of that email — a Google-
    // verified session, or a wallet session whose wallet is already bound to
    // this email. A wallet session that merely *types* an email (Plan 017) has
    // not proven ownership, so it must NOT be able to drain that email's credit
    // (which would also hand the attacker a deposit-funded spot). Such sessions
    // fall through to the normal payment path.
    let credit_identity_ok = if !is_wallet_session {
        true
    } else if let (Some(db), Some(wallet)) = (state.d1.as_deref(), session_wallet.as_deref()) {
        matches!(
            crate::db::contacts::find_email_by_wallet(db, wallet).await,
            Ok(Some(bound_email)) if bound_email.eq_ignore_ascii_case(&email)
        )
    } else {
        false
    };

    let slug = body.slug.trim();
    if slug.is_empty() {
        return Err(AppError::Validation("event slug is required".to_string()).into());
    }

    // 2. Resolve event by slug (KV → D1 fallback)
    let kv = state.events_kv.as_ref();

    let config = crate::event_store::resolve_event_by_slug(kv, slug, state.d1.as_deref())
        .await
        .map_err(AppError::NotFound)?;

    let event_id = config.id.clone();

    // 3. Check event is Active
    if config.status != EventStatus::Active {
        return Err(
            AppError::Validation("registration is not open for this event".to_string()).into(),
        );
    }

    // 3b. Validate contact info if required by event
    let contact_channel = body
        .contact_channel
        .as_deref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty());
    let contact_handle = body
        .contact_handle
        .as_deref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty());

    if config.require_contact_info {
        if contact_channel.is_none() {
            return Err(AppError::Validation(
                "please select a preferred contact channel".to_string(),
            )
            .into());
        }
        if contact_handle.is_none() {
            return Err(AppError::Validation(
                "please provide your contact username or profile link".to_string(),
            )
            .into());
        }
    }

    // 3c. Determine participation_type early (needed before deposit check)
    // Returns canonical snake_case for D1; `*_display` is the display-case form
    // written to the Google Sheet (organizer-facing).
    let participation_type =
        resolve_participation_type(&config.event_format, body.participation_type.as_deref())?;
    let participation_type_display = ParticipationType::parse(&participation_type)
        .display()
        .to_string();

    // 3d. Validate PDPA consent — always required
    if body.consent_given != Some(true) {
        return Err(AppError::Validation(
            "you must consent to data collection to register".to_string(),
        )
        .into());
    }

    // 3d2. Validate photo consent if event requires it
    if config.require_photo_consent && body.photo_consent_given != Some(true) {
        return Err(AppError::Validation(
            "you must consent to photo/media capture to register".to_string(),
        )
        .into());
    }

    // Staff / organizer / super-admin registering for their own event: waive the
    // deposit entirely (they run the event, won't no-show). This replaces the old
    // workaround of uploading a fake slip to get past the deposit step, and marks
    // the record as a comp so it is never treated as a cash deposit to refund.
    let deposit_waived = crate::event_store::has_event_access(&config, &email)
        || state.is_staff(&email)
        || state
            .config
            .super_admin_emails
            .contains(&email.to_lowercase());

    // 3e. Validate deposit agreement if deposit is enabled — skip for Online
    // attendees and for waived staff/organizers.
    if config.deposit_enabled
        && !is_online_participation(&participation_type)
        && !deposit_waived
        && body.deposit_agreed != Some(true)
    {
        return Err(AppError::Validation(
            "you must agree to the deposit commitment to register".to_string(),
        )
        .into());
    }

    // 5. Check for duplicate email in the Google Sheet
    let attendees = sheets::get_attendees_for_event(
        &state,
        &config.sheet_id,
        &config.sheet_name,
        kv,
        &config.id,
    )
    .await
    .map_err(|e| {
        tracing::warn!(error = ?e, "could not fetch attendees for dedup");
        AppError::Internal(format!("failed to check existing registrations: {e}"))
    })?;

    // Duplicate email check: if already registered, return existing attendee info
    // so the frontend can redirect to the correct step (deposit/ticket) instead of
    // showing an error. This handles the case where localStorage is cleared or the
    // attendee uses a different device.
    if let Some(existing) = attendees.iter().find(|a| a.email.to_lowercase() == email) {
        // SECURITY (#1, IDOR): a wallet session that merely TYPED this email has
        // NOT proven ownership (credit_identity_ok is false unless the wallet is
        // bound to it). Returning the existing attendee's claim_token / api_id /
        // name would let anyone with a throwaway wallet read a victim's claim and
        // mint their badge. Block the read and direct them to authenticate.
        if is_wallet_session && !credit_identity_ok {
            tracing::warn!(
                %email, %slug, wallet = ?session_wallet,
                "blocked wallet-session duplicate-return for unproven email (IDOR guard)"
            );
            return Err(AppError::Validation(
                "This email is already registered. Sign in with that email (Google) — or link this wallet to it from your profile — to access your ticket.".to_string(),
            )
            .into());
        }
        tracing::info!(%email, %slug, "registration duplicate — returning existing attendee");
        let claim_token = existing.claim_token.clone().unwrap_or_default();
        // Fetch deposit status (D1-first, KV fallback)
        let deposit = crate::event_store::get_deposit_status_with_fallback(
            state.events_kv.as_ref(),
            state.d1.as_deref(),
            &event_id,
            &existing.api_id,
        )
        .await
        .ok()
        .flatten();

        // Check if deposit deadline expired — attendee may have been auto-switched to Online
        let deadline_expired = deposit.is_none()
            && config.deposit_deadline_hours.is_some()
            && existing.is_in_person()
            && existing.registration_date.as_ref().is_some_and(|reg_str| {
                if let Ok(reg_time) = chrono::DateTime::parse_from_rfc3339(reg_str) {
                    let deadline = reg_time.with_timezone(&chrono::Utc)
                        + chrono::Duration::hours(i64::from(
                            config.deposit_deadline_hours.unwrap_or(0),
                        ));
                    chrono::Utc::now() > deadline
                } else {
                    false
                }
            });

        let next_step = if deadline_expired {
            // Deadline expired — check if reclaim is possible
            let capacity_available = if let Some(cap) = config.in_person_capacity {
                let in_person_count = attendees.iter().filter(|a| a.is_in_person()).count() as u32;
                in_person_count < cap
            } else {
                true // No capacity limit = reclaim available
            };

            if capacity_available && deposit.is_none() {
                // Reclaim: send to deposit page — the deposit handler will
                // switch participation_type back to In-Person
                NextStep {
                    step_type: "deposit".to_string(),
                    url: format!("/deposit/{}?event_id={}", existing.api_id, event_id),
                }
            } else {
                // Capacity full or already deposited — online track
                NextStep {
                    step_type: "waiting".to_string(),
                    url: format!("/ticket/{}?event_id={}", existing.api_id, event_id),
                }
            }
        } else {
            let is_checked_in = existing
                .checked_in_at
                .as_ref()
                .is_some_and(|s| !s.is_empty());
            let is_claimed = existing.claimed_at.as_ref().is_some_and(|s| !s.is_empty());
            build_next_step(
                &config.event_format,
                &event_id,
                &existing.api_id,
                &claim_token,
                &state,
                deposit.as_ref(),
                &existing.participation_type,
                is_checked_in,
                is_claimed,
            )
        };
        return Ok(ApiOk::new(RegisterResponse {
            attendee_id: existing.api_id.clone(),
            name: existing.name.clone(),
            email: existing.email.clone(),
            claim_token,
            next_step,
            // Already registered ⇒ email exists ⇒ never auto-bind here.
            wallet_linked: if is_wallet_session { Some(false) } else { None },
        }));
    }

    // SECURITY (#3, identity spoofing): reaching here means the email is not yet
    // registered for THIS event. A wallet session filing a NEW reservation under
    // an email that already has an account it doesn't own would poison that
    // identity (a token bound to the victim's email, a spot they never took).
    // Brand-new emails (Plan 017 wallet→email bind) and proven owners are allowed.
    if is_wallet_session && !credit_identity_ok && !email_is_new {
        tracing::warn!(
            %email, %slug, wallet = ?session_wallet,
            "blocked wallet-session reservation under an existing unowned email (spoofing guard)"
        );
        return Err(AppError::Validation(
            "This email already has an account. Sign in with that email (Google), or link this wallet to it from your profile, to register.".to_string(),
        )
        .into());
    }

    // 5b. Enforce capacity limits (only for new registrations)
    enforce_capacity(&state, &config, &participation_type, kv).await?;

    // 5c. Check if attendee has rolling deposit credit that covers this event's deposit
    let mut credit_covered_method: Option<String> = None;
    let mut credit_amount_applied: u64 = 0;

    if config.deposit_enabled
        && !is_online_participation(&participation_type)
        && credit_identity_ok
        && !deposit_waived
    {
        // Balance comes from the org-scoped D1 credit ledger (source of truth),
        // not the Google Contacts sheet (whose duplicate rows shadowed credit and
        // whose non-atomic writes lost it — incident 2026-08-14). Scoped by the
        // event's organization_id so Org A's credit can't cover Org B's deposit.
        if let Some(db) = state.d1.as_deref() {
            let org = &config.organization_id;
            let credit_thb = crate::db::credit_ledger::balance(db, &email, org, "thb")
                .await
                .unwrap_or(0)
                .max(0) as u64;
            let credit_usdc = crate::db::credit_ledger::balance(db, &email, org, "usdc")
                .await
                .unwrap_or(0)
                .max(0) as u64;
            let required_thb = config.deposit_amount_thb;
            let required_usdc = config.deposit_amount_usdc;
            if required_thb > 0 && credit_thb >= required_thb {
                credit_covered_method = Some("credit_thb".to_string());
                credit_amount_applied = required_thb;
            } else if required_usdc > 0 && credit_usdc >= required_usdc {
                credit_covered_method = Some("credit_usdc".to_string());
                credit_amount_applied = required_usdc;
            }
            if let Some(ref method) = credit_covered_method {
                tracing::info!(%email, %slug, %method, amount = credit_amount_applied, "deposit covered by rolling credit (ledger)");
            }
        }
    }

    // 6. Generate IDs
    let api_id = Uuid::now_v7().to_string();
    let claim_token = Uuid::now_v7().to_string();

    // 7. Split name into first_name / last_name
    let (first_name, last_name) = split_name(name);

    let now = chrono::Utc::now().to_rfc3339();

    // 7b. Persist the attendee to D1 (the source of truth) BEFORE spending any
    // credit or returning success. Fatal by design: if this write fails the
    // reservation isn't durable, so we must not (a) tell the attendee they're
    // registered, nor (b) consume their rolling credit for a reservation that
    // vanished. They get a retryable error instead of a silent lost registration.
    // (When D1 isn't configured — tests/local — we skip and rely on Sheets.)
    if let Some(ref d1) = state.d1
        && let Err(e) = crate::db::attendees::upsert_attendee(
            d1,
            &api_id,
            &event_id,
            &email,
            name,
            "approved", // self-registered attendees are auto-approved
            &participation_type,
            contact_channel.unwrap_or(""),
            contact_handle.unwrap_or(""),
            body.consent_marketing,
            Some(&claim_token),
        )
        .await
    {
        // A UNIQUE(event_id, lower(email)) violation means a concurrent or prior
        // registration for the same person won the race (the Sheets-based dedup
        // above can lag). D1 is authoritative, so treat it as already-registered
        // rather than a hard failure — and importantly, no credit is spent (that
        // happens below, only if we get past this).
        if e.to_ascii_uppercase().contains("UNIQUE") {
            tracing::info!(%email, %event_id, "duplicate registration blocked by unique index");
            return Err(AppError::Validation(
                "You're already registered for this event — check your email or 'My Registrations'."
                    .to_string(),
            )
            .into());
        }
        tracing::error!(%api_id, %email, %event_id, error = %e, "D1 attendee write failed — failing registration (source of truth)");
        return Err(AppError::Internal(
            "could not save your registration — please try again".to_string(),
        )
        .into());
    }

    // Auto-apply rolling credit — fail-closed and correctly ordered.
    //
    // Consume the credit FIRST; only mark the deposit covered if that succeeded.
    // If we can't decrement (e.g. Sheets write error / no contacts sheet), we do
    // NOT grant a free deposit — `credit_covered_method` is cleared so the
    // attendee falls back to the normal payment path and keeps their credit.
    // The reverse order (mark covered, then decrement) risks double-spending
    // credit whenever the decrement fails — real money leaking on every retry.
    if let Some(method) = credit_covered_method.clone() {
        let currency = if method == "credit_thb" { "thb" } else { "usdc" };
        // Atomic spend against the org-scoped credit ledger: one conditional
        // INSERT (balance >= amount) — no advisory lock or Sheets re-read needed,
        // and two concurrent registrations for the same email can't double-spend
        // (guard + insert are a single statement). Idempotent per (event, email).
        let apply_key = format!("apply:{}:{}", event_id, email.to_lowercase());
        let decremented = match state.d1.as_deref() {
            Some(db) => crate::db::credit_ledger::try_spend(
                db,
                &email,
                &config.organization_id,
                currency,
                credit_amount_applied as i64,
                &event_id,
                &apply_key,
            )
            .await
            .unwrap_or(false),
            // No D1 → can't spend safely → charge normally and keep the credit.
            None => false,
        };
        // Best-effort Sheets mirror of the spend (display only; ledger is truth).
        if decremented
            && let Some(db) = state.d1.as_deref()
        {
            let resolved =
                crate::org_store::resolve_contacts_sheet(db, &config, &state.config.sheets).await;
            if !resolved.sheet_id.is_empty() {
                let _ = crate::sheets::contacts::decrement_credit(
                    &state,
                    &resolved.sheet_id,
                    &resolved.contacts_sheet_name,
                    kv,
                    &email,
                    currency,
                    credit_amount_applied,
                )
                .await;
            }
        }

        if decremented {
            // Credit consumed — record the covered, verified deposit.
            let thb_dep = event_checkin_domain::models::deposit::ThbDeposit {
                event_id: event_id.clone(),
                attendee_id: api_id.clone(),
                amount_thb: credit_amount_applied,
                slip_url: Some("ROLLING_CREDIT_AUTO_APPLIED".to_string()),
                verified: true,
                verified_at: Some(now.clone()),
                verified_by: Some("SYSTEM_ROLLING_CREDIT".to_string()),
                uploaded_at: now.clone(),
                refunded: false,
                refunded_at: None,
                held_as_credit: false,
                held_as_credit_at: None,
                attendee_name: Some(name.to_string()),
                bank_account: None,
                bank_name: None,
                account_name: None,
                refund_proof_url: None,
            };
            if let Some(kv_store) = kv
                && let Err(e) = crate::event_store::save_thb_deposit(kv_store, &thb_dep, state.d1.as_deref()).await
            {
                // Credit already consumed but the deposit record didn't persist.
                // Non-fatal to the reservation; log loudly for reconciliation.
                tracing::error!(%api_id, %email, error = %e, "credit consumed but deposit record save failed — needs reconciliation");
            }
        } else {
            // Fail closed: revert to the normal payment path (credit untouched).
            credit_covered_method = None;
        }
    }

    // Staff/organizer comp: record a waived deposit (฿0, verified, not
    // refundable) so the ticket flow proceeds without a real or faked payment.
    // Marked distinctly (STAFF_COMP_WAIVED / ฿0) so refund + held-as-credit
    // tooling never treats it as cash.
    if deposit_waived
        && config.deposit_enabled
        && !is_online_participation(&participation_type)
    {
        let comp = event_checkin_domain::models::deposit::ThbDeposit {
            event_id: event_id.clone(),
            attendee_id: api_id.clone(),
            amount_thb: 0,
            slip_url: Some("STAFF_COMP_WAIVED".to_string()),
            verified: true,
            verified_at: Some(now.clone()),
            verified_by: Some("SYSTEM_STAFF_WAIVE".to_string()),
            uploaded_at: now.clone(),
            refunded: false,
            refunded_at: None,
            held_as_credit: false,
            held_as_credit_at: None,
            attendee_name: Some(name.to_string()),
            bank_account: None,
            bank_name: None,
            account_name: None,
            refund_proof_url: None,
        };
        if let Some(kv_store) = kv
            && let Err(e) =
                crate::event_store::save_thb_deposit(kv_store, &comp, state.d1.as_deref()).await
        {
            tracing::warn!(%api_id, %email, error = %e, "staff comp deposit record save failed");
        }
    }

    // 8. Resolve column mapping
    let mapping = match sheets::get_column_mapping(&state, &config.sheet_id, &config.sheet_name, kv)
        .await
    {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "failed to get column mapping, using hardcoded fallback");
            event_checkin_domain::models::attendee::ColumnMapping::hardcoded()
        }
    };

    // 9. Write to D1 first (source of truth), then detach all Sheets writes

    // 9a. Dual-write to D1 — contact row (non-fatal). The attendee row is
    // already written fatally at step 7b above (before credit spend), so it is
    // NOT re-written here.
    if let Some(ref d1) = state.d1 {
        let events_joined = event_id.clone();
        if let Err(e) = crate::db::contacts::upsert_contact(
            d1,
            &email,
            name,
            &events_joined,
            1, // event_count will be updated on subsequent registrations
            contact_channel.unwrap_or(""),
            contact_handle.unwrap_or(""),
        )
        .await
        {
            tracing::warn!(
                %email,
                error = %e,
                "D1 contact upsert failed (non-fatal)"
            );
        }

        // 9d. Developer profile + registration responses (Issue #049 Phase 2, non-fatal)
        let mut profile_fields: Vec<(String, String)> = Vec::new();

        // Merge hardcoded fields (backward compat)
        if let Some(ref v) = body.experience_level {
            profile_fields.push(("experience_level".to_string(), v.clone()));
        }
        if let Some(ref v) = body.tech_stack {
            profile_fields.push(("tech_stack".to_string(), v.clone()));
        }
        if let Some(ref v) = body.interests {
            profile_fields.push(("interests".to_string(), v.clone()));
        }

        // Merge dynamic fields (Phase 2)
        if let Some(ref fields) = body.profile_fields {
            for (key, value) in fields {
                if !value.is_empty() && !profile_fields.iter().any(|(k, _)| k == key) {
                    profile_fields.push((key.clone(), value.clone()));
                }
            }
        }

        write_developer_data(&DeveloperData {
            d1,
            email: &email,
            name,
            event_id: &event_id,
            contact_channel: contact_channel.unwrap_or(""),
            contact_handle: contact_handle.unwrap_or(""),
            participation_type: &participation_type,
            consent_given: body.consent_given.unwrap_or(false),
            photo_consent_given: body.photo_consent_given.unwrap_or(false),
            consent_marketing: body.consent_marketing.unwrap_or(false),
            profile_fields,
        })
        .await;
    }

    // 9b. Detach Google Sheets writes — response returns immediately (Phase 2c)
    let bg_credit_method = credit_covered_method.clone();
    if let Some(ctx) = &state.worker_ctx {
        let bg_state = state.clone();
        let bg_api_id = api_id.clone();
        let bg_name = name.to_string();
        let bg_first_name = first_name.to_string();
        let bg_last_name = last_name.to_string();
        let bg_email = email.clone();
        let bg_claim_token = claim_token.clone();
        let bg_participation_type = participation_type_display.clone();
        let bg_now = now.clone();
        let bg_contact_channel = contact_channel.map(String::from);
        let bg_contact_handle = contact_handle.map(String::from);
        let bg_deposit_agreed = body.deposit_agreed.unwrap_or(false);
        let bg_consent_given = body.consent_given.unwrap_or(false);
        let bg_photo_consent_given = body.photo_consent_given.unwrap_or(false);
        let bg_consent_marketing = body.consent_marketing;
        let bg_mapping = mapping.clone();
        let bg_sheet_id = config.sheet_id.clone();
        let bg_sheet_name = config.sheet_name.clone();
        let bg_kv = kv.cloned();
        let bg_event_id = event_id.clone();
        let bg_config = config.clone();

        ctx.wait_until(async move {
            // Append attendee row
            crate::sheets::bg_sync::append_attendee_row(
                bg_state.clone(),
                bg_api_id.clone(),
                bg_name.to_string(),
                bg_first_name.clone(),
                bg_last_name.clone(),
                bg_email.clone(),
                bg_claim_token.clone(),
                bg_participation_type.clone(),
                bg_now.clone(),
                bg_contact_channel.clone(),
                bg_contact_handle.clone(),
                bg_deposit_agreed,
                bg_consent_given,
                bg_photo_consent_given,
                bg_consent_marketing,
                bg_mapping.clone(),
                bg_sheet_id.clone(),
                bg_sheet_name.clone(),
                bg_kv.clone(),
            )
            .await;

            // Upsert to contacts sheet (matches upsert_contact_after_registration)
            let resolved = if let Some(db) = bg_state.d1.as_deref() {
                crate::org_store::resolve_contacts_sheet(db, &bg_config, &bg_state.config.sheets)
                    .await
            } else {
                event_checkin_domain::models::org::ResolvedContactsSheet {
                    sheet_id: bg_state.config.sheets.contacts_sheet_id.clone(),
                    contacts_sheet_name: bg_state.config.sheets.contacts_sheet_name.clone(),
                    events_sheet_name: bg_state.config.sheets.events_sheet_name.clone(),
                }
            };
            if !resolved.sheet_id.is_empty() {
                let contact_upsert = crate::sheets::contacts::ContactUpsert {
                    email: &bg_email,
                    name: &bg_name,
                    event_id: &bg_event_id,
                    contact_channel: bg_contact_channel.as_deref(),
                    contact_handle: bg_contact_handle.as_deref(),
                };
                if let Err(e) = crate::sheets::contacts::upsert_contact(
                    &contact_upsert,
                    &bg_state,
                    &resolved.sheet_id,
                    &resolved.contacts_sheet_name,
                    bg_kv.as_ref(),
                )
                .await
                {
                    tracing::warn!(%bg_email, error = %e, "bg_sync: contacts upsert failed");
                }
            }

            // Write deposit_method if credit covered the deposit
            if let Some(ref method) = bg_credit_method
                && let Err(e) = crate::sheets::write::update_deposit_method(
                    &bg_state,
                    &bg_sheet_id,
                    &bg_sheet_name,
                    bg_kv.as_ref(),
                    &bg_api_id,
                    method,
                )
                .await
            {
                tracing::warn!(%bg_api_id, error = %e, "bg_sync: deposit_method write failed");
            }
        });
    } else {
        // Fallback: blocking Sheets writes when worker_ctx unavailable (tests)
        if let Err(e) = sheets::append_attendee_row(
            &api_id,
            name,
            &first_name,
            &last_name,
            &email,
            &claim_token,
            &participation_type_display,
            &now,
            contact_channel,
            contact_handle,
            body.deposit_agreed.unwrap_or(false),
            body.consent_given.unwrap_or(false),
            body.photo_consent_given.unwrap_or(false),
            body.consent_marketing,
            &mapping,
            &state,
            &config.sheet_id,
            &config.sheet_name,
            kv,
        )
        .await
        {
            tracing::warn!(%email, error = %e, "Sheets append row failed (non-fatal)");
        }

        upsert_contact_after_registration(
            &email,
            name,
            &event_id,
            contact_channel,
            contact_handle,
            &state,
            &config,
            kv,
        )
        .await;
    }

    // 10. Determine next_step based on event format and participation type
    // Note: deposit_method is now written in the background (step 9b)
    // New registrations are never checked in or claimed yet
    let next_step = if credit_covered_method.is_some() || deposit_waived {
        NextStep {
            step_type: "ticket".to_string(),
            url: format!("/ticket/{api_id}?event_id={event_id}"),
        }
    } else {
        build_next_step(
            &config.event_format,
            &event_id,
            &api_id,
            &claim_token,
            &state,
            None,
            &participation_type,
            false, // new registration, not checked in
            false, // new registration, not claimed
        )
    };

    tracing::info!(
        %api_id,
        %email,
        %slug,
        %participation_type,
        "attendee self-registered"
    );

    // Plan 017: converge wallet→email. Bind the proven wallet only when the
    // email was brand-new; an existing email must be linked via the profile
    // flow (ownership-verified) instead of a typed-email bind here.
    let wallet_linked = if is_wallet_session {
        if email_is_new
            && let (Some(db), Some(w)) = (state.d1.as_deref(), session_wallet.as_ref())
        {
            match crate::db::contacts::link_wallet_to_email(db, &email, w).await {
                Ok(()) => {
                    tracing::info!(%email, "wallet bound to new email at registration");
                    Some(true)
                }
                Err(e) => {
                    tracing::warn!(%email, error = %e, "wallet bind at registration failed");
                    Some(false)
                }
            }
        } else {
            tracing::info!(%email, "email already has an account — wallet not auto-bound");
            Some(false)
        }
    } else {
        None
    };

    Ok(ApiOk::new(RegisterResponse {
        attendee_id: api_id,
        name: name.to_string(),
        email,
        claim_token,
        next_step,
        wallet_linked,
    }))
}

/// Minimal sanity check for a typed email (wallet-session reservations).
/// Not RFC-complete — just rejects obviously-invalid input: one `@`, a dot in
/// the domain, no spaces, reasonable length.
fn is_plausible_email(email: &str) -> bool {
    let e = email.trim();
    if e.len() < 3 || e.len() > 254 || e.contains(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = e.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

/// Resolve participation type based on event format and user selection.
///
/// Returns the **canonical** storage form (`in_person`/`online`) — see
/// `ParticipationType::as_str()`. The Google Sheet append path must convert
/// this to display-case via `ParticipationType::display()` for organizer-facing
/// cells; everything else (D1, capacity checks, logging) consumes canonical.
pub(super) fn resolve_participation_type(
    format: &EventFormat,
    user_choice: Option<&str>,
) -> Result<String, AppError> {
    let resolved = match format {
        EventFormat::InPerson => ParticipationType::InPerson,
        EventFormat::Online => ParticipationType::Online,
        EventFormat::Hybrid => match user_choice.map(str::trim).filter(|s| !s.is_empty()) {
            Some(choice) => ParticipationType::parse(choice),
            None => ParticipationType::InPerson,
        },
    };
    Ok(resolved.as_str().to_string())
}

/// Split a full name into (first_name, last_name).
/// First word → first_name, rest → last_name.
fn split_name(name: &str) -> (String, String) {
    let parts: Vec<&str> = name.split_whitespace().collect();
    match parts.as_slice() {
        [] => (String::new(), String::new()),
        [only] => (only.to_string(), String::new()),
        [first, rest @ ..] => (first.to_string(), rest.join(" ")),
    }
}

#[cfg(test)]
mod tests {
    use super::is_plausible_email;

    #[test]
    fn accepts_normal_emails() {
        assert!(is_plausible_email("a@b.co"));
        assert!(is_plausible_email("dev.user+tag@example.com"));
    }

    #[test]
    fn rejects_malformed_emails() {
        assert!(!is_plausible_email(""));
        assert!(!is_plausible_email("no-at-sign"));
        assert!(!is_plausible_email("@example.com"));
        assert!(!is_plausible_email("user@nodot"));
        assert!(!is_plausible_email("user@.com"));
        assert!(!is_plausible_email("user@example."));
        assert!(!is_plausible_email("has space@example.com"));
        assert!(!is_plausible_email("wallet:So1111111111111111111111111111111111111111"));
    }
}

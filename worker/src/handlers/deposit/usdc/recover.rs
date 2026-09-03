//! Self-healing recovery of an unverified deposit record from on-chain state.

use chrono::Utc;
use event_checkin_domain::models::deposit::DepositStatus;
use event_checkin_domain::models::event::EventConfig;

use crate::event_store;
use crate::state::AppState;

use super::discovery::{DISCOVERY_COOLDOWN_SECS, binding_conflict, discover_deposit_tx_on_chain};
use super::rpc::verify_tx_with_signer;

/// Self-heal a deposit record from the on-chain state.
///
/// Runs on every read/write path that touches an unverified `DepositStatus`.
/// It does:
///
/// 1. **Discovery** — if `tx_signature` is empty but `wallet_address` is
///    known, derive the AttendeeDeposit PDA and query `getSignaturesForAddress`
///    to recover the missing signature.
/// 2. **Verification** — if a `tx_signature` is present and the deposit is
///    not yet verified, run `verify_tx_with_signer` (confirms the TX on-chain
///    AND cross-checks the signer against the expected wallet). On success,
///    mark the deposit verified and trigger the side effects (D1 + Google
///    Sheet + QR generation).
///
/// **Idempotent & safe on public read paths**: returns immediately when
/// `status.verified == true`. Discovery is self-limiting (only runs while
/// `tx_signature` is empty; once recorded it never runs again). Verification
/// only runs while `!verified` (a transient state). The side effects are
/// themselves idempotent (set verified=true, sheet overwrite, QR only when
/// missing).
///
/// Returns the possibly-updated status so callers can reflect it in their
/// response without a second read.
pub(crate) async fn recover_and_verify_deposit(
    state: &AppState,
    event: &EventConfig,
    mut status: DepositStatus,
) -> DepositStatus {
    // Already verified — nothing to do.
    if status.verified {
        return status;
    }

    let kv = state.events_kv.as_ref();
    let d1 = state.d1.as_deref();
    let rpc_url = state.config.solana.full_rpc_url();
    let attendee_id = status.attendee_id.clone();

    // ── Phase 1: discover tx_signature on-chain if missing ──
    if status.tx_signature.as_deref().is_none_or(|s| s.is_empty()) {
        // Rate-limit discovery on public read paths: a malicious caller could
        // hammer /public/ticket for an attendee whose deposit has no on-chain
        // TX yet, triggering a getSignaturesForAddress RPC on every call. The
        // cooldown caps discovery to one attempt per DISCOVERY_COOLDOWN_SECS
        // per attendee. On KV-unavailable (tests), the gate is bypassed.
        let cooldown_key = format!("discovery_cooldown:{}:{}", event.id, attendee_id);
        let cooldown_active = if let Some(k) = kv {
            k.get(&cooldown_key).text().await.unwrap_or(None).is_some()
        } else {
            false
        };

        if !cooldown_active {
            let wallet = status
                .wallet_address
                .as_deref()
                .filter(|w| !w.is_empty());

            let escrow_addr = if !event.escrow_address.is_empty() {
                Some(event.escrow_address.clone())
            } else if !event.organizer_wallet.is_empty() {
                // Derive escrow address on the fly when not yet persisted.
                let on_chain_event_id = if event.on_chain_event_id != 0 {
                    event.on_chain_event_id
                } else {
                    crate::handlers::deposit::derive_on_chain_event_id(&event.id)
                };
                crate::solana_escrow::derive_escrow_address(
                    &event.organizer_wallet,
                    on_chain_event_id,
                )
                .await
                .ok()
            } else {
                None
            };

            if let (Some(wallet), Some(escrow)) = (wallet, escrow_addr)
                && let Some(discovered_sig) =
                    discover_deposit_tx_on_chain(&rpc_url, &escrow, wallet).await
            {
                tracing::info!(
                    attendee_id = %attendee_id,
                    event_id = %event.id,
                    tx_signature = %discovered_sig,
                    "Recovered deposit TX signature via on-chain PDA discovery (read-path)"
                );
                status.tx_signature = Some(discovered_sig);
                let _ = event_store::save_deposit_status_with_fallback(kv, d1, &status).await;
            }

            // Set cooldown after any discovery attempt (success, no-result, or
            // RPC failure) so the public read paths can't trigger unbounded
            // getSignaturesForAddress calls. Best-effort — KV failure is
            // non-fatal (next poll may retry sooner, which is acceptable).
            if let Some(k) = kv
                && let Ok(builder) = k.put(&cooldown_key, "1")
            {
                let _ = builder
                    .expiration_ttl(DISCOVERY_COOLDOWN_SECS)
                    .execute()
                    .await;
            }
        }
    }

    // ── Phase 2: verify via signer cross-check ──
    let Some(sig) = status.tx_signature.as_deref().filter(|s| !s.is_empty()) else {
        return status;
    };

    let expected_wallet = status.wallet_address.as_deref();
    let outcome = verify_tx_with_signer(&rpc_url, sig, expected_wallet).await;
    if !outcome.is_confirmed_and_matched() {
        if outcome.is_confirmed() {
            tracing::warn!(
                attendee_id = %attendee_id,
                tx_signature = %sig,
                "TX confirmed on-chain but signer does not match expected wallet (read-path) — refusing to verify"
            );
        }
        return status;
    }

    // Backfill wallet_address if missing (older record / web2 hiccup).
    if status.wallet_address.as_deref().is_none_or(|w| w.is_empty())
        && let Some(signer) = outcome.signer()
    {
        status.wallet_address = Some(signer.to_string());
    }

    // F1 (audit): the signer cross-check proves the wallet signed *a* confirmed
    // tx — NOT that it was a real deposit. Bind verification to the on-chain
    // `AttendeeDeposit` PDA: require it to exist with amount == the event's deposit
    // and refunded == false. Without this, a confirmed-but-unrelated tx by the
    // right fee-payer (e.g. a self-transfer) could earn a free verified ticket.
    // Fail CLOSED (defer, don't verify) on an RPC error; the read-path retries.
    let deposit_wallet = status.wallet_address.as_deref().unwrap_or("").to_string();
    if !event.organizer_wallet.is_empty() && !deposit_wallet.is_empty() {
        let on_chain_event_id = if event.on_chain_event_id != 0 {
            event.on_chain_event_id
        } else {
            crate::handlers::deposit::derive_on_chain_event_id(&event.id)
        };
        match crate::solana_escrow::verify_attendee_deposit_onchain(
            &rpc_url,
            &event.organizer_wallet,
            on_chain_event_id,
            &deposit_wallet,
            event.deposit_amount_usdc,
        )
        .await
        {
            Ok(true) => { /* genuine on-chain deposit — proceed to verify */ }
            Ok(false) => {
                tracing::warn!(
                    attendee_id = %attendee_id,
                    event_id = %event.id,
                    "deposit verify refused: no matching on-chain AttendeeDeposit PDA (amount/refunded) — F1 guard"
                );
                return status;
            }
            Err(e) => {
                tracing::warn!(
                    attendee_id = %attendee_id,
                    error = %e,
                    "deposit PDA read-back failed — deferring verification (fail-closed)"
                );
                return status;
            }
        }
    }

    // Guard 2 (double-registration defence — plan 003): refuse to verify if
    // the wallet or the discovered tx_signature is already bound to a
    // *different* attendee_id. The on-chain `AttendeeDeposit` PDA is keyed by
    // `(event_escrow, attendee_wallet)`, so a single deposit can otherwise be
    // claimed by N attendee rows when the DB is reset (e.g., organizer
    // deletes a row, attendee re-registers with the same wallet and lets the
    // read-path self-heal re-verify the new row). DB read failures are
    // non-fatal: they skip that half of the guard with a warning rather than
    // blocking the happy path, because the signer cross-check above has
    // already proven the TX is real.
    let post_backfill_wallet = status.wallet_address.as_deref().unwrap_or("");
    let wallet_owner =
        event_store::find_attendee_by_wallet_with_fallback(kv, d1, &event.id, post_backfill_wallet)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(
                    attendee_id = %attendee_id,
                    error = %e,
                    "wallet binding lookup failed on read-path recovery — skipping that half of the guard (non-fatal)"
                );
                None
            });

    let tx_owner =
        event_store::find_attendee_by_tx_signature_with_fallback(kv, d1, &event.id, sig)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(
                    attendee_id = %attendee_id,
                    error = %e,
                    "tx_signature binding lookup failed on read-path recovery — skipping that half of the guard (non-fatal)"
                );
                None
            });

    if binding_conflict(&attendee_id, wallet_owner.as_deref(), tx_owner.as_deref()) {
        tracing::warn!(
            attendee_id = %attendee_id,
            event_id = %event.id,
            tx_signature = %sig,
            wallet_owner = ?wallet_owner,
            tx_owner = ?tx_owner,
            "read-path recovery refused: deposit already bound to a different attendee"
        );

        // Best-effort audit: surface the refusal in the event's audit trail.
        // Reuses `DepositConfirmed` with a `refused: true` meta flag — a
        // dedicated `DepositRefused` action would be cleaner but is out of
        // scope for this fix.
        if let Some(kv) = kv {
            let _ = crate::audit_store::append_event_audit(
                kv,
                &event.id,
                crate::audit_store::create_entry_with_meta(
                    "system",
                    crate::audit_store::AuditAction::DepositConfirmed,
                    &attendee_id,
                    "USDC read-path recovery refused — deposit already bound to another attendee",
                    serde_json::json!({
                        "tx_signature": sig,
                        "wallet_owner": wallet_owner,
                        "tx_owner": tx_owner,
                        "refused": true,
                    }),
                ),
                d1,
            )
            .await;
        }

        // Return unchanged: still unverified, still carrying the discovered
        // `tx_signature` so a human can investigate the double-binding.
        return status;
    }

    status.verified = true;
    if let Err(e) = event_store::save_deposit_status_with_fallback(kv, d1, &status).await {
        tracing::error!(
            attendee_id = %attendee_id,
            error = %e,
            "failed to save verified deposit status (read-path)"
        );
        return status;
    }

    tracing::info!(
        attendee_id = %attendee_id,
        tx_signature = %sig,
        "USDC deposit self-healed via read-path recovery"
    );

    // D1 dual-write for the attendees row.
    if let Some(db) = d1
        && let Err(e) = crate::db::attendees::verify_deposit(
            db,
            &attendee_id,
            "verified",
            sig,
            status.amount as i64,
            &Utc::now().to_rfc3339(),
            "usdc_read_path",
        )
        .await
    {
        tracing::warn!(
            attendee_id = %attendee_id,
            error = %e,
            "D1 USDC deposit verify failed on read-path (non-fatal)"
        );
    }

    // Audit log (best-effort).
    if let Some(kv) = kv {
        let _ = crate::audit_store::append_event_audit(
            kv,
            &event.id,
            crate::audit_store::create_entry_with_meta(
                "system",
                crate::audit_store::AuditAction::DepositConfirmed,
                &attendee_id,
                "USDC deposit self-healed via read-path recovery",
                serde_json::json!({
                    "tx_signature": sig,
                    "confirmed": true,
                }),
            ),
            state.d1.as_deref(),
        )
        .await;
    }

    // Sheet write + QR auto-gen — detached via wait_until on the hot path so
    // the read response returns immediately; falls back to blocking when
    // worker_ctx is unavailable (tests).
    let deposit_amount_str = status.amount.to_string();
    let (mapping_result, attendee_result) = futures_util::join!(
        crate::sheets::get_column_mapping(
            state,
            &event.sheet_id,
            &event.sheet_name,
            kv,
        ),
        crate::sheets::get_attendee_by_id(
            &attendee_id,
            state,
            &event.sheet_id,
            &event.sheet_name,
            kv,
        )
    );
    if let (Ok(mapping), Ok(Some(attendee))) = (mapping_result, attendee_result) {
        if let Some(ctx) = &state.worker_ctx {
            ctx.wait_until(crate::sheets::bg_sync::write_deposit_verification(
                state.clone(),
                attendee.row_index,
                "USDC".to_string(),
                deposit_amount_str.clone(),
                true,
                mapping.clone(),
                event.sheet_id.clone(),
                event.sheet_name.clone(),
                kv.cloned(),
            ));

            if attendee.qr_code_url.as_ref().is_none_or(|u| u.is_empty()) {
                let server_url = &state.config.server.url;
                let qr_url = format!("{server_url}/staff/?scan={}", attendee.api_id);

                if let Some(ref d1) = state.d1
                    && let Err(e) =
                        crate::db::attendees::set_qr_url(d1, &attendee.api_id, &qr_url).await
                {
                    tracing::warn!(
                        attendee_id = %attendee.api_id,
                        error = %e,
                        "D1 set_qr_url failed on read-path recovery (non-fatal)"
                    );
                }

                ctx.wait_until(crate::sheets::bg_sync::update_qr_urls(
                    state.clone(),
                    vec![(attendee.row_index, qr_url)],
                    mapping,
                    event.sheet_id.clone(),
                    event.sheet_name.clone(),
                    kv.cloned(),
                ));
            }
        } else {
            // Fallback: blocking writes when worker_ctx unavailable (tests).
            let ctx = crate::sheets::write::SheetContext {
                mapping: &mapping,
                state,
                sheet_id: &event.sheet_id,
                sheet_name: &event.sheet_name,
                kv,
            };

            if let Err(e) = crate::sheets::write::write_deposit_verification(
                attendee.row_index,
                "USDC",
                &deposit_amount_str,
                true,
                &ctx,
            )
            .await
            {
                tracing::warn!(error = %e, "failed to write deposit verification to sheet on read-path (non-fatal)");
            }

            if attendee.qr_code_url.as_ref().is_none_or(|u| u.is_empty()) {
                let server_url = &state.config.server.url;
                let qr_url = format!("{server_url}/staff/?scan={}", attendee.api_id);

                if let Some(ref d1) = state.d1
                    && let Err(e) =
                        crate::db::attendees::set_qr_url(d1, &attendee.api_id, &qr_url).await
                {
                    tracing::warn!(
                        attendee_id = %attendee.api_id,
                        error = %e,
                        "D1 set_qr_url failed on read-path recovery (non-fatal)"
                    );
                }

                if let Err(e) = crate::sheets::write::update_qr_urls(
                    &[(attendee.row_index, qr_url)],
                    &mapping,
                    state,
                    &event.sheet_id,
                    &event.sheet_name,
                    kv,
                )
                .await
                {
                    tracing::warn!(error = %e, "failed to auto-generate QR on read-path recovery (non-fatal)");
                }
            }
        }
    }

    status
}

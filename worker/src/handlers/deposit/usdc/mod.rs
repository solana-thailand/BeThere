//! USDC deposit handlers — Solana Pay flow, on-chain verification, Helius webhook.

mod handlers;

use chrono::Utc;
use event_checkin_domain::models::event::EventConfig;

use crate::event_store;
use crate::state::AppState;

// Re-export all public handlers so `deposit/mod.rs` can reference them as `usdc::<handler>`.
pub use handlers::{
    confirm_deposit_handler, deposit_usdc_handler, deposit_usdc_tx_handler,
    deposit_webhook_handler, get_deposit_status_handler,
};

// ---------------------------------------------------------------------------
// Types (shared between handlers and helpers)
// ---------------------------------------------------------------------------

/// Query parameters for the Solana Pay TX callback.
#[derive(Debug, serde::Deserialize)]
pub struct DepositTxQuery {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID from Google Sheets.
    pub attendee_id: String,
    /// Attendee's Solana wallet address (base58).
    pub wallet: String,
}

/// Solana Pay Transaction Request response.
///
/// Returned when a wallet calls the callback URL from a Solana Pay QR code.
/// Contains a base64-encoded serialized transaction for the wallet to sign and submit.
#[derive(Debug, serde::Serialize)]
pub struct DepositTxResponse {
    /// Base64-encoded serialized transaction (unsigned — wallet adds signature).
    pub transaction: String,
    /// Human-readable message shown in the wallet confirmation UI.
    pub message: String,
}

/// Query parameters for checking deposit confirmation.
#[derive(Debug, serde::Deserialize)]
pub struct ConfirmDepositQuery {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID.
    pub attendee_id: String,
}

/// Response for deposit confirmation check.
#[derive(Debug, serde::Serialize)]
pub struct ConfirmDepositResponse {
    /// Whether the deposit has been confirmed on-chain.
    pub confirmed: bool,
    /// Transaction signature if confirmed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_signature: Option<String>,
    /// Solana Pay URL to retry (if not yet confirmed and TX not sent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solana_pay_url: Option<String>,
}

/// Helius webhook payload for transaction notification.
/// See: https://docs.helius.dev/webhooks/webhook-payload
#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct HeliusWebhookPayload {
    /// Array of transaction notifications.
    #[serde(default)]
    pub data: Vec<HeliusTransactionData>,
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
pub struct HeliusTransactionData {
    /// Transaction signature.
    pub signature: String,
    /// Transaction type.
    #[serde(default)]
    pub r#type: String,
    /// Description (human-readable).
    #[serde(default)]
    pub description: String,
}

/// Response body for updating deposit status with TX signature.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UpdateDepositSignatureRequest {
    /// Event ID.
    pub event_id: String,
    /// Attendee API ID.
    pub attendee_id: String,
    /// On-chain transaction signature.
    pub tx_signature: String,
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Check if the deposit deadline has passed and auto-switch participation_type.
/// Returns `true` if the deadline expired and the switch was performed.
pub(crate) async fn check_and_switch_deadline(
    state: &AppState,
    kv: &worker::KvStore,
    event: &EventConfig,
    attendee: &event_checkin_domain::models::attendee::Attendee,
    registration_date_str: &str,
) -> bool {
    let Some(deadline_hours) = event.deposit_deadline_hours else {
        return false;
    };

    // Parse registration_date (ISO 8601)
    let reg_time = match chrono::DateTime::parse_from_rfc3339(registration_date_str) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(e) => {
            tracing::warn!(
                attendee_id = %attendee.api_id,
                error = %e,
                raw = registration_date_str,
                "deposit deadline: failed to parse registration_date"
            );
            return false;
        }
    };

    let deadline = reg_time + chrono::Duration::hours(i64::from(deadline_hours));
    let now = Utc::now();

    if now <= deadline {
        return false; // Still within deadline
    }

    // Deadline passed — auto-switch participation_type in the sheet
    tracing::info!(
        attendee_id = %attendee.api_id,
        event_id = %event.id,
        deadline = %deadline.to_rfc3339(),
        "deposit deadline expired: auto-switching participation_type to Online"
    );

    // Get column mapping for the sheet
    let mapping = match crate::sheets::get_column_mapping(
        state,
        &event.sheet_id,
        &event.sheet_name,
        Some(kv),
    )
    .await
    {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "deadline switch: failed to get column mapping");
            return true; // Deadline expired even if we can't switch
        }
    };

    if let Some(ctx) = &state.worker_ctx {
        ctx.wait_until(crate::sheets::bg_sync::update_participation_type(
            state.clone(),
            attendee.row_index,
            "Online".to_string(),
            mapping,
            event.sheet_id.clone(),
            event.sheet_name.clone(),
            Some(kv.clone()),
        ));
        tracing::info!(
            attendee_id = %attendee.api_id,
            "deposit deadline: participation_type switched to Online (bg)"
        );
    } else {
        match crate::sheets::write::update_participation_type(
            attendee.row_index,
            "Online",
            &mapping,
            state,
            &event.sheet_id,
            &event.sheet_name,
            Some(kv),
        )
        .await
        {
            Ok(()) => tracing::info!(
                attendee_id = %attendee.api_id,
                "deposit deadline: participation_type switched to Online"
            ),
            Err(e) => tracing::warn!(
                attendee_id = %attendee.api_id,
                error = %e,
                "deposit deadline: failed to update sheet participation_type"
            ),
        }
    }

    true
}

/// Check if in-person capacity is still available for the event.
/// Returns `true` if spots are available (or unlimited), `false` if full.
pub(crate) async fn check_in_person_capacity(
    state: &AppState,
    event: &EventConfig,
    kv: &worker::KvStore,
) -> bool {
    // No capacity limit = always available (handled by has_in_person_capacity)
    if event.in_person_capacity.is_none() {
        return true;
    }

    // Count in-person attendees from sheet
    let attendees = match crate::sheets::get_attendees_for_event(
        state,
        &event.sheet_id,
        &event.sheet_name,
        Some(kv),
        &event.id,
    )
    .await
    {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(error = %e, "reclaim capacity: failed to get attendees");
            return false; // Assume full on error
        }
    };

    let in_person_count = attendees.iter().filter(|a| a.is_in_person()).count() as u32;

    // Count walk-in attendees as in-person
    let walkin_prefix = format!("walkin:{}:", event.id);
    let mut walkin_count: u32 = 0;
    let mut cursor: Option<String> = None;
    loop {
        let mut builder = kv.list().prefix(walkin_prefix.clone());
        if let Some(c) = cursor.take() {
            builder = builder.cursor(c);
        }
        match builder.execute().await {
            Ok(resp) => {
                walkin_count += resp.keys.len() as u32;
                if resp.list_complete {
                    break;
                }
                cursor = resp.cursor;
            }
            Err(e) => {
                tracing::warn!(error = ?e, "reclaim capacity: failed to list walk-in keys");
                break;
            }
        }
    }

    let total = in_person_count + walkin_count;
    event.has_in_person_capacity(total)
}

/// Verify a transaction signature on-chain by checking its confirmation status via RPC.
pub(crate) async fn verify_tx_on_chain(rpc_url: &str, signature: &str) -> bool {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "bethere-confirm",
        "method": "getSignatureStatuses",
        "params": [
            [signature],
            { "searchTransactionHistory": true }
        ]
    });

    let json_body = match serde_json::to_string(&body) {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("failed to serialize signature status request: {e}");
            return false;
        }
    };

    let headers = worker::Headers::new();
    if let Err(e) = headers.set("Content-Type", "application/json") {
        tracing::error!("failed to set header: {e:?}");
        return false;
    }

    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post)
        .with_headers(headers)
        .with_body(Some(wasm_bindgen::JsValue::from_str(&json_body)));

    let request = match worker::Request::new_with_init(rpc_url, &init) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("failed to create RPC request: {e:?}");
            return false;
        }
    };

    let mut response = match worker::Fetch::Request(request).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("RPC request failed: {e:?}");
            return false;
        }
    };

    let body_text = match response.text().await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("failed to read RPC response: {e:?}");
            return false;
        }
    };

    // Parse the response: { result: { value: [ { confirmationStatus: "confirmed" | "finalized", err: null } ] } }
    let parsed: serde_json::Value = match serde_json::from_str(&body_text) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("failed to parse RPC response: {e}");
            return false;
        }
    };

    // Navigate to result.value[0]
    let status_opt = parsed
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .cloned();

    let Some(status) = status_opt else {
        tracing::debug!(
            tx_signature = %signature,
            "getSignatureStatuses returned null — TX not found (may have expired from status cache)"
        );
        return false;
    };

    // Null status entry means TX not found
    if status.is_null() {
        tracing::debug!(
            tx_signature = %signature,
            "Signature status is null — TX not found on-chain"
        );
        return false;
    }

    // Check if there's an error
    if status.get("err").is_some_and(|e| !e.is_null()) {
        tracing::warn!(tx_signature = %signature, "TX failed on-chain: {:?}", status.get("err"));
        return false;
    }

    // Check confirmation status
    let confirmation = status
        .get("confirmationStatus")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");
    let confirmed = confirmation == "confirmed" || confirmation == "finalized";
    tracing::debug!(
        tx_signature = %signature,
        confirmation_status = %confirmation,
        confirmed = %confirmed,
        "Signature status checked"
    );
    confirmed
}

/// Background task: verify deposit TX on-chain and update status (H8).
///
/// Detached via `wait_until` so the webhook response returns immediately.
/// Owns all data — no borrows on the handler's stack.
pub(crate) async fn verify_and_confirm_deposit(
    state: &AppState,
    body: &UpdateDepositSignatureRequest,
) {
    let Some(kv) = state.events_kv.as_ref() else {
        tracing::error!("EVENTS KV not configured in background verification");
        return;
    };

    let rpc_url = state.config.solana.full_rpc_url();
    let confirmed = verify_tx_on_chain(&rpc_url, &body.tx_signature).await;

    if confirmed {
        // Reload and update deposit status
        match event_store::get_deposit_status(kv, &body.event_id, &body.attendee_id).await {
            Ok(Some(mut deposit_status)) => {
                deposit_status.verified = true;
                if let Err(e) = event_store::save_deposit_status(kv, &deposit_status).await {
                    tracing::error!(
                        attendee_id = %body.attendee_id,
                        error = %e,
                        "failed to save verified deposit status in background"
                    );
                    return;
                }

                tracing::info!(
                    attendee_id = %body.attendee_id,
                    tx_signature = %body.tx_signature,
                    "USDC deposit verified in background"
                );

                // Audit log
                let _ = crate::audit_store::append_event_audit(
                    kv,
                    &body.event_id,
                    crate::audit_store::create_entry_with_meta(
                        "system",
                        crate::audit_store::AuditAction::DepositConfirmed,
                        &body.attendee_id,
                        "USDC deposit confirmed on-chain (background)",
                        serde_json::json!({
                            "tx_signature": body.tx_signature,
                            "confirmed": true,
                        }),
                    ),
                    state.d1.as_deref(),
                )
                .await;

                // Write deposit columns to sheet + auto-generate QR (non-blocking)
                let deposit_amount_str = deposit_status.amount.to_string();
                // Can't collapse: inner lookups depend on event_config bound by this pattern
                #[allow(clippy::collapsible_if)]
                if let Ok(Some(event_config)) =
                    event_store::get_event_config(kv, &body.event_id).await
                {
                    let (mapping_result, attendee_result) = futures_util::join!(
                        crate::sheets::get_column_mapping(
                            state,
                            &event_config.sheet_id,
                            &event_config.sheet_name,
                            Some(kv),
                        ),
                        crate::sheets::get_attendee_by_id(
                            &body.attendee_id,
                            state,
                            &event_config.sheet_id,
                            &event_config.sheet_name,
                            Some(kv),
                        )
                    );
                    if let (Ok(mapping), Ok(Some(attendee))) = (mapping_result, attendee_result) {
                        let ctx = crate::sheets::write::SheetContext {
                            mapping: &mapping,
                            state,
                            sheet_id: &event_config.sheet_id,
                            sheet_name: &event_config.sheet_name,
                            kv: Some(kv),
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
                            tracing::warn!(error = %e, "failed to write deposit verification to sheet in background");
                        }
                        if attendee.qr_code_url.as_ref().is_none_or(|u| u.is_empty()) {
                            let server_url = &state.config.server.url;
                            let qr_url = format!("{server_url}/staff/?scan={}", attendee.api_id);
                            if let Err(e) = crate::sheets::write::update_qr_urls(
                                &[(attendee.row_index, qr_url)],
                                &mapping,
                                state,
                                &event_config.sheet_id,
                                &event_config.sheet_name,
                                Some(kv),
                            )
                            .await
                            {
                                tracing::warn!(error = %e, "failed to auto-generate QR in background");
                            }
                        }
                    }
                }
            }
            Ok(None) => {
                tracing::warn!(
                    attendee_id = %body.attendee_id,
                    "deposit record disappeared before background verification"
                );
            }
            Err(e) => {
                tracing::error!(
                    attendee_id = %body.attendee_id,
                    error = %e,
                    "failed to reload deposit status for background verification"
                );
            }
        }
    } else {
        tracing::info!(
            attendee_id = %body.attendee_id,
            tx_signature = %body.tx_signature,
            "USDC deposit not yet confirmed on-chain (background check)"
        );
    }
}

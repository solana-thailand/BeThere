//! POST /api/deposit/usdc/webhook — Helius webhook for TX confirmation

use axum::{Json, extract::State, http::HeaderMap};

use crate::error::{ApiOk, WorkerError};
use crate::event_store;
use crate::handlers::deposit::usdc::{UpdateDepositSignatureRequest, verify_and_confirm_deposit};
use crate::state::AppState;

/// Dual-purpose endpoint for recording USDC deposit TX signatures.
///
/// Called by:
/// 1. **Frontend** after a wallet sends a deposit TX — sends JWT Bearer token
/// 2. **Helius** when a monitored TX is confirmed — sends `WEBHOOK_SECRET` Bearer token
///
/// **Authentication**: Accepts either a valid `WEBHOOK_SECRET` Bearer token or
/// a valid JWT. Rejects if `WEBHOOK_SECRET` is not configured and the Bearer
/// token doesn't parse as a valid JWT (VULN-001 remediation).
#[worker::send]
pub async fn deposit_webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<UpdateDepositSignatureRequest>,
) -> Result<ApiOk<serde_json::Value>, WorkerError> {
    // Dual auth: accept either WEBHOOK_SECRET or valid JWT
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let is_webhook_authed = if !state.webhook_secret.is_empty() {
        let expected = format!("Bearer {}", state.webhook_secret);
        auth_header == expected
    } else {
        false
    };

    let is_jwt_authed = if !is_webhook_authed {
        // Try JWT verification
        let token = auth_header
            .strip_prefix("Bearer ")
            .map(|s| Some(s.to_string()))
            .unwrap_or(None);
        match crate::auth::verify_token(&token, &state).await {
            Ok(_) => true,
            Err(e) => {
                tracing::debug!(
                    error = %e,
                    "deposit webhook: JWT verification failed"
                );
                false
            }
        }
    } else {
        false // already authed via webhook secret, skip JWT check
    };

    if !is_webhook_authed && !is_jwt_authed {
        // Never log `auth_header` itself. A rejected header is still credential
        // material — a near-miss `WEBHOOK_SECRET` or a JWT that is valid
        // somewhere else — and Cloudflare request logs are a far weaker
        // trust boundary than the secret store. Log only its shape.
        tracing::warn!(
            auth_present = !auth_header.is_empty(),
            auth_is_bearer = auth_header.starts_with("Bearer "),
            auth_len = auth_header.len(),
            "deposit webhook rejected: no valid webhook secret or JWT"
        );
        return Err(event_checkin_domain::models::error::AppError::Unauthorized(
            "invalid or missing Authorization header".to_string(),
        )
        .into());
    }

    let kv = state.events_kv.as_ref();
    let d1 = state.d1.as_deref();

    // Get existing deposit status
    let mut deposit_status =
        event_store::get_deposit_status_with_fallback(kv, d1, &body.event_id, &body.attendee_id)
            .await
            .map_err(event_checkin_domain::models::error::AppError::Internal)?
            .ok_or_else(|| {
                event_checkin_domain::models::error::AppError::NotFound(format!(
                    "no deposit record for attendee '{}' in event '{}'",
                    body.attendee_id, body.event_id
                ))
            })?;

    // Refuse to rewrite the signature of an already-verified deposit. The JWT
    // branch above authenticates the *caller* but does not bind them to
    // `body.attendee_id`, so any holder of a valid token could otherwise
    // overwrite a settled money record's proof-of-payment with an arbitrary
    // string — the `verified` flag would survive while the signature backing
    // it, and every refund/audit path that reads it, would not. Re-sending the
    // same signature stays a no-op success so Helius retries are safe.
    if deposit_status.verified {
        let recorded = deposit_status.tx_signature.as_deref().unwrap_or("");
        if !recorded.is_empty() && recorded != body.tx_signature {
            tracing::warn!(
                attendee_id = %body.attendee_id,
                event_id = %body.event_id,
                recorded_tx_signature = %recorded,
                incoming_tx_signature = %body.tx_signature,
                "deposit webhook refused: deposit already verified with a different TX signature"
            );
        }
        return Ok(ApiOk::new(serde_json::json!({
            "success": true,
            "confirmed": true,
            "tx_signature": recorded,
        })));
    }

    // Update with TX signature
    deposit_status.tx_signature = Some(body.tx_signature.clone());

    // Save immediately so the frontend sees the TX signature (H8: verification detached)
    event_store::save_deposit_status_with_fallback(kv, d1, &deposit_status)
        .await
        .map_err(event_checkin_domain::models::error::AppError::Internal)?;

    tracing::info!(
        attendee_id = %body.attendee_id,
        tx_signature = %body.tx_signature,
        "USDC deposit TX signature recorded, pending on-chain verification"
    );

    // Detach on-chain verification — response returns immediately (H8)
    // If verified, updates deposit status + audit log in background via wait_until.
    if let Some(ctx) = &state.worker_ctx {
        let verify_state = state.clone();
        let verify_body = body.clone();
        ctx.wait_until(async move {
            verify_and_confirm_deposit(&verify_state, &verify_body).await;
        });
    } else {
        tracing::warn!(
            attendee_id = %body.attendee_id,
            "no worker_ctx available — skipping detached on-chain verification"
        );
    }

    Ok(ApiOk::new(serde_json::json!({
        "success": true,
        "confirmed": false, // pending — will be verified in background
        "tx_signature": body.tx_signature,
    })))
}

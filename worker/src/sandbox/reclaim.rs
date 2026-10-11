//! Organizer-rent reclaim (SG3): each sandbox event locks the organizer's
//! rent in its escrow and vault accounts. The hourly cron closes the ones that
//! are done and gives the rent back to the organizer key.
//!
//! An event qualifies only when both hold, so no one's deposit is touched:
//! - its two-minute run ended a margin ago (its `sandbox:event:{id}` quota key
//!   was claimed at creation and lives 24 h, so the key gives the time);
//! - its vault holds no test USDC (refunded, or never deposited).
//!
//! The organizer key signs `deactivate_event` then `close_event`. A closed
//! event gets a `sandbox:closed:{id}` key so it is not tried again. A failed
//! read or send is tried again next hour, until the quota key expires.

use base64::Engine;
use worker::{D1Database, D1Type};

use super::config::{EVENT_SECONDS, SandboxKeys};
use super::keys::SandboxSigner;
use super::send::{send_and_confirm, token_balance};
use crate::db::d1_safe::safe_all_rows;
use crate::solana_escrow::tx_builders::EscrowCtx;
use crate::solana_escrow::{
    build_close_event_transaction, build_deactivate_event_transaction, pubkey_to_base58,
};

/// After the run ends, wait this long before reclaiming.
pub const RECLAIM_MARGIN_SECONDS: i64 = 300;
/// Events closed per sweep (each is two signed transactions).
pub const SWEEP_MAX: u32 = 5;

/// Sandbox events whose run ended a margin ago and that are not closed yet,
/// oldest first. ?1 = `+{seconds after creation} seconds`, ?2 = the limit.
pub const DUE_SQL: &str = "SELECT substr(l.lock_key, 15) AS id FROM advisory_locks l \
     WHERE l.lock_key >= 'sandbox:event:' AND l.lock_key < 'sandbox:event;' \
       AND l.expires_at > datetime('now') \
       AND datetime(l.expires_at, '-86400 seconds', ?1) < datetime('now') \
       AND NOT EXISTS (SELECT 1 FROM advisory_locks c \
                       WHERE c.lock_key = 'sandbox:closed:' || substr(l.lock_key, 15) \
                         AND c.expires_at > datetime('now')) \
     ORDER BY l.expires_at LIMIT ?2";
/// Remember a closed event for longer than its quota key lives.
pub const MARK_SQL: &str = "INSERT INTO advisory_locks (lock_key, expires_at) \
     VALUES ('sandbox:closed:' || ?1, datetime('now', '+2 days')) \
     ON CONFLICT(lock_key) DO UPDATE SET expires_at = excluded.expires_at";

/// What one sweep did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Sweep {
    pub due: u32,
    pub closed: u32,
    /// A deposit still in the vault: left alone.
    pub holding: u32,
    pub failed: u32,
}

async fn sign_send(rpc_url: &str, signer: &SandboxSigner, tx_b64: &str) -> Result<String, String> {
    let mut tx = base64::engine::general_purpose::STANDARD
        .decode(tx_b64)
        .map_err(|e| format!("bad base64: {e}"))?;
    signer
        .sign_transaction(&mut tx)
        .map_err(|e| e.to_string())?;
    send_and_confirm(rpc_url, &tx)
        .await
        .map_err(|e| e.to_string())
}

/// Close one event if its vault is empty. `Ok(true)` closed, `Ok(false)`
/// still holding a deposit.
async fn reclaim_one(rpc_url: &str, keys: &SandboxKeys, id: u64) -> Result<bool, String> {
    let organizer = keys.organizer.address();
    let ctx = EscrowCtx::resolve(organizer, id)
        .await
        .map_err(|e| e.to_string())?;
    let held = token_balance(rpc_url, &pubkey_to_base58(&ctx.vault))
        .await
        .map_err(|e| e.to_string())?;
    if held > 0 {
        return Ok(false);
    }
    // An earlier sweep may have deactivated it and then failed to close: a
    // failed deactivate is logged, and the close decides.
    match build_deactivate_event_transaction(rpc_url, organizer, id).await {
        Ok(tx) => {
            if let Err(e) = sign_send(rpc_url, &keys.organizer, &tx.transaction_b64).await {
                tracing::info!(id, error = %e, "sandbox reclaim: deactivate not sent");
            }
        }
        Err(e) => tracing::info!(id, error = %e, "sandbox reclaim: deactivate not built"),
    }
    let close = build_close_event_transaction(rpc_url, organizer, id)
        .await
        .map_err(|e| e.to_string())?;
    sign_send(rpc_url, &keys.organizer, &close.transaction_b64).await?;
    Ok(true)
}

/// Close up to `max` finished sandbox events.
pub async fn sweep(db: &D1Database, rpc_url: &str, keys: &SandboxKeys, max: u32) -> Sweep {
    let after = format!("+{} seconds", EVENT_SECONDS + RECLAIM_MARGIN_SECONDS);
    let due = match db
        .prepare(DUE_SQL)
        .bind_refs(&[D1Type::Text(&after), D1Type::Integer(max as i32)])
    {
        Ok(stmt) => safe_all_rows(&stmt).await.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "sandbox reclaim: due read failed");
            Vec::new()
        }),
        Err(e) => {
            tracing::warn!(error = ?e, "sandbox reclaim: due bind failed");
            Vec::new()
        }
    };
    let mut out = Sweep {
        due: u32::try_from(due.len()).unwrap_or(u32::MAX),
        ..Sweep::default()
    };
    for row in &due {
        let id_text = row.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        let Ok(id) = id_text.parse::<u64>() else {
            out.failed += 1;
            continue;
        };
        match reclaim_one(rpc_url, keys, id).await {
            Ok(true) => {
                out.closed += 1;
                if let Ok(stmt) = db.prepare(MARK_SQL).bind_refs(&[D1Type::Text(id_text)])
                    && let Err(e) = stmt.run().await
                {
                    tracing::warn!(id, error = ?e, "sandbox reclaim: closed but not marked");
                }
            }
            Ok(false) => out.holding += 1,
            Err(e) => {
                out.failed += 1;
                tracing::warn!(id, error = %e, "sandbox reclaim failed; next hour");
            }
        }
    }
    out
}

/// The devnet RPC for the cron path, which builds no `AppState`.
pub fn devnet_rpc_from_env(env: &worker::Env) -> String {
    event_checkin_domain::config::SolanaConfig {
        rpc_url: event_checkin_domain::config::DEVNET_RPC.to_string(),
        api_key: env
            .secret("HELIUS_API_KEY")
            .map(|s| s.to_string())
            .unwrap_or_default(),
        crossmint_host: String::new(),
        crossmint_api_key: String::new(),
        crossmint_collection_id: String::new(),
    }
    .devnet_rpc_url()
}

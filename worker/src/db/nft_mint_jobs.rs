//! Durable NFT mint journal used to recover post-mint projection failures.

use worker::D1Database;
use worker::d1::D1Type;

use crate::db::d1_safe;

#[derive(Debug, Clone)]
pub struct ConfirmedMint {
    pub asset_id: String,
    pub signature: String,
}

/// Create the intent before external I/O and return an earlier confirmed result
/// when this claim is being retried.
pub async fn begin_or_resume(
    db: &D1Database,
    event_id: &str,
    claim_token: &str,
    wallet: &str,
    provider_mint_id: &str,
) -> Result<Option<ConfirmedMint>, String> {
    db.prepare(
        "INSERT INTO nft_mint_jobs (event_id,claim_token,wallet,provider_mint_id,status) \
         VALUES (?1,?2,?3,?4,'pending') ON CONFLICT(event_id,claim_token) DO NOTHING",
    )
    .bind_refs(&[
        D1Type::Text(event_id),
        D1Type::Text(claim_token),
        D1Type::Text(wallet),
        D1Type::Text(provider_mint_id),
    ])
    .map_err(|error| format!("D1 mint job begin bind: {error:?}"))?
    .run()
    .await
    .map_err(|error| format!("D1 mint job begin: {error:?}"))?;

    let statement = db
        .prepare(
            "SELECT wallet,provider_mint_id,status,asset_id,signature FROM nft_mint_jobs \
             WHERE event_id=?1 AND claim_token=?2",
        )
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(claim_token)])
        .map_err(|error| format!("D1 mint job read bind: {error:?}"))?;
    let rows = d1_safe::safe_all_rows(&statement).await?;
    let row = rows
        .first()
        .ok_or_else(|| "mint job disappeared after insert".to_string())?;
    let stored_wallet = row
        .get("wallet")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if !stored_wallet.eq_ignore_ascii_case(wallet) {
        return Err("mint job wallet does not match the original claim wallet".to_string());
    }
    if row.get("provider_mint_id").and_then(|value| value.as_str()) != Some(provider_mint_id) {
        return Err("mint job provider id does not match the original request".to_string());
    }
    let confirmed = matches!(
        row.get("status").and_then(|value| value.as_str()),
        Some("confirmed" | "persisted")
    );
    if !confirmed {
        return Ok(None);
    }
    let asset_id = row
        .get("asset_id")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "confirmed mint job is missing asset_id".to_string())?;
    Ok(Some(ConfirmedMint {
        asset_id: asset_id.to_string(),
        signature: row
            .get("signature")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string(),
    }))
}

pub async fn mark_confirmed(
    db: &D1Database,
    event_id: &str,
    claim_token: &str,
    asset_id: &str,
    signature: &str,
) -> Result<(), String> {
    let result = db
        .prepare(
            "UPDATE nft_mint_jobs SET status='confirmed',asset_id=?1,signature=?2, \
             updated_at=datetime('now') WHERE event_id=?3 AND claim_token=?4",
        )
        .bind_refs(&[
            D1Type::Text(asset_id),
            D1Type::Text(signature),
            D1Type::Text(event_id),
            D1Type::Text(claim_token),
        ])
        .map_err(|error| format!("D1 mint job confirm bind: {error:?}"))?
        .run()
        .await
        .map_err(|error| format!("D1 mint job confirm: {error:?}"))?;
    let changes = result
        .meta()
        .ok()
        .flatten()
        .and_then(|meta| meta.changes)
        .unwrap_or(0);
    if changes == 0 {
        return Err("mint job confirm updated no row".to_string());
    }
    Ok(())
}

pub async fn mark_persisted(
    db: &D1Database,
    event_id: &str,
    claim_token: &str,
) -> Result<(), String> {
    let result = db
        .prepare(
            "UPDATE nft_mint_jobs SET status='persisted',updated_at=datetime('now') \
             WHERE event_id=?1 AND claim_token=?2 AND status IN ('confirmed','persisted')",
        )
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(claim_token)])
        .map_err(|error| format!("D1 mint job persist bind: {error:?}"))?
        .run()
        .await
        .map_err(|error| format!("D1 mint job persist: {error:?}"))?;
    let changes = result
        .meta()
        .ok()
        .flatten()
        .and_then(|meta| meta.changes)
        .unwrap_or(0);
    if changes == 0 {
        return Err("mint job persist updated no confirmed row".to_string());
    }
    Ok(())
}

//! Durable NFT mint journal used to recover post-mint projection failures.

use worker::D1Database;
use worker::d1::D1Type;

use crate::db::d1_safe;

const REPAIR_ATTENDEE_PROJECTIONS_SQL: &str = "UPDATE attendees AS a SET \
     claimed_at=(SELECT j.updated_at FROM nft_mint_jobs j \
                 WHERE j.event_id=a.event_id AND j.claim_token=a.claim_token \
                   AND j.status='confirmed'), \
     claim_asset_id=(SELECT j.asset_id FROM nft_mint_jobs j \
                     WHERE j.event_id=a.event_id AND j.claim_token=a.claim_token \
                       AND j.status='confirmed'), \
     claim_signature=(SELECT j.signature FROM nft_mint_jobs j \
                      WHERE j.event_id=a.event_id AND j.claim_token=a.claim_token \
                        AND j.status='confirmed'), \
     updated_at=datetime('now') \
     WHERE a.claimed_at IS NULL AND a.claim_asset_id IS NULL \
       AND a.claim_signature IS NULL \
       AND EXISTS (SELECT 1 FROM nft_mint_jobs j \
                   WHERE j.event_id=a.event_id AND j.claim_token=a.claim_token \
                     AND j.status='confirmed' AND j.asset_id IS NOT NULL)";

const MARK_MATCHING_JOBS_PERSISTED_SQL: &str = "UPDATE nft_mint_jobs AS j SET status='persisted',updated_at=datetime('now') \
     WHERE j.status='confirmed' AND j.asset_id IS NOT NULL \
       AND EXISTS (SELECT 1 FROM attendees a \
                   WHERE a.event_id=j.event_id AND a.claim_token=j.claim_token \
                     AND a.claimed_at IS NOT NULL \
                     AND a.claim_asset_id=j.asset_id \
                     AND coalesce(a.claim_signature,'')=coalesce(j.signature,''))";

#[derive(Debug, Clone)]
pub struct ConfirmedMint {
    pub asset_id: String,
    pub signature: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReconcileReport {
    pub attendee_projections_repaired: usize,
    pub jobs_marked_persisted: usize,
    pub confirmed_remaining: i64,
    pub stale_pending: i64,
}

impl ReconcileReport {
    pub fn is_clean(&self) -> bool {
        self.confirmed_remaining == 0 && self.stale_pending == 0
    }
}

fn changes(result: &worker::D1Result) -> usize {
    result
        .meta()
        .ok()
        .flatten()
        .and_then(|meta| meta.changes)
        .unwrap_or(0)
}

async fn count(db: &D1Database, sql: &str) -> Result<i64, String> {
    let rows = d1_safe::safe_all_rows(&db.prepare(sql)).await?;
    Ok(rows
        .first()
        .and_then(|row| row.get("n"))
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0))
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

/// Repair confirmed external mints whose attendee projection did not finish.
///
/// Both statements are idempotent. The first only fills an entirely unclaimed
/// attendee, so it cannot overwrite an existing NFT. The second advances a job
/// only after the attendee contains the exact journaled result. If the cron dies
/// between statements, the next run safely finishes the transition.
pub async fn reconcile(db: &D1Database) -> Result<ReconcileReport, String> {
    let projected = db
        .prepare(REPAIR_ATTENDEE_PROJECTIONS_SQL)
        .run()
        .await
        .map_err(|error| format!("D1 mint reconcile attendee projection: {error:?}"))?;

    let persisted = db
        .prepare(MARK_MATCHING_JOBS_PERSISTED_SQL)
        .run()
        .await
        .map_err(|error| format!("D1 mint reconcile journal persist: {error:?}"))?;

    Ok(ReconcileReport {
        attendee_projections_repaired: changes(&projected),
        jobs_marked_persisted: changes(&persisted),
        confirmed_remaining: count(
            db,
            "SELECT COUNT(*) AS n FROM nft_mint_jobs WHERE status='confirmed'",
        )
        .await?,
        stale_pending: count(
            db,
            "SELECT COUNT(*) AS n FROM nft_mint_jobs \
             WHERE status='pending' AND updated_at < datetime('now','-1 hour')",
        )
        .await?,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::process::{Command, Stdio};

    use super::{MARK_MATCHING_JOBS_PERSISTED_SQL, REPAIR_ATTENDEE_PROJECTIONS_SQL};

    #[test]
    fn reconciliation_repairs_only_unclaimed_matching_attendees() {
        if !Command::new("sqlite3")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
        {
            eprintln!("sqlite3 not on PATH; skipping executed SQL test");
            return;
        }
        let mut child = Command::new("sqlite3")
            .arg(":memory:")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn sqlite3");
        let sql = format!(
            "CREATE TABLE attendees(event_id TEXT,claim_token TEXT,claimed_at TEXT,claim_asset_id TEXT,claim_signature TEXT,updated_at TEXT);\n\
             CREATE TABLE nft_mint_jobs(event_id TEXT,claim_token TEXT,status TEXT,asset_id TEXT,signature TEXT,updated_at TEXT);\n\
             INSERT INTO attendees VALUES\
               ('e','repair',NULL,NULL,NULL,'old'),\
               ('e','keep','already','ORIGINAL','SIG0','old'),\
               ('e','orphan',NULL,NULL,NULL,'old');\n\
             INSERT INTO nft_mint_jobs VALUES\
               ('e','repair','confirmed','ASSET1','SIG1','confirmed-at'),\
               ('e','keep','confirmed','ASSET2','SIG2','confirmed-at');\n\
             {REPAIR_ATTENDEE_PROJECTIONS_SQL};\n\
             {MARK_MATCHING_JOBS_PERSISTED_SQL};\n\
             SELECT claim_token||'|'||coalesce(claimed_at,'~')||'|'||coalesce(claim_asset_id,'~') FROM attendees ORDER BY claim_token;\n\
             SELECT claim_token||'|'||status FROM nft_mint_jobs ORDER BY claim_token;"
        );
        child
            .stdin
            .take()
            .unwrap()
            .write_all(sql.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "keep|already|ORIGINAL\norphan|~|~\nrepair|confirmed-at|ASSET1\nkeep|confirmed\nrepair|persisted\n"
        );
    }
}

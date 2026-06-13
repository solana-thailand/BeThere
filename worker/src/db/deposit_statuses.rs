//! D1 deposit status query helpers.
//!
//! Deposit statuses stored exclusively in D1 (Phase 3e complete).

use worker::D1Database;
use worker::d1::D1Type;

use event_checkin_domain::models::deposit::{DepositMethod, DepositStatus};

// ---------------------------------------------------------------------------
// Read
// ---------------------------------------------------------------------------

/// Read a single deposit status by event + attendee. Returns `None` if not found.
///
/// Uses raw JS `.first()` + `JSON.stringify` → serde_json to bypass the worker crate's
/// `.first::<T>()` which crashes on `JsValue(null)` when no row is found.
pub async fn get_deposit_status(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
) -> Result<Option<DepositStatus>, String> {
    let stmt =
        db.prepare("SELECT * FROM deposit_statuses WHERE event_id = ?1 AND attendee_id = ?2");
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(attendee_id)])
        .map_err(|e| format!("D1 get_deposit_status bind: {e:?}"))?;

    // Bypass worker crate's .first::<T>() — crashes on JsValue(null).
    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 get_deposit_status first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_deposit_status first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    if json_str.is_empty() {
        return Ok(None);
    }

    let row: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(
            error = %e,
            json = %json_str.chars().take(300).collect::<String>(),
            "D1 get_deposit_status: deserialize failed"
        );
        format!("D1 get_deposit_status deserialize: {e}")
    })?;

    Ok(Some(row_to_deposit_status(row)?))
}

/// List all deposit statuses for an event (ordered by deposit_order).
pub async fn list_deposit_statuses(
    db: &D1Database,
    event_id: &str,
) -> Result<Vec<DepositStatus>, String> {
    let stmt =
        db.prepare("SELECT * FROM deposit_statuses WHERE event_id = ?1 ORDER BY deposit_order ASC");
    let result = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 list_deposit_statuses bind: {e:?}"))?
        .all()
        .await
        .map_err(|e| format!("D1 list_deposit_statuses: {e:?}"))?;

    let rows: Vec<serde_json::Value> = result
        .results()
        .map_err(|e| format!("D1 list_deposit_statuses results: {e:?}"))?;

    rows.into_iter()
        .map(row_to_deposit_status)
        .collect::<Result<Vec<_>, _>>()
}

/// Find attendee API ID by wallet address within a specific event's deposit records.
///
/// Uses raw JS `.first()` + `JSON.stringify` → serde_json to bypass the worker crate's
/// `.first::<T>()` which crashes on `JsValue(null)` when no row is found.
pub async fn find_attendee_by_wallet(
    db: &D1Database,
    event_id: &str,
    wallet_address: &str,
) -> Result<Option<String>, String> {
    let stmt = db.prepare(
        "SELECT attendee_id FROM deposit_statuses WHERE event_id = ?1 AND wallet_address = ?2 LIMIT 1",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(wallet_address)])
        .map_err(|e| format!("D1 find_attendee_by_wallet bind: {e:?}"))?;

    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 find_attendee_by_wallet first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 find_attendee_by_wallet first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    if json_str.is_empty() {
        return Ok(None);
    }

    let row: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(
            error = %e,
            json = %json_str.chars().take(300).collect::<String>(),
            "D1 find_attendee_by_wallet: deserialize failed"
        );
        format!("D1 find_attendee_by_wallet deserialize: {e}")
    })?;

    Ok(row
        .get("attendee_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string()))
}

/// Count deposits for an event (for deposit counter).
pub async fn count_deposits_by_event(db: &D1Database, event_id: &str) -> Result<u32, String> {
    let stmt = db.prepare("SELECT COUNT(*) as cnt FROM deposit_statuses WHERE event_id = ?1");
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 count_deposits bind: {e:?}"))?;

    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 count_deposits first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 count_deposits first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(0);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    let row: serde_json::Value = serde_json::from_str(&json_str).unwrap_or_default();
    Ok(row.get("cnt").and_then(|v| v.as_i64()).unwrap_or(0) as u32)
}

// ---------------------------------------------------------------------------
// Write
// ---------------------------------------------------------------------------

/// Insert a new deposit status into D1.
/// Uses `db.exec()` with format string per project convention (avoids prepared stmt issues).
pub async fn insert_deposit_status(db: &D1Database, status: &DepositStatus) -> Result<(), String> {
    let method_str = status.method.to_string();
    let esc = |s: &str| s.replace('\'', "''");
    let null_or = |opt: &Option<String>| -> String {
        match opt {
            Some(v) if !v.is_empty() => format!("'{}'", esc(v)),
            _ => "NULL".to_string(),
        }
    };

    let sql = format!(
        "INSERT INTO deposit_statuses \
         (attendee_id, event_id, method, amount, currency, tx_signature, \
          verified, deposited_at, wallet_address, deposit_order, refundable, rejected) \
         VALUES ('{attendee_id}', '{event_id}', '{method}', {amount}, '{currency}', \
          {tx_signature}, {verified}, '{deposited_at}', {wallet_address}, \
          {deposit_order}, {refundable}, {rejected})",
        attendee_id = esc(&status.attendee_id),
        event_id = esc(&status.event_id),
        method = esc(&method_str),
        amount = status.amount,
        currency = esc(&status.currency),
        tx_signature = null_or(&status.tx_signature),
        verified = status.verified as i32,
        deposited_at = esc(&status.deposited_at),
        wallet_address = null_or(&status.wallet_address),
        deposit_order = status.deposit_order,
        refundable = status.refundable as i32,
        rejected = status.rejected as i32,
    );

    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 insert_deposit_status exec: {e:?}"))?;

    Ok(())
}

/// Update an existing deposit status (for verify/reject operations).
/// Uses `db.exec()` with format string per project convention (avoids prepared stmt issues).
pub async fn update_deposit_status(db: &D1Database, status: &DepositStatus) -> Result<(), String> {
    let method_str = status.method.to_string();
    let esc = |s: &str| s.replace('\'', "''");
    let null_or = |opt: &Option<String>| -> String {
        match opt {
            Some(v) if !v.is_empty() => format!("'{}'", esc(v)),
            _ => "NULL".to_string(),
        }
    };

    let sql = format!(
        "UPDATE deposit_statuses SET \
         method = '{method}', amount = {amount}, currency = '{currency}', \
         tx_signature = {tx_signature}, verified = {verified}, \
         deposited_at = '{deposited_at}', wallet_address = {wallet_address}, \
         deposit_order = {deposit_order}, refundable = {refundable}, rejected = {rejected} \
         WHERE event_id = '{event_id}' AND attendee_id = '{attendee_id}'",
        method = esc(&method_str),
        amount = status.amount,
        currency = esc(&status.currency),
        tx_signature = null_or(&status.tx_signature),
        verified = status.verified as i32,
        deposited_at = esc(&status.deposited_at),
        wallet_address = null_or(&status.wallet_address),
        deposit_order = status.deposit_order,
        refundable = status.refundable as i32,
        rejected = status.rejected as i32,
        event_id = esc(&status.event_id),
        attendee_id = esc(&status.attendee_id),
    );

    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 update_deposit_status exec: {e:?}"))?;

    Ok(())
}

/// Save (upsert) a deposit status — inserts or updates.
pub async fn save_deposit_status(db: &D1Database, status: &DepositStatus) -> Result<(), String> {
    let existing = get_deposit_status(db, &status.event_id, &status.attendee_id).await?;
    if existing.is_some() {
        update_deposit_status(db, status).await
    } else {
        insert_deposit_status(db, status).await
    }
}

/// Delete all deposit statuses for an event (cleanup).
pub async fn delete_deposit_statuses_for_event(
    db: &D1Database,
    event_id: &str,
) -> Result<(), String> {
    let stmt = db.prepare("DELETE FROM deposit_statuses WHERE event_id = ?1");
    stmt.bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 delete_deposit_statuses bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 delete_deposit_statuses run: {e:?}"))?;

    Ok(())
}

/// Delete a single deposit status by event + attendee (attendee deletion).
pub async fn delete_deposit_status(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
) -> Result<(), String> {
    let stmt = db.prepare("DELETE FROM deposit_statuses WHERE event_id = ?1 AND attendee_id = ?2");
    stmt.bind_refs(&[D1Type::Text(event_id), D1Type::Text(attendee_id)])
        .map_err(|e| format!("D1 delete_deposit_status bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 delete_deposit_status run: {e:?}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Row → Domain conversion
// ---------------------------------------------------------------------------

fn row_to_deposit_status(row: serde_json::Value) -> Result<DepositStatus, String> {
    let get_str = |field: &str| -> String {
        row.get(field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let get_opt_str = |field: &str| -> Option<String> {
        row.get(field)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    };
    let get_bool =
        |field: &str| -> bool { row.get(field).and_then(|v| v.as_i64()).unwrap_or(0) != 0 };

    let method_str = get_str("method");
    let method = match method_str.as_str() {
        "usdc" => DepositMethod::Usdc,
        "thb" => DepositMethod::Thb,
        "credit_thb" => DepositMethod::CreditThb,
        "credit_usdc" => DepositMethod::CreditUsdc,
        other => {
            return Err(format!("unknown DepositMethod: '{other}'"));
        }
    };

    Ok(DepositStatus {
        attendee_id: get_str("attendee_id"),
        event_id: get_str("event_id"),
        method,
        amount: row.get("amount").and_then(|v| v.as_i64()).unwrap_or(0) as u64,
        currency: get_str("currency"),
        tx_signature: get_opt_str("tx_signature"),
        verified: get_bool("verified"),
        deposited_at: get_str("deposited_at"),
        wallet_address: get_opt_str("wallet_address"),
        deposit_order: row
            .get("deposit_order")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as u32,
        refundable: row.get("refundable").and_then(|v| v.as_i64()).unwrap_or(1) != 0,
        rejected: get_bool("rejected"),
    })
}

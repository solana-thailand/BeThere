//! D1 THB deposit query helpers.
//!
//! THB deposits stored exclusively in D1 (Phase 3d complete).

use worker::D1Database;
use worker::d1::D1Type;

use event_checkin_domain::models::deposit::ThbDeposit;

// ---------------------------------------------------------------------------
// Read
// ---------------------------------------------------------------------------

/// Read a single THB deposit by event + attendee. Returns `None` if not found.
pub async fn get_thb_deposit(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
) -> Result<Option<ThbDeposit>, String> {
    let stmt = db.prepare("SELECT * FROM thb_deposits WHERE event_id = ?1 AND attendee_id = ?2");
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(attendee_id)])
        .map_err(|e| format!("D1 get_thb_deposit bind: {e:?}"))?;

    // Bypass worker crate's .first::<T>() — crashes on JsValue(null).
    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 get_thb_deposit first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_thb_deposit first() await: {e:?}"))?;

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
            "D1 get_thb_deposit: deserialize failed"
        );
        format!("D1 get_thb_deposit deserialize: {e}")
    })?;

    Ok(Some(row_to_thb_deposit(row)?))
}

/// List all THB deposits for an event (newest first).
pub async fn list_thb_deposits(db: &D1Database, event_id: &str) -> Result<Vec<ThbDeposit>, String> {
    let stmt =
        db.prepare("SELECT * FROM thb_deposits WHERE event_id = ?1 ORDER BY uploaded_at ASC");
    let result = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 list_thb_deposits bind: {e:?}"))?
        .all()
        .await
        .map_err(|e| format!("D1 list_thb_deposits: {e:?}"))?;

    let rows: Vec<serde_json::Value> = result
        .results()
        .map_err(|e| format!("D1 list_thb_deposits results: {e:?}"))?;

    rows.into_iter()
        .map(row_to_thb_deposit)
        .collect::<Result<Vec<_>, _>>()
}

// ---------------------------------------------------------------------------
// Write
// ---------------------------------------------------------------------------

/// Insert a new THB deposit into D1.
/// Uses `db.exec()` with format string per project convention (avoids prepared stmt issues).
pub async fn insert_thb_deposit(db: &D1Database, deposit: &ThbDeposit) -> Result<(), String> {
    let esc = |s: &str| s.replace('\'', "''");
    let null_or = |opt: &Option<String>| -> String {
        match opt {
            Some(v) if !v.is_empty() => format!("'{}'", esc(v)),
            _ => "NULL".to_string(),
        }
    };

    let sql = format!(
        "INSERT INTO thb_deposits \
         (attendee_id, event_id, amount_thb, slip_url, verified, verified_by, verified_at, \
          uploaded_at, refunded, refunded_at, attendee_name, bank_account, bank_name, \
          account_name, refund_proof_url) \
         VALUES ('{attendee_id}', '{event_id}', {amount_thb}, {slip_url}, {verified}, \
          {verified_by}, {verified_at}, '{uploaded_at}', {refunded}, {refunded_at}, \
          {attendee_name}, {bank_account}, {bank_name}, {account_name}, {refund_proof_url})",
        attendee_id = esc(&deposit.attendee_id),
        event_id = esc(&deposit.event_id),
        amount_thb = deposit.amount_thb,
        slip_url = null_or(&deposit.slip_url),
        verified = deposit.verified as i32,
        verified_by = null_or(&deposit.verified_by),
        verified_at = null_or(&deposit.verified_at),
        uploaded_at = esc(&deposit.uploaded_at),
        refunded = deposit.refunded as i32,
        refunded_at = null_or(&deposit.refunded_at),
        attendee_name = null_or(&deposit.attendee_name),
        bank_account = null_or(&deposit.bank_account),
        bank_name = null_or(&deposit.bank_name),
        account_name = null_or(&deposit.account_name),
        refund_proof_url = null_or(&deposit.refund_proof_url),
    );

    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 insert_thb_deposit exec: {e:?}"))?;

    Ok(())
}

/// Update an existing THB deposit (for verify / refund operations).
/// Uses `db.exec()` with format string per project convention (avoids prepared stmt issues).
pub async fn update_thb_deposit(db: &D1Database, deposit: &ThbDeposit) -> Result<(), String> {
    let esc = |s: &str| s.replace('\'', "''");
    let null_or = |opt: &Option<String>| -> String {
        match opt {
            Some(v) if !v.is_empty() => format!("'{}'", esc(v)),
            _ => "NULL".to_string(),
        }
    };

    let sql = format!(
        "UPDATE thb_deposits SET \
         amount_thb = {amount_thb}, slip_url = {slip_url}, verified = {verified}, \
         verified_by = {verified_by}, verified_at = {verified_at}, refunded = {refunded}, \
         refunded_at = {refunded_at}, attendee_name = {attendee_name}, \
         bank_account = {bank_account}, bank_name = {bank_name}, \
         account_name = {account_name}, refund_proof_url = {refund_proof_url} \
         WHERE event_id = '{event_id}' AND attendee_id = '{attendee_id}'",
        amount_thb = deposit.amount_thb,
        slip_url = null_or(&deposit.slip_url),
        verified = deposit.verified as i32,
        verified_by = null_or(&deposit.verified_by),
        verified_at = null_or(&deposit.verified_at),
        refunded = deposit.refunded as i32,
        refunded_at = null_or(&deposit.refunded_at),
        attendee_name = null_or(&deposit.attendee_name),
        bank_account = null_or(&deposit.bank_account),
        bank_name = null_or(&deposit.bank_name),
        account_name = null_or(&deposit.account_name),
        refund_proof_url = null_or(&deposit.refund_proof_url),
        event_id = esc(&deposit.event_id),
        attendee_id = esc(&deposit.attendee_id),
    );

    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 update_thb_deposit exec: {e:?}"))?;

    Ok(())
}

/// Delete all THB deposits for an event (cleanup).
pub async fn delete_thb_deposits_for_event(db: &D1Database, event_id: &str) -> Result<(), String> {
    let stmt = db.prepare("DELETE FROM thb_deposits WHERE event_id = ?1");
    stmt.bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 delete_thb_deposits bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 delete_thb_deposits run: {e:?}"))?;

    Ok(())
}

/// Delete a single THB deposit by event + attendee (attendee deletion).
pub async fn delete_thb_deposit(
    db: &D1Database,
    event_id: &str,
    attendee_id: &str,
) -> Result<(), String> {
    let stmt = db.prepare("DELETE FROM thb_deposits WHERE event_id = ?1 AND attendee_id = ?2");
    stmt.bind_refs(&[D1Type::Text(event_id), D1Type::Text(attendee_id)])
        .map_err(|e| format!("D1 delete_thb_deposit bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 delete_thb_deposit run: {e:?}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Row → Domain conversion
// ---------------------------------------------------------------------------

fn row_to_thb_deposit(row: serde_json::Value) -> Result<ThbDeposit, String> {
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

    Ok(ThbDeposit {
        attendee_id: get_str("attendee_id"),
        event_id: get_str("event_id"),
        amount_thb: row.get("amount_thb").and_then(|v| v.as_i64()).unwrap_or(0) as u64,
        slip_url: get_opt_str("slip_url"),
        verified: get_bool("verified"),
        verified_by: get_opt_str("verified_by"),
        verified_at: get_opt_str("verified_at"),
        uploaded_at: get_str("uploaded_at"),
        refunded: get_bool("refunded"),
        refunded_at: get_opt_str("refunded_at"),
        attendee_name: get_opt_str("attendee_name"),
        bank_account: get_opt_str("bank_account"),
        bank_name: get_opt_str("bank_name"),
        account_name: get_opt_str("account_name"),
        refund_proof_url: get_opt_str("refund_proof_url"),
    })
}

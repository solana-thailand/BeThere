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
    let result = stmt
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(attendee_id)])
        .map_err(|e| format!("D1 get_thb_deposit bind: {e:?}"))?
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| format!("D1 get_thb_deposit query: {e:?}"))?;

    match result {
        Some(row) => Ok(Some(row_to_thb_deposit(row)?)),
        None => Ok(None),
    }
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
pub async fn insert_thb_deposit(db: &D1Database, deposit: &ThbDeposit) -> Result<(), String> {
    let stmt = db.prepare(
        "INSERT INTO thb_deposits (attendee_id, event_id, amount_thb, slip_url, verified, verified_by, verified_at, uploaded_at, refunded, refunded_at, attendee_name, bank_account, bank_name, account_name, refund_proof_url) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
    );
    stmt.bind_refs(&[
        D1Type::Text(&deposit.attendee_id),
        D1Type::Text(&deposit.event_id),
        D1Type::Integer(deposit.amount_thb as i32),
        opt_text(&deposit.slip_url),
        D1Type::Integer(deposit.verified as i32),
        opt_text(&deposit.verified_by),
        opt_text(&deposit.verified_at),
        D1Type::Text(&deposit.uploaded_at),
        D1Type::Integer(deposit.refunded as i32),
        opt_text(&deposit.refunded_at),
        opt_text(&deposit.attendee_name),
        opt_text(&deposit.bank_account),
        opt_text(&deposit.bank_name),
        opt_text(&deposit.account_name),
        opt_text(&deposit.refund_proof_url),
    ])
    .map_err(|e| format!("D1 insert_thb_deposit bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 insert_thb_deposit run: {e:?}"))?;

    Ok(())
}

/// Update an existing THB deposit (for verify / refund operations).
pub async fn update_thb_deposit(db: &D1Database, deposit: &ThbDeposit) -> Result<(), String> {
    let stmt = db.prepare(
        "UPDATE thb_deposits SET amount_thb = ?1, slip_url = ?2, verified = ?3, verified_by = ?4, verified_at = ?5, refunded = ?6, refunded_at = ?7, attendee_name = ?8, bank_account = ?9, bank_name = ?10, account_name = ?11, refund_proof_url = ?12 \
         WHERE event_id = ?13 AND attendee_id = ?14",
    );
    stmt.bind_refs(&[
        D1Type::Integer(deposit.amount_thb as i32),
        opt_text(&deposit.slip_url),
        D1Type::Integer(deposit.verified as i32),
        opt_text(&deposit.verified_by),
        opt_text(&deposit.verified_at),
        D1Type::Integer(deposit.refunded as i32),
        opt_text(&deposit.refunded_at),
        opt_text(&deposit.attendee_name),
        opt_text(&deposit.bank_account),
        opt_text(&deposit.bank_name),
        opt_text(&deposit.account_name),
        opt_text(&deposit.refund_proof_url),
        D1Type::Text(&deposit.event_id),
        D1Type::Text(&deposit.attendee_id),
    ])
    .map_err(|e| format!("D1 update_thb_deposit bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 update_thb_deposit run: {e:?}"))?;

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

/// Helper: convert `Option<String>` to `D1Type::Text` (empty string for None).
fn opt_text(opt: &Option<String>) -> D1Type<'_> {
    match opt {
        Some(s) => D1Type::Text(s),
        None => D1Type::Null,
    }
}

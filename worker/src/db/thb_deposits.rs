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
///
/// Uses parameterized `bind_refs` — required because `slip_url` may contain a
/// large base64 data URL (several MB) that `db.exec()` cannot inline into a
/// single SQL string. D1 is Cloudflare SQLite, not PostgreSQL/PgCat, so the
/// `raw_sql` convention does not apply here.
pub async fn insert_thb_deposit(db: &D1Database, deposit: &ThbDeposit) -> Result<(), String> {
    let stmt = db.prepare(
        "INSERT INTO thb_deposits (attendee_id, event_id, amount_thb, slip_url, verified, verified_by, verified_at, uploaded_at, refunded, refunded_at, attendee_name, bank_account, bank_name, account_name, refund_proof_url) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
    );
    stmt.bind_refs(&[
        D1Type::Text(&deposit.attendee_id),
        D1Type::Text(&deposit.event_id),
        D1Type::Integer(deposit.amount_thb as i32),
        D1Type::Text(deposit.slip_url.as_deref().unwrap_or("")),
        D1Type::Integer(deposit.verified as i32),
        D1Type::Text(deposit.verified_by.as_deref().unwrap_or("")),
        D1Type::Text(deposit.verified_at.as_deref().unwrap_or("")),
        D1Type::Text(&deposit.uploaded_at),
        D1Type::Integer(deposit.refunded as i32),
        D1Type::Text(deposit.refunded_at.as_deref().unwrap_or("")),
        D1Type::Text(deposit.attendee_name.as_deref().unwrap_or("")),
        D1Type::Text(deposit.bank_account.as_deref().unwrap_or("")),
        D1Type::Text(deposit.bank_name.as_deref().unwrap_or("")),
        D1Type::Text(deposit.account_name.as_deref().unwrap_or("")),
        D1Type::Text(deposit.refund_proof_url.as_deref().unwrap_or("")),
    ])
    .map_err(|e| format!("D1 insert_thb_deposit bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 insert_thb_deposit run: {e:?}"))?;

    Ok(())
}

/// Update an existing THB deposit (for verify / refund operations).
///
/// Uses parameterized `bind_refs` — see `insert_thb_deposit` for rationale.
pub async fn update_thb_deposit(db: &D1Database, deposit: &ThbDeposit) -> Result<(), String> {
    let stmt = db.prepare(
        "UPDATE thb_deposits SET amount_thb = ?1, slip_url = ?2, verified = ?3, verified_by = ?4, verified_at = ?5, refunded = ?6, refunded_at = ?7, attendee_name = ?8, bank_account = ?9, bank_name = ?10, account_name = ?11, refund_proof_url = ?12 \
         WHERE event_id = ?13 AND attendee_id = ?14",
    );
    stmt.bind_refs(&[
        D1Type::Integer(deposit.amount_thb as i32),
        D1Type::Text(deposit.slip_url.as_deref().unwrap_or("")),
        D1Type::Integer(deposit.verified as i32),
        D1Type::Text(deposit.verified_by.as_deref().unwrap_or("")),
        D1Type::Text(deposit.verified_at.as_deref().unwrap_or("")),
        D1Type::Integer(deposit.refunded as i32),
        D1Type::Text(deposit.refunded_at.as_deref().unwrap_or("")),
        D1Type::Text(deposit.attendee_name.as_deref().unwrap_or("")),
        D1Type::Text(deposit.bank_account.as_deref().unwrap_or("")),
        D1Type::Text(deposit.bank_name.as_deref().unwrap_or("")),
        D1Type::Text(deposit.account_name.as_deref().unwrap_or("")),
        D1Type::Text(deposit.refund_proof_url.as_deref().unwrap_or("")),
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

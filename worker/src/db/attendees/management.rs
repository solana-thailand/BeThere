//! Attendee lifecycle / PDPA management helpers (deletes, PII erasure,
//! full sheet→D1 upserts, marketing consent, claim-token repair).

use event_checkin_domain::models::attendee::Attendee;
use serde::Deserialize;
use worker::D1Database;
use worker::d1::D1Type;

use super::reads::D1AttendeeRow;

/// Delete an attendee from D1 by event_id + email.
/// Used for walk-in attendee deletion.
pub(crate) async fn delete_attendee(
    db: &D1Database,
    event_id: &str,
    email: &str,
) -> Result<(), String> {
    let stmt = db.prepare("DELETE FROM attendees WHERE event_id = ?1 AND email = ?2");
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(email)])
        .map_err(|e| format!("D1 delete_attendee bind: {e:?}"))?;
    bound
        .run()
        .await
        .map_err(|e| format!("D1 delete_attendee run: {e:?}"))?;
    Ok(())
}

/// Delete an attendee from D1 by primary key `id`.
/// Avoids the serde_wasm_bindgen deserialization issue in `get_attendee_by_id`.
pub(crate) async fn delete_attendee_by_id(db: &D1Database, id: &str) -> Result<(), String> {
    let stmt = db.prepare("DELETE FROM attendees WHERE id = ?1");
    let bound = stmt
        .bind_refs(&[D1Type::Text(id)])
        .map_err(|e| format!("D1 delete_attendee_by_id bind: {e:?}"))?;
    bound
        .run()
        .await
        .map_err(|e| format!("D1 delete_attendee_by_id run: {e:?}"))?;
    Ok(())
}

/// Minimal struct for PDPA deletion — attendee with event_id (not in domain Attendee).
pub(crate) struct AttendeeWithEmail {
    pub attendee: Attendee,
    pub event_id: String,
}

/// Find all attendees across all events matching an email.
/// Used for PDPA data deletion requests (find all data for a user).
pub(crate) async fn get_attendees_by_email(
    db: &D1Database,
    email: &str,
) -> Result<Vec<AttendeeWithEmail>, String> {
    let sql = format!(
        "SELECT id, event_id, email, name, approval_status, participation_type, \
         checked_in_at, checked_in_by, claim_token, claimed_at, claim_asset_id, \
         claim_signature, qr_url, contact_channel, contact_handle, \
         deposit_status, deposit_amount_usdc, deposit_tx_hash, \
         refund_tx_hash, refund_link, bank_name, bank_account_number, \
         bank_account_name, sheet_row_index \
         FROM attendees WHERE LOWER(email) = '{email}'"
    );
    let stmt = db.prepare(&sql);

    // Bypass D1Result::results() — it uses serde_wasm_bindgen::from_value().unwrap()
    // which panics on NULL columns. Use raw JS interop + JSON.stringify instead.
    let raw_result = wasm_bindgen_futures::JsFuture::from(
        stmt.inner()
            .all()
            .map_err(|e| format!("D1 get_attendees_by_email all() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_attendees_by_email all() await: {e:?}"))?;

    let results_key = wasm_bindgen::JsValue::from_str("results");
    let raw_rows =
        js_sys::Reflect::get(&raw_result, &results_key).unwrap_or(wasm_bindgen::JsValue::NULL);

    let json_str = js_sys::JSON::stringify(&raw_rows)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    let rows: Vec<D1AttendeeRow> = serde_json::from_str(&json_str)
        .map_err(|e| format!("D1 get_attendees_by_email deserialize: {e:?}"))?;
    Ok(rows
        .iter()
        .map(|r| AttendeeWithEmail {
            event_id: r.event_id.clone().unwrap_or_default(),
            attendee: r.to_attendee(),
        })
        .collect())
}

/// Clear PII columns for an attendee row (PDPA right to erasure).
/// Keeps the row but blanks: name, email, contact_channel, contact_handle,
/// checked_in_by, deposit_verified_by, refund_marked_by, refund_link,
/// bank_name, bank_account_number, bank_account_name, claim_token, qr_url.
pub(crate) async fn clear_attendee_pii(db: &D1Database, attendee_id: &str) -> Result<(), String> {
    let sql = format!(
        "UPDATE attendees SET \
         name = '[DELETED]', email = '[DELETED]:' || id, \
         contact_channel = NULL, contact_handle = NULL, \
         checked_in_by = NULL, \
         claim_token = NULL, qr_url = NULL, \
         bank_name = NULL, bank_account_number = NULL, bank_account_name = NULL, \
         refund_link = NULL, \
         updated_at = datetime('now') \
         WHERE id = '{attendee_id}'"
    );
    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 clear_attendee_pii: {e:?}"))?;
    Ok(())
}

/// Full upsert from Google Sheet row — includes lifecycle data (check-in, claims,
/// deposits, QR, refunds). Used by the Sheet → D1 sync endpoint.
///
/// Idempotent: ON CONFLICT updates all lifecycle columns from the sheet data.
///
/// Uses parameterized `prepare().bind_refs().run()` — NOT `db.exec()`.
/// D1's `exec()` truncates multi-line `INSERT ... ON CONFLICT` statements
/// ("incomplete input: SQLITE_ERROR" at `INSERT INTO attendees (`), which is
/// why every other function in this module uses the parameterized path.
/// Empty optional strings bind as NULL so the `COALESCE(excluded.X, attendees.X)`
/// branches preserve existing values instead of overwriting with ''.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn upsert_attendee_full(
    db: &D1Database,
    id: &str,
    event_id: &str,
    email: &str,
    name: &str,
    approval_status: &str,
    participation_type: &str,
    contact_channel: &str,
    contact_handle: &str,
    checked_in_at: Option<&str>,
    checked_in_by: Option<&str>,
    claim_token: Option<&str>,
    claimed_at: Option<&str>,
    qr_url: Option<&str>,
    deposit_status: &str,
    deposit_tx_hash: Option<&str>,
    refund_tx_hash: Option<&str>,
    refund_link: Option<&str>,
    bank_name: Option<&str>,
    bank_account_number: Option<&str>,
    bank_account_name: Option<&str>,
    sheet_row_index: Option<i32>,
) -> Result<(), String> {
    // Map an Option<&str> to a D1 bind, falling back to an empty string.
    // We intentionally do NOT use `D1Type::Null` — the rest of this module
    // binds empty strings for optionals (D1's `bind_refs` rejects `Null` with
    // "D1_TYPE_ERROR: Type 'object' not supported"). To preserve the
    // "empty ⇒ keep existing value" semantics on conflict, the SQL wraps each
    // preserve-column in `COALESCE(NULLIF(excluded.X, ''), attendees.X)`.
    // Declared as a named fn (not a closure) so a single lifetime `'a` ties
    // the borrowed input to the returned `D1Type<'a>`.
    fn opt_text<'a>(opt: Option<&'a str>) -> D1Type<'a> {
        D1Type::Text(opt.unwrap_or(""))
    }

    let stmt = db.prepare(
        "INSERT INTO attendees ( \
         id, event_id, email, name, approval_status, participation_type, \
         contact_channel, contact_handle, \
         checked_in_at, checked_in_by, claim_token, claimed_at, qr_url, \
         deposit_status, deposit_tx_hash, \
         refund_tx_hash, refund_link, \
         bank_name, bank_account_number, bank_account_name, \
         sheet_row_index, synced_at, created_at, updated_at \
         ) VALUES ( \
         ?1, ?2, ?3, ?4, ?5, ?6, \
         ?7, ?8, \
         NULLIF(?9, ''), NULLIF(?10, ''), NULLIF(?11, ''), NULLIF(?12, ''), NULLIF(?13, ''), \
         ?14, NULLIF(?15, ''), \
         NULLIF(?16, ''), NULLIF(?17, ''), \
         NULLIF(?18, ''), NULLIF(?19, ''), NULLIF(?20, ''), \
         ?21, datetime('now'), datetime('now'), datetime('now') \
         ) \
         ON CONFLICT (id) DO UPDATE SET \
         name = excluded.name, \
         approval_status = excluded.approval_status, \
         participation_type = excluded.participation_type, \
         contact_channel = excluded.contact_channel, \
         contact_handle = excluded.contact_handle, \
         checked_in_at = COALESCE(excluded.checked_in_at, attendees.checked_in_at), \
         checked_in_by = COALESCE(excluded.checked_in_by, attendees.checked_in_by), \
         claim_token = COALESCE(excluded.claim_token, attendees.claim_token), \
         claimed_at = COALESCE(excluded.claimed_at, attendees.claimed_at), \
         qr_url = COALESCE(excluded.qr_url, attendees.qr_url), \
         deposit_status = excluded.deposit_status, \
         deposit_tx_hash = COALESCE(excluded.deposit_tx_hash, attendees.deposit_tx_hash), \
         refund_tx_hash = COALESCE(excluded.refund_tx_hash, attendees.refund_tx_hash), \
         refund_link = COALESCE(excluded.refund_link, attendees.refund_link), \
         bank_name = COALESCE(excluded.bank_name, attendees.bank_name), \
         bank_account_number = COALESCE(excluded.bank_account_number, attendees.bank_account_number), \
         bank_account_name = COALESCE(excluded.bank_account_name, attendees.bank_account_name), \
         sheet_row_index = CASE WHEN excluded.sheet_row_index = 0 THEN attendees.sheet_row_index ELSE excluded.sheet_row_index END, \
         synced_at = datetime('now'), \
         updated_at = datetime('now')",
    );

    // sheet_row_index is 1-based (sheet row 2+), so 0 is a safe "unset" sentinel.
    // Avoids `D1Type::Null` (rejected by `bind_refs`); the ON CONFLICT clause
    // treats 0 as "preserve existing".
    let sheet_row_bind = D1Type::Integer(sheet_row_index.unwrap_or(0));

    stmt.bind_refs(&[
        D1Type::Text(id),                 // ?1
        D1Type::Text(event_id),           // ?2
        D1Type::Text(email),              // ?3
        D1Type::Text(name),               // ?4
        D1Type::Text(approval_status),    // ?5
        D1Type::Text(participation_type), // ?6
        D1Type::Text(contact_channel),    // ?7
        D1Type::Text(contact_handle),     // ?8
        opt_text(checked_in_at),          // ?9
        opt_text(checked_in_by),          // ?10
        opt_text(claim_token),            // ?11
        opt_text(claimed_at),             // ?12
        opt_text(qr_url),                 // ?13
        D1Type::Text(deposit_status),     // ?14
        opt_text(deposit_tx_hash),        // ?15
        opt_text(refund_tx_hash),         // ?16
        opt_text(refund_link),            // ?17
        opt_text(bank_name),              // ?18
        opt_text(bank_account_number),    // ?19
        opt_text(bank_account_name),      // ?20
        sheet_row_bind,                   // ?21
    ])
    .map_err(|e| format!("D1 upsert_attendee_full bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 upsert_attendee_full run: {e:?}"))?;

    Ok(())
}

/// Set marketing consent for all attendee rows matching an email.
/// Used for PDPA marketing opt-out (unsubscribe).
pub(crate) async fn set_marketing_consent(
    db: &D1Database,
    email: &str,
    consent: bool,
) -> Result<usize, String> {
    let sql = format!(
        "UPDATE attendees SET \
         consent_marketing = {consent}, \
         consent_marketing_at = datetime('now'), \
         updated_at = datetime('now') \
         WHERE email = '{email}'"
    );
    let result = db
        .exec(&sql)
        .await
        .map_err(|e| format!("D1 set_marketing_consent: {e:?}"))?;
    let count = result.count().unwrap_or(None).unwrap_or(0) as usize;
    Ok(count)
}

/// Repair attendees with empty or NULL claim_tokens by generating new UUID v7s.
/// Returns the number of rows repaired.
pub(crate) async fn repair_empty_claim_tokens(db: &D1Database) -> Result<usize, String> {
    // Count rows that need repair
    let count_stmt = db.prepare(
        "SELECT COUNT(*) as cnt FROM attendees WHERE claim_token = '' OR claim_token IS NULL",
    );
    let count_result = count_stmt
        .first::<serde_json::Value>(None)
        .await
        .map_err(|e| format!("D1 repair count: {e:?}"))?;
    let total = count_result
        .and_then(|v| v.get("cnt").and_then(|c| c.as_i64()))
        .unwrap_or(0);

    if total == 0 {
        return Ok(0);
    }

    // Fetch all attendee IDs that need repair
    #[derive(Deserialize)]
    struct IdRow {
        id: String,
    }
    let stmt = db.prepare("SELECT id FROM attendees WHERE claim_token = '' OR claim_token IS NULL");
    let results = stmt
        .all()
        .await
        .map_err(|e| format!("D1 repair fetch: {e:?}"))?;

    let rows = results
        .results::<IdRow>()
        .map_err(|e| format!("D1 repair parse: {e:?}"))?;

    let mut repaired = 0usize;
    for row in rows {
        let new_token = uuid::Uuid::now_v7().to_string();
        let update_stmt = db.prepare(
            "UPDATE attendees SET claim_token = ?1, updated_at = datetime('now') WHERE id = ?2",
        );
        update_stmt
            .bind_refs(&[D1Type::Text(&new_token), D1Type::Text(&row.id)])
            .map_err(|e| format!("D1 repair bind: {e:?}"))?
            .run()
            .await
            .map_err(|e| format!("D1 repair run for {}: {e:?}", row.id))?;
        repaired += 1;
    }

    Ok(repaired)
}

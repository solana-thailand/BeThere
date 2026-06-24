//! D1 attendee query helpers.
//!
//! Phase 2a: Every write to Google Sheets also writes to D1.
//! Phase 2b: Read paths try D1 first, fall back to Sheets on miss.

use std::str::FromStr;

use event_checkin_domain::models::attendee::{Attendee, CheckInStatus};
use js_sys::Array;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures;
use worker::D1Database;
use worker::d1::D1Type;

/// Insert or update an attendee row in D1.
///
/// Used during registration dual-write. If the attendee already exists
/// (same `id`), updates name and timestamp.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn upsert_attendee(
    db: &D1Database,
    id: &str,
    event_id: &str,
    email: &str,
    name: &str,
    approval_status: &str,
    participation_type: &str,
    contact_channel: &str,
    contact_handle: &str,
    consent_marketing: Option<bool>,
    claim_token: Option<&str>,
) -> Result<(), String> {
    let cm = consent_marketing.unwrap_or(false);
    let stmt = db.prepare(
        "INSERT INTO attendees (id, event_id, email, name, approval_status, participation_type, \
         contact_channel, contact_handle, consent_marketing, consent_marketing_at, claim_token, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'), ?10, datetime('now'), datetime('now')) \
         ON CONFLICT (id) DO UPDATE SET \
         name = excluded.name, \
         approval_status = excluded.approval_status, \
         participation_type = excluded.participation_type, \
         contact_channel = excluded.contact_channel, \
         contact_handle = excluded.contact_handle, \
         consent_marketing = excluded.consent_marketing, \
         consent_marketing_at = excluded.consent_marketing_at, \
         claim_token = COALESCE(attendees.claim_token, excluded.claim_token), \
         updated_at = datetime('now')",
    );
    stmt.bind_refs(&[
        D1Type::Text(id),
        D1Type::Text(event_id),
        D1Type::Text(email),
        D1Type::Text(name),
        D1Type::Text(approval_status),
        D1Type::Text(participation_type),
        D1Type::Text(contact_channel),
        D1Type::Text(contact_handle),
        D1Type::Integer(if cm { 1 } else { 0 }),
        D1Type::Text(claim_token.unwrap_or("")),
    ])
    .map_err(|e| format!("D1 upsert_attendee bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 upsert_attendee run: {e:?}"))?;

    Ok(())
}

/// Write check-in data to D1 (dual-write alongside Sheets).
pub(crate) async fn check_in_attendee(
    db: &D1Database,
    id: &str,
    checked_in_at: &str,
    checked_in_by: &str,
    claim_token: &str,
) -> Result<(), String> {
    let stmt = db.prepare(
        "UPDATE attendees \
         SET checked_in_at = ?1, checked_in_by = ?2, \
         claim_token = CASE WHEN ?3 = '' THEN claim_token ELSE ?3 END, \
         updated_at = datetime('now') \
         WHERE id = ?4",
    );
    stmt.bind_refs(&[
        D1Type::Text(checked_in_at),
        D1Type::Text(checked_in_by),
        D1Type::Text(claim_token),
        D1Type::Text(id),
    ])
    .map_err(|e| format!("D1 check_in_attendee bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 check_in_attendee run: {e:?}"))?;

    Ok(())
}

/// Write claim/NFT result to D1 (dual-write alongside Sheets).
pub(crate) async fn claim_attendee(
    db: &D1Database,
    claim_token: &str,
    claimed_at: &str,
    claim_asset_id: &str,
    claim_signature: &str,
) -> Result<(), String> {
    let stmt = db.prepare(
        "UPDATE attendees \
         SET claimed_at = ?1, claim_asset_id = ?2, claim_signature = ?3, \
         updated_at = datetime('now') \
         WHERE claim_token = ?4",
    );
    stmt.bind_refs(&[
        D1Type::Text(claimed_at),
        D1Type::Text(claim_asset_id),
        D1Type::Text(claim_signature),
        D1Type::Text(claim_token),
    ])
    .map_err(|e| format!("D1 claim_attendee bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 claim_attendee run: {e:?}"))?;

    Ok(())
}

/// Write deposit verification to D1.
pub(crate) async fn verify_deposit(
    db: &D1Database,
    id: &str,
    deposit_status: &str,
    deposit_tx_hash: &str,
    deposit_amount_usdc: i64,
    verified_at: &str,
    verified_by: &str,
) -> Result<(), String> {
    let stmt = db.prepare(
        "UPDATE attendees \
         SET deposit_status = ?1, deposit_tx_hash = ?2, deposit_amount_usdc = ?3, \
         deposit_verified_at = ?4, deposit_verified_by = ?5, \
         updated_at = datetime('now') \
         WHERE id = ?6",
    );
    stmt.bind_refs(&[
        D1Type::Text(deposit_status),
        D1Type::Text(deposit_tx_hash),
        D1Type::Integer(deposit_amount_usdc as i32),
        D1Type::Text(verified_at),
        D1Type::Text(verified_by),
        D1Type::Text(id),
    ])
    .map_err(|e| format!("D1 verify_deposit bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 verify_deposit run: {e:?}"))?;

    Ok(())
}

/// Write a QR URL to D1 for a single attendee.
///
/// Required because the public ticket endpoint reads `qr_url` from D1
/// (D1-first path). Without this, a QR written only to the Google Sheet is
/// invisible to the ticket page until a manual sheet→D1 sync runs.
pub(crate) async fn set_qr_url(db: &D1Database, id: &str, qr_url: &str) -> Result<(), String> {
    let stmt = db.prepare(
        "UPDATE attendees \n         SET qr_url = ?1, updated_at = datetime('now') \n         WHERE id = ?2",
    );
    stmt.bind_refs(&[D1Type::Text(qr_url), D1Type::Text(id)])
        .map_err(|e| format!("D1 set_qr_url bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 set_qr_url run: {e:?}"))?;

    Ok(())
}

/// Write a new `participation_type` to D1 for a single attendee.
///
/// Used by the manual admin override (PATCH /attendee/:id/participation-type)
/// to fix attendees stuck in the wrong track — e.g. someone who chose
/// deposit/in-person but confirmed out-of-band that they'll attend online.
///
/// NOTE: the deposit-deadline auto-switch (`check_and_switch_deadline`)
/// only writes the Sheet, not D1. This helper keeps D1 in sync for the
/// manual path so the public ticket page and admin list agree.
pub(crate) async fn set_participation_type(
    db: &D1Database,
    id: &str,
    participation_type: &str,
) -> Result<(), String> {
    let stmt = db.prepare(
        "UPDATE attendees \n         SET participation_type = ?1, updated_at = datetime('now') \n         WHERE id = ?2",
    );
    stmt.bind_refs(&[D1Type::Text(participation_type), D1Type::Text(id)])
        .map_err(|e| format!("D1 set_participation_type bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 set_participation_type run: {e:?}"))?;

    Ok(())
}

/// Write QR URLs to D1 for multiple attendees in one statement.
///
/// `entries` is `(attendee_id, qr_url)` pairs. Used by batch QR generation.
pub(crate) async fn set_qr_urls_batch(
    db: &D1Database,
    entries: &[(String, String)],
) -> Result<usize, String> {
    if entries.is_empty() {
        return Ok(0);
    }
    let mut updated = 0usize;
    for (id, qr_url) in entries {
        if let Err(e) = set_qr_url(db, id, qr_url).await {
            tracing::warn!(attendee_id = %id, error = %e, "D1 set_qr_urls_batch: row failed");
            continue;
        }
        updated += 1;
    }
    Ok(updated)
}

/// Get deposit status from D1 by attendee ID.
/// Returns the raw deposit columns if a row is found.
///
/// Uses JSON.stringify + serde_json to bypass `.first::<T>()` which uses
/// `serde_wasm_bindgen::from_value()` — that crashes on `JsValue(null)`
/// columns (e.g. rows inserted before ALTER TABLE added NOT NULL DEFAULT).
#[allow(dead_code)]
pub(crate) async fn get_deposit_status_from_d1(
    db: &D1Database,
    attendee_id: &str,
) -> Result<Option<DepositStatusRow>, String> {
    let stmt = db.prepare(
        "SELECT id, event_id, deposit_status, deposit_tx_hash, deposit_amount_usdc \
         FROM attendees WHERE id = ?1",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(attendee_id)])
        .map_err(|e| format!("D1 get_deposit_status bind: {e:?}"))?;

    // Bypass worker crate's .first::<T>() which uses serde_wasm_bindgen::from_value()
    // — that crashes on JsValue(null) columns. Instead, call the raw JS .first() via
    // inner(), then JSON.stringify → serde_json (same pattern as get_attendees_by_event).
    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 get_deposit_status first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_deposit_status first() await: {e:?}"))?;

    // No row found
    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    if json_str.is_empty() {
        return Ok(None);
    }

    let row: DepositStatusRow = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(
            error = %e,
            json = %json_str.chars().take(300).collect::<String>(),
            "D1 get_deposit_status: deserialize failed"
        );
        format!("D1 get_deposit_status deserialize: {e}")
    })?;

    Ok(Some(row))
}

/// Raw D1 row for deposit status columns.
///
/// Uses serde_json (via JSON.stringify) so NULL columns become `None`
/// in Option fields — no serde_wasm_bindgen crash.
#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct DepositStatusRow {
    pub id: Option<String>,
    pub event_id: Option<String>,
    pub deposit_status: Option<String>,
    pub deposit_tx_hash: Option<String>,
    pub deposit_amount_usdc: Option<i64>,
}

/// Save a pending deposit to D1 (upsert deposit columns on the attendee row).
#[allow(dead_code)]
pub(crate) async fn save_deposit_status_to_d1(
    db: &D1Database,
    attendee_id: &str,
    deposit_status: &str,
    deposit_tx_hash: Option<&str>,
    deposit_amount_usdc: i64,
) -> Result<(), String> {
    let tx_hash = deposit_tx_hash.unwrap_or("");
    let stmt = db.prepare(
        "UPDATE attendees \
         SET deposit_status = ?1, deposit_tx_hash = ?2, deposit_amount_usdc = ?3, \
         updated_at = datetime('now') \
         WHERE id = ?4",
    );
    stmt.bind_refs(&[
        D1Type::Text(deposit_status),
        D1Type::Text(tx_hash),
        D1Type::Integer(deposit_amount_usdc as i32),
        D1Type::Text(attendee_id),
    ])
    .map_err(|e| format!("D1 save_deposit_status bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 save_deposit_status run: {e:?}"))?;

    Ok(())
}

/// Count deposits for an event from D1 (fallback for KV deposit counter).
#[allow(dead_code)]
pub(crate) async fn count_deposits_by_event(
    db: &D1Database,
    event_id: &str,
) -> Result<u32, String> {
    let stmt = db.prepare(
        "SELECT COUNT(*) as cnt FROM attendees \
         WHERE event_id = ?1 AND deposit_status IS NOT NULL AND deposit_status != ''",
    );
    #[derive(Deserialize)]
    struct CountRow {
        cnt: i64,
    }
    let row = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 count_deposits bind: {e:?}"))?
        .first::<CountRow>(Some("cnt"))
        .await
        .map_err(|e| format!("D1 count_deposits first: {e:?}"))?;

    Ok(row.map(|r| r.cnt as u32).unwrap_or(0))
}

/// Write refund status to D1.
pub(crate) async fn mark_refund(
    db: &D1Database,
    id: &str,
    deposit_status: &str,
    refund_tx_hash: &str,
    refund_marked_at: &str,
    refund_marked_by: &str,
) -> Result<(), String> {
    let stmt = db.prepare(
        "UPDATE attendees \
         SET deposit_status = ?1, refund_tx_hash = ?2, \
         refund_marked_at = ?3, refund_marked_by = ?4, \
         updated_at = datetime('now') \
         WHERE id = ?5",
    );
    stmt.bind_refs(&[
        D1Type::Text(deposit_status),
        D1Type::Text(refund_tx_hash),
        D1Type::Text(refund_marked_at),
        D1Type::Text(refund_marked_by),
        D1Type::Text(id),
    ])
    .map_err(|e| format!("D1 mark_refund bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 mark_refund run: {e:?}"))?;

    Ok(())
}

/// Undo a check-in in D1 (clear checked_in fields).
pub(crate) async fn undo_check_in(db: &D1Database, id: &str) -> Result<(), String> {
    let stmt = db.prepare(
        "UPDATE attendees \
         SET checked_in_at = NULL, checked_in_by = NULL, claim_token = NULL, \
         updated_at = datetime('now') \
         WHERE id = ?1",
    );
    stmt.bind_refs(&[D1Type::Text(id)])
        .map_err(|e| format!("D1 undo_check_in bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 undo_check_in run: {e:?}"))?;

    Ok(())
}

// ==========================================================================
// Phase 2b: D1-first reads
// ==========================================================================

/// D1 row representation — maps to the `attendees` table columns.
///
/// Not all `Attendee` struct fields exist in D1; missing fields are
/// filled with defaults/None during conversion.
#[derive(Debug, Clone, Deserialize)]
struct D1AttendeeRow {
    id: Option<String>,
    #[allow(dead_code)]
    event_id: Option<String>,
    email: Option<String>,
    name: Option<String>,
    approval_status: Option<String>,
    participation_type: Option<String>,
    checked_in_at: Option<String>,
    checked_in_by: Option<String>,
    claim_token: Option<String>,
    claimed_at: Option<String>,
    claim_asset_id: Option<String>,
    #[allow(dead_code)]
    claim_signature: Option<String>,
    qr_url: Option<String>,
    contact_channel: Option<String>,
    contact_handle: Option<String>,
    deposit_status: Option<String>,
    deposit_amount_usdc: Option<i64>,
    deposit_tx_hash: Option<String>,
    #[allow(dead_code)]
    refund_tx_hash: Option<String>,
    refund_link: Option<String>,
    bank_name: Option<String>,
    bank_account_number: Option<String>,
    bank_account_name: Option<String>,
    sheet_row_index: Option<i64>,
}

impl D1AttendeeRow {
    /// Convert D1 row to domain `Attendee`.
    ///
    /// Fields not present in D1 (first_name, last_name, ticket_name, phone,
    /// deposit_agreed, deposit_method, deposit_amount, solana_address,
    /// send_email_status) are set to defaults.
    fn to_attendee(&self) -> Attendee {
        let approval = CheckInStatus::from_str(self.approval_status.as_deref().unwrap_or(""))
            .unwrap_or(CheckInStatus::PendingApproval);

        // Derive deposit_verified from deposit_status
        let deposit_verified = match self.deposit_status.as_deref() {
            Some("verified") | Some("confirmed") => Some("true".to_string()),
            _ => None,
        };

        // Derive refund_status from deposit_status
        let refund_status = match self.deposit_status.as_deref() {
            Some("refunded" | "manual_refund") => Some("refunded".to_string()),
            _ => None,
        };

        // Build nft_proof_url from claim_asset_id
        let nft_proof_url = self
            .claim_asset_id
            .as_ref()
            .map(|id| format!("https://orb.helius.com/nft/{id}"));

        Attendee {
            api_id: self.id.clone().unwrap_or_default(),
            first_name: String::new(),
            last_name: String::new(),
            name: self.name.clone().unwrap_or_default(),
            email: self.email.clone().unwrap_or_default(),
            ticket_name: self.name.clone().unwrap_or_default(),
            approval_status: approval,
            participation_type: self.participation_type.clone().unwrap_or_default(),
            registration_date: None,
            phone: None,
            contact_channel: self.contact_channel.clone(),
            contact_handle: self.contact_handle.clone(),
            deposit_agreed: None,
            deposit_method: None,
            deposit_amount: self.deposit_amount_usdc.map(|v| v.to_string()),
            deposit_tx_signature: self.deposit_tx_hash.clone(),
            deposit_verified,
            checked_in_at: self.checked_in_at.clone(),
            checked_in_by: self.checked_in_by.clone(),
            solana_address: None,
            qr_code_url: self.qr_url.clone(),
            claim_token: self.claim_token.clone(),
            claimed_at: self.claimed_at.clone(),
            nft_proof_url,
            bank_account: self.bank_account_number.clone(),
            bank_name: self.bank_name.clone(),
            account_name: self.bank_account_name.clone(),
            refund_status,
            refund_link: self.refund_link.clone(),
            send_email_status: None,
            row_index: self.sheet_row_index.unwrap_or(0) as usize,
        }
    }
}

/// Fetch a single attendee by `api_id` from D1.
/// Returns `Ok(None)` if not found.
pub(crate) async fn get_attendee_by_id(
    db: &D1Database,
    api_id: &str,
) -> Result<Option<Attendee>, String> {
    let stmt = db.prepare(
        "SELECT id, event_id, email, name, approval_status, participation_type, \
         checked_in_at, checked_in_by, claim_token, claimed_at, claim_asset_id, \
         claim_signature, qr_url, contact_channel, contact_handle, \
         deposit_status, deposit_amount_usdc, deposit_tx_hash, \
         refund_tx_hash, refund_link, bank_name, bank_account_number, \
         bank_account_name, sheet_row_index \
         FROM attendees WHERE id = ?1",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(api_id)])
        .map_err(|e| format!("D1 get_attendee_by_id bind: {e:?}"))?;

    // Bypass worker crate's .first::<T>() which uses serde_wasm_bindgen::from_value()
    // — that crashes on JsValue(null) columns. Instead: raw JS .first() → JSON.stringify
    // → serde_json (same pattern as get_deposit_status_from_d1).
    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 get_attendee_by_id first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_attendee_by_id first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    if json_str.is_empty() {
        return Ok(None);
    }

    let row: D1AttendeeRow = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(
            error = %e,
            json = %json_str.chars().take(300).collect::<String>(),
            "D1 get_attendee_by_id: deserialize failed"
        );
        format!("D1 get_attendee_by_id deserialize: {e}")
    })?;

    Ok(Some(row.to_attendee()))
}

/// Fetch a single attendee by claim token from D1.
pub(crate) async fn get_attendee_by_claim_token(
    db: &D1Database,
    claim_token: &str,
) -> Result<Option<Attendee>, String> {
    let stmt = db.prepare(
        "SELECT id, event_id, email, name, approval_status, participation_type, \
         checked_in_at, checked_in_by, claim_token, claimed_at, claim_asset_id, \
         claim_signature, qr_url, contact_channel, contact_handle, \
         deposit_status, deposit_amount_usdc, deposit_tx_hash, \
         refund_tx_hash, refund_link, bank_name, bank_account_number, \
         bank_account_name, sheet_row_index \
         FROM attendees WHERE claim_token = ?1",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(claim_token)])
        .map_err(|e| format!("D1 get_attendee_by_claim_token bind: {e:?}"))?;

    // Bypass worker crate's .first::<T>() which uses serde_wasm_bindgen::from_value()
    // — that crashes on JsValue(null) columns. Instead: raw JS .first() → JSON.stringify
    // → serde_json (same pattern as get_attendee_by_id).
    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 get_attendee_by_claim_token first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_attendee_by_claim_token first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    if json_str.is_empty() {
        return Ok(None);
    }

    let row: D1AttendeeRow = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(
            error = %e,
            json = %json_str.chars().take(300).collect::<String>(),
            "D1 get_attendee_by_claim_token: deserialize failed"
        );
        format!("D1 get_attendee_by_claim_token deserialize: {e}")
    })?;

    Ok(Some(row.to_attendee()))
}

/// Fetch only the `event_id` for an attendee by claim token.
///
/// Lightweight scalar query used to resolve the correct event context for a
/// claim token when the caller has no event_id (e.g. the public claim URL
/// `/claim/{token}` carries no event). Without this, the claim flow falls
/// back to the "first active event", which may NOT be the attendee's event —
/// causing the claim page to show the wrong event's name/NFT/deposit config.
pub(crate) async fn get_attendee_event_id_by_claim_token(
    db: &D1Database,
    claim_token: &str,
) -> Result<Option<String>, String> {
    #[derive(Deserialize)]
    struct EventIdRow {
        event_id: Option<String>,
    }
    let stmt = db.prepare("SELECT event_id FROM attendees WHERE claim_token = ?1");
    let row = stmt
        .bind_refs(&[D1Type::Text(claim_token)])
        .map_err(|e| format!("D1 get_attendee_event_id_by_claim_token bind: {e:?}"))?
        .first::<EventIdRow>(Some("event_id"))
        .await
        .map_err(|e| format!("D1 get_attendee_event_id_by_claim_token first: {e:?}"))?;
    Ok(row.and_then(|r| r.event_id.filter(|s| !s.is_empty())))
}

/// Fetch all attendees for a given event from D1.
///
/// Uses JSON.stringify-based deserialization to avoid the panic in
/// `D1Result::results()` which uses `serde_wasm_bindgen::from_value(...).unwrap()`.
pub(crate) async fn get_attendees_by_event(
    db: &D1Database,
    event_id: &str,
) -> Result<Vec<Attendee>, String> {
    let stmt = db.prepare(
        "SELECT id, event_id, email, name, approval_status, participation_type, \
         checked_in_at, checked_in_by, claim_token, claimed_at, claim_asset_id, \
         claim_signature, qr_url, contact_channel, contact_handle, \
         deposit_status, deposit_amount_usdc, deposit_tx_hash, \
         refund_tx_hash, refund_link, bank_name, bank_account_number, \
         bank_account_name, sheet_row_index \
         FROM attendees WHERE event_id = ?1 \
         ORDER BY sheet_row_index ASC, created_at ASC",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 get_attendees_by_event bind: {e:?}"))?;

    // Bypass D1Result::results() entirely — it uses serde_wasm_bindgen::from_value().unwrap()
    // which panics on type mismatches (e.g. D1 NULL → serde_json::Value). With panic="abort",
    // this kills the Worker isolate and causes the runtime hang.
    //
    // Instead, call the prepared statement's .all() promise directly via raw JS interop,
    // extract the .results array from the returned JS object, then serialize each row via
    // JSON.stringify and parse with serde_json (pure Rust, never panics on bad data).
    let raw_result = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .all()
            .map_err(|e| format!("D1 raw all() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 raw all() await: {e:?}"))?;

    // Extract the .results array from the D1 result JS object
    let results_val = js_sys::Reflect::get(&raw_result, &js_sys::JsString::from("results"))
        .map_err(|e| format!("D1 get results property: {e:?}"))?;

    // null results → empty
    if results_val.is_null() || results_val.is_undefined() {
        return Ok(Vec::new());
    }

    let results_arr: Array = results_val
        .dyn_into()
        .map_err(|e| format!("D1 results is not an array: {e:?}"))?;

    let mut rows = Vec::with_capacity(results_arr.length() as usize);
    for (i, js_row) in results_arr.iter().enumerate() {
        // JSON.stringify the JS object → string, then parse with serde_json.
        // This completely avoids serde_wasm_bindgen and its .unwrap() panics.
        let json_str = match js_sys::JSON::stringify(&js_row) {
            Ok(s) => s.as_string().unwrap_or_default(),
            Err(e) => {
                let msg = format!("{e:?}");
                tracing::warn!(row_index = i, error = %msg, "D1: JSON.stringify failed, skipping row");
                continue;
            }
        };
        match serde_json::from_str::<D1AttendeeRow>(&json_str) {
            Ok(row) => rows.push(row),
            Err(e) => {
                tracing::warn!(
                    row_index = i,
                    error = %e,
                    json = %json_str.chars().take(200).collect::<String>(),
                    "D1: skipping row with deserialize error"
                );
            }
        }
    }

    Ok(rows.into_iter().map(|r| r.to_attendee()).collect())
}

/// D1 row with aggregate counts for targeted claim lookup.
#[derive(Deserialize)]
struct D1AttendeeWithCounts {
    id: Option<String>,
    #[allow(dead_code)]
    event_id: Option<String>,
    email: Option<String>,
    name: Option<String>,
    approval_status: Option<String>,
    participation_type: Option<String>,
    checked_in_at: Option<String>,
    checked_in_by: Option<String>,
    claim_token: Option<String>,
    claimed_at: Option<String>,
    claim_asset_id: Option<String>,
    #[allow(dead_code)]
    claim_signature: Option<String>,
    qr_url: Option<String>,
    contact_channel: Option<String>,
    contact_handle: Option<String>,
    deposit_status: Option<String>,
    deposit_amount_usdc: Option<i64>,
    deposit_tx_hash: Option<String>,
    #[allow(dead_code)]
    refund_tx_hash: Option<String>,
    refund_link: Option<String>,
    bank_name: Option<String>,
    bank_account_number: Option<String>,
    bank_account_name: Option<String>,
    sheet_row_index: Option<i64>,
    total_checked_in: Option<i64>,
    total_claimed: Option<i64>,
}

impl D1AttendeeWithCounts {
    fn to_attendee(&self) -> Attendee {
        let approval = CheckInStatus::from_str(self.approval_status.as_deref().unwrap_or(""))
            .unwrap_or(CheckInStatus::PendingApproval);
        let deposit_verified = match self.deposit_status.as_deref() {
            Some("verified" | "confirmed") => Some("true".to_string()),
            _ => None,
        };
        let refund_status = match self.deposit_status.as_deref() {
            Some("refunded" | "manual_refund") => Some("refunded".to_string()),
            _ => None,
        };
        let nft_proof_url = self
            .claim_asset_id
            .as_ref()
            .map(|id| format!("https://orb.helius.com/nft/{id}"));
        Attendee {
            api_id: self.id.clone().unwrap_or_default(),
            first_name: String::new(),
            last_name: String::new(),
            name: self.name.clone().unwrap_or_default(),
            email: self.email.clone().unwrap_or_default(),
            ticket_name: self.name.clone().unwrap_or_default(),
            approval_status: approval,
            participation_type: self.participation_type.clone().unwrap_or_default(),
            registration_date: None,
            phone: None,
            contact_channel: self.contact_channel.clone(),
            contact_handle: self.contact_handle.clone(),
            deposit_agreed: None,
            deposit_method: None,
            deposit_amount: self.deposit_amount_usdc.map(|v| v.to_string()),
            deposit_tx_signature: self.deposit_tx_hash.clone(),
            deposit_verified,
            checked_in_at: self.checked_in_at.clone(),
            checked_in_by: self.checked_in_by.clone(),
            solana_address: None,
            qr_code_url: self.qr_url.clone(),
            claim_token: self.claim_token.clone(),
            claimed_at: self.claimed_at.clone(),
            nft_proof_url,
            bank_account: self.bank_account_number.clone(),
            bank_name: self.bank_name.clone(),
            account_name: self.bank_account_name.clone(),
            refund_status,
            refund_link: self.refund_link.clone(),
            send_email_status: None,
            row_index: self.sheet_row_index.unwrap_or(0) as usize,
        }
    }
}

async fn count_checked_in(db: &D1Database, event_id: &str) -> Result<usize, String> {
    count_by_status(db, event_id, "checked_in_at").await
}

async fn count_claimed(db: &D1Database, event_id: &str) -> Result<usize, String> {
    count_by_status(db, event_id, "claimed_at").await
}

async fn count_by_status(db: &D1Database, event_id: &str, column: &str) -> Result<usize, String> {
    #[derive(Deserialize)]
    struct CountRow {
        cnt: i64,
    }
    let stmt = db.prepare(format!(
        "SELECT COUNT(*) as cnt FROM attendees WHERE event_id = ?1 AND {column} IS NOT NULL"
    ));
    let row = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 count_by_status bind: {e:?}"))?
        .first::<CountRow>(Some("cnt"))
        .await
        .map_err(|e| format!("D1 count_by_status first: {e:?}"))?;
    Ok(row.map(|r| r.cnt as usize).unwrap_or(0))
}

/// Look up attendee by claim token and return event-level claim counts.
pub(crate) async fn get_attendee_with_claim_counts(
    db: &D1Database,
    claim_token: &str,
    event_id: &str,
) -> Result<(Option<Attendee>, usize, usize), String> {
    // Targeted query: find attendee by claim_token + count aggregates in one D1 call.
    // Previous version fetched ALL attendees for the event (O(n) data transfer).
    let stmt = db.prepare(
        "SELECT a.id, a.event_id, a.email, a.name, a.approval_status, a.participation_type, \
                a.checked_in_at, a.checked_in_by, a.claim_token, a.claimed_at, a.claim_asset_id, \
                a.claim_signature, a.qr_url, a.contact_channel, a.contact_handle, \
                a.deposit_status, a.deposit_amount_usdc, a.deposit_tx_hash, \
                a.refund_tx_hash, a.refund_link, a.bank_name, a.bank_account_number, \
                a.bank_account_name, a.sheet_row_index, \
                (SELECT COUNT(*) FROM attendees WHERE event_id = a.event_id AND checked_in_at IS NOT NULL) AS total_checked_in, \
                (SELECT COUNT(*) FROM attendees WHERE event_id = a.event_id AND claimed_at IS NOT NULL) AS total_claimed \
         FROM attendees a \
         WHERE a.event_id = ?1 AND a.claim_token = ?2",
    );

    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(claim_token)])
        .map_err(|e| format!("D1 get_attendee_with_counts bind: {e:?}"))?;

    // Use JSON.stringify approach (same as get_attendees_by_event) to avoid
    // D1Result::results() panic on NULL→serde type mismatch.
    let raw_result = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .all()
            .map_err(|e| format!("D1 raw all() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_attendee_with_counts await: {e:?}"))?;

    let results_val = js_sys::Reflect::get(&raw_result, &js_sys::JsString::from("results"))
        .map_err(|e| format!("D1 get results property: {e:?}"))?;

    if results_val.is_null() || results_val.is_undefined() {
        // No attendee with this claim_token — get counts for the event
        let checked_in = count_checked_in(db, event_id).await?;
        let claimed = count_claimed(db, event_id).await?;
        return Ok((None, checked_in, claimed));
    }

    let results_arr: Array = results_val
        .dyn_into()
        .map_err(|e| format!("D1 results is not an array: {e:?}"))?;

    if results_arr.length() == 0 {
        let checked_in = count_checked_in(db, event_id).await?;
        let claimed = count_claimed(db, event_id).await?;
        return Ok((None, checked_in, claimed));
    }

    // Parse the first (and only) row with counts
    let js_row = results_arr.get(0);
    let json_str = js_sys::JSON::stringify(&js_row)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    let row: D1AttendeeWithCounts = serde_json::from_str(&json_str).map_err(|e| {
        format!(
            "D1 attendee_with_counts parse: {e}, json: {}",
            json_str.chars().take(200).collect::<String>()
        )
    })?;

    let attendee = row.to_attendee();
    Ok((
        Some(attendee),
        row.total_checked_in.unwrap_or(0) as usize,
        row.total_claimed.unwrap_or(0) as usize,
    ))
}

/// Count in-person attendees for an event from D1.
#[allow(dead_code)]
pub(crate) async fn count_in_person_attendees(
    db: &D1Database,
    event_id: &str,
) -> Result<usize, String> {
    let stmt = db.prepare(
        "SELECT COUNT(*) as cnt FROM attendees \
         WHERE event_id = ?1 AND \
         (participation_type LIKE '%in-person%' \
          OR participation_type LIKE '%in person%' \
          OR participation_type = '' \
          OR participation_type IS NULL)",
    );
    #[derive(Deserialize)]
    struct CountRow {
        cnt: i64,
    }
    let row = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 count_in_person bind: {e:?}"))?
        .first::<CountRow>(Some("cnt"))
        .await
        .map_err(|e| format!("D1 count_in_person first: {e:?}"))?;

    Ok(row.map(|r| r.cnt as usize).unwrap_or(0))
}

// ==========================================================================
// Walk-in attendee D1 helpers (P3.3: KV-optional fallback)
// ==========================================================================

use event_checkin_domain::models::attendee::WalkinAttendee;

/// Check if a walk-in attendee already exists for the given event + email.
pub(crate) async fn check_walkin_duplicate(
    db: &D1Database,
    event_id: &str,
    email: &str,
) -> Result<bool, String> {
    let stmt = db.prepare(
        "SELECT COUNT(*) as cnt FROM attendees \
         WHERE event_id = ?1 AND email = ?2 AND participation_type = 'walkin'",
    );
    #[derive(Deserialize)]
    struct CountRow {
        cnt: i64,
    }
    let row = stmt
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(email)])
        .map_err(|e| format!("D1 check_walkin_duplicate bind: {e:?}"))?
        .first::<CountRow>(Some("cnt"))
        .await
        .map_err(|e| format!("D1 check_walkin_duplicate first: {e:?}"))?;

    Ok(row.map(|r| r.cnt > 0).unwrap_or(false))
}

/// Attempt to insert a walk-in attendee, rejecting duplicates atomically.
///
/// Uses `INSERT ... SELECT ... WHERE NOT EXISTS` to combine the duplicate
/// check and insert into a single D1 round-trip. Returns `Ok(true)` if the
/// row was inserted, `Ok(false)` if a duplicate already existed.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn try_insert_walkin(
    db: &D1Database,
    id: &str,
    event_id: &str,
    email: &str,
    name: &str,
    phone: Option<&str>,
    checked_in_at: &str,
    checked_in_by: &str,
    claim_token: &str,
) -> Result<bool, String> {
    let stmt = db.prepare(
        "INSERT INTO attendees (id, event_id, email, name, approval_status, participation_type, \
         contact_channel, contact_handle, checked_in_at, checked_in_by, claim_token, \
         created_at, updated_at) \
         SELECT ?1, ?2, ?3, ?4, 'approved', 'walkin', ?5, ?6, ?7, ?8, ?9, \
         datetime('now'), datetime('now') \
         WHERE NOT EXISTS (\
         SELECT 1 FROM attendees \
         WHERE event_id = ?10 AND email = ?11 AND participation_type = 'walkin')",
    );
    let contact_channel = phone.unwrap_or("");
    let result = stmt
        .bind_refs(&[
            D1Type::Text(id),
            D1Type::Text(event_id),
            D1Type::Text(email),
            D1Type::Text(name),
            D1Type::Text(contact_channel),
            D1Type::Text(""), // contact_handle
            D1Type::Text(checked_in_at),
            D1Type::Text(checked_in_by),
            D1Type::Text(claim_token),
            D1Type::Text(event_id),
            D1Type::Text(email),
        ])
        .map_err(|e| format!("D1 try_insert_walkin bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 try_insert_walkin run: {e:?}"))?;

    let changes = result
        .meta()
        .map_err(|e| format!("D1 try_insert_walkin meta: {e:?}"))?
        .and_then(|m| m.changes)
        .unwrap_or(0);

    Ok(changes > 0)
}

/// Count walk-in attendees for an event from D1.
///
/// Returns the count without fetching full rows.
pub(crate) async fn count_walkin_attendees(db: &D1Database, event_id: &str) -> Result<u32, String> {
    let stmt = db.prepare(
        "SELECT COUNT(*) as cnt FROM attendees \
         WHERE event_id = ?1 AND participation_type = 'walkin'",
    );
    #[derive(Deserialize)]
    struct CountRow {
        cnt: i64,
    }
    let row = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 count_walkin_attendees bind: {e:?}"))?
        .first::<CountRow>(Some("cnt"))
        .await
        .map_err(|e| format!("D1 count_walkin_attendees first: {e:?}"))?;

    Ok(row.map(|r| r.cnt as u32).unwrap_or(0))
}

/// Fetch walk-in attendees for an event from D1.
///
/// Filters by `participation_type = 'walkin'` and converts D1 rows
/// to `WalkinAttendee` domain objects.
pub(crate) async fn get_walkin_attendees(
    db: &D1Database,
    event_id: &str,
) -> Result<Vec<WalkinAttendee>, String> {
    let stmt = db.prepare(
        "SELECT id, event_id, email, name, participation_type, \
         checked_in_at, checked_in_by, claim_token, claimed_at, \
         contact_channel, deposit_status \
         FROM attendees \
         WHERE event_id = ?1 AND participation_type = 'walkin' \
         ORDER BY checked_in_at ASC",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 get_walkin_attendees bind: {e:?}"))?;

    // Use same safe JSON stringify approach as get_attendees_by_event
    let raw_result = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .all()
            .map_err(|e| format!("D1 get_walkin raw all() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_walkin raw all() await: {e:?}"))?;

    let results_val = js_sys::Reflect::get(&raw_result, &js_sys::JsString::from("results"))
        .map_err(|e| format!("D1 get_walkin results property: {e:?}"))?;

    if results_val.is_null() || results_val.is_undefined() {
        return Ok(Vec::new());
    }

    let results_arr: Array = results_val
        .dyn_into()
        .map_err(|e| format!("D1 get_walkin results is not an array: {e:?}"))?;

    let mut walkins = Vec::with_capacity(results_arr.length() as usize);
    for (i, js_row) in results_arr.iter().enumerate() {
        let json_str = match js_sys::JSON::stringify(&js_row) {
            Ok(s) => s.as_string().unwrap_or_default(),
            Err(e) => {
                let msg = format!("{e:?}");
                tracing::warn!(row_index = i, error = %msg, "D1 walkin: JSON.stringify failed, skipping row");
                continue;
            }
        };
        #[derive(Deserialize)]
        struct WalkinRow {
            event_id: String,
            email: String,
            name: String,
            checked_in_at: Option<String>,
            checked_in_by: Option<String>,
            claim_token: Option<String>,
            claimed_at: Option<String>,
            contact_channel: Option<String>,
        }
        match serde_json::from_str::<WalkinRow>(&json_str) {
            Ok(row) => {
                // phone stored in contact_channel for walkins
                let phone = row.contact_channel.filter(|c| !c.is_empty());
                walkins.push(WalkinAttendee {
                    event_id: row.event_id,
                    name: row.name,
                    email: row.email,
                    phone,
                    claim_token: row.claim_token.unwrap_or_default(),
                    checked_in_at: row.checked_in_at.unwrap_or_default(),
                    checked_in_by: row.checked_in_by.unwrap_or_default(),
                    wallet_address: None,
                    claimed_at: row.claimed_at,
                });
            }
            Err(e) => {
                tracing::warn!(
                    row_index = i,
                    error = %e,
                    json = %json_str.chars().take(200).collect::<String>(),
                    "D1 walkin: skipping row with deserialize error"
                );
            }
        }
    }

    Ok(walkins)
}

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

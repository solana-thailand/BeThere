//! D1 attendee query helpers.
//!
//! Phase 2a: Every write to Google Sheets also writes to D1.
//! Phase 2b: Read paths try D1 first, fall back to Sheets on miss.

use std::str::FromStr;

use event_checkin_domain::models::attendee::{Attendee, CheckInStatus};
use serde::Deserialize;
use worker::D1Database;
use worker::d1::D1Type;

/// Insert or update an attendee row in D1.
///
/// Used during registration dual-write. If the attendee already exists
/// (same `id`), updates name and timestamp.
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
) -> Result<(), String> {
    let stmt = db.prepare(
        "INSERT INTO attendees (id, event_id, email, name, approval_status, participation_type, \
         contact_channel, contact_handle, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'), datetime('now')) \
         ON CONFLICT (id) DO UPDATE SET \
         name = excluded.name, \
         approval_status = excluded.approval_status, \
         participation_type = excluded.participation_type, \
         contact_channel = excluded.contact_channel, \
         contact_handle = excluded.contact_handle, \
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
         SET checked_in_at = ?1, checked_in_by = ?2, claim_token = ?3, \
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
    id: String,
    event_id: String,
    email: String,
    name: String,
    approval_status: String,
    participation_type: String,
    checked_in_at: Option<String>,
    checked_in_by: Option<String>,
    claim_token: Option<String>,
    claimed_at: Option<String>,
    claim_asset_id: Option<String>,
    claim_signature: Option<String>,
    qr_url: Option<String>,
    contact_channel: Option<String>,
    contact_handle: Option<String>,
    deposit_status: Option<String>,
    deposit_amount_usdc: Option<i64>,
    deposit_tx_hash: Option<String>,
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
        let approval = CheckInStatus::from_str(&self.approval_status)
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
            api_id: self.id.clone(),
            first_name: String::new(),
            last_name: String::new(),
            name: self.name.clone(),
            email: self.email.clone(),
            ticket_name: self.name.clone(),
            approval_status: approval,
            participation_type: self.participation_type.clone(),
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
    let row = stmt
        .bind_refs(&[D1Type::Text(api_id)])
        .map_err(|e| format!("D1 get_attendee_by_id bind: {e:?}"))?
        .first::<D1AttendeeRow>(None)
        .await
        .map_err(|e| format!("D1 get_attendee_by_id first: {e:?}"))?;

    Ok(row.map(|r| r.to_attendee()))
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
    let row = stmt
        .bind_refs(&[D1Type::Text(claim_token)])
        .map_err(|e| format!("D1 get_attendee_by_claim_token bind: {e:?}"))?
        .first::<D1AttendeeRow>(None)
        .await
        .map_err(|e| format!("D1 get_attendee_by_claim_token first: {e:?}"))?;

    Ok(row.map(|r| r.to_attendee()))
}

/// Fetch all attendees for a given event from D1.
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
    let result = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 get_attendees_by_event bind: {e:?}"))?
        .all()
        .await
        .map_err(|e| format!("D1 get_attendees_by_event all: {e:?}"))?;

    let rows: Vec<D1AttendeeRow> = result
        .results()
        .map_err(|e| format!("D1 get_attendees_by_event results: {e:?}"))?;

    Ok(rows.into_iter().map(|r| r.to_attendee()).collect())
}

/// Look up attendee by claim token and return event-level claim counts.
pub(crate) async fn get_attendee_with_claim_counts(
    db: &D1Database,
    claim_token: &str,
    event_id: &str,
) -> Result<(Option<Attendee>, usize, usize), String> {
    // Single query: get all attendees for the event, then find the one with
    // matching claim token and count checked_in / claimed.
    let attendees = get_attendees_by_event(db, event_id).await?;
    let total_checked_in = attendees
        .iter()
        .filter(|a| a.checked_in_at.is_some())
        .count();
    let total_claimed = attendees.iter().filter(|a| a.claimed_at.is_some()).count();
    let attendee = attendees
        .into_iter()
        .find(|a| a.claim_token.as_deref() == Some(claim_token));

    Ok((attendee, total_checked_in, total_claimed))
}

/// Count in-person attendees for an event from D1.
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

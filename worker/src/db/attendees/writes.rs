//! Attendee mutation helpers (INSERT / UPDATE dual-writes to D1).

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

/// Insert a post-event registration attendee row (Plan 008 — Phase 3).
///
/// Mirrors `upsert_attendee` but sets `registration_phase = 'post_event'` and
/// `approval_status = 'post_event_registered'`. No claim token, no check-in —
/// post-event registrants are leads, not attendees. They're naturally excluded
/// from capacity / check-in queries that filter on `approval_status = 'approved'`
/// or `registration_phase = 'pre_event'`.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn upsert_post_event_attendee(
    db: &D1Database,
    id: &str,
    event_id: &str,
    email: &str,
    name: &str,
    participation_type: &str,
    contact_channel: &str,
    contact_handle: &str,
    consent_marketing: Option<bool>,
) -> Result<(), String> {
    let cm = consent_marketing.unwrap_or(false);
    let stmt = db.prepare(
        "INSERT INTO attendees (id, event_id, email, name, approval_status, participation_type, \
         contact_channel, contact_handle, consent_marketing, consent_marketing_at, registration_phase, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'post_event_registered', ?5, ?6, ?7, ?8, datetime('now'), 'post_event', datetime('now'), datetime('now')) \
         ON CONFLICT (id) DO UPDATE SET \
         name = excluded.name, \
         participation_type = excluded.participation_type, \
         contact_channel = excluded.contact_channel, \
         contact_handle = excluded.contact_handle, \
         consent_marketing = excluded.consent_marketing, \
         consent_marketing_at = excluded.consent_marketing_at, \
         updated_at = datetime('now')",
    );
    stmt.bind_refs(&[
        D1Type::Text(id),
        D1Type::Text(event_id),
        D1Type::Text(email),
        D1Type::Text(name),
        D1Type::Text(participation_type),
        D1Type::Text(contact_channel),
        D1Type::Text(contact_handle),
        D1Type::Integer(if cm { 1 } else { 0 }),
    ])
    .map_err(|e| format!("D1 upsert_post_event_attendee bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 upsert_post_event_attendee run: {e:?}"))?;

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

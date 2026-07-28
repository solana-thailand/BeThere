//! Phase 2b: D1-first reads — row mapping structs and read queries.

use std::str::FromStr;

use event_checkin_domain::models::attendee::{Attendee, CheckInStatus};
use js_sys::Array;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use worker::D1Database;
use worker::d1::D1Type;

/// D1 row representation — maps to the `attendees` table columns.
///
/// Not all `Attendee` struct fields exist in D1; missing fields are
/// filled with defaults/None during conversion.
#[derive(Debug, Clone, Deserialize)]
pub(super) struct D1AttendeeRow {
    id: Option<String>,
    #[allow(dead_code)]
    pub(super) event_id: Option<String>,
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
    pub(super) fn to_attendee(&self) -> Attendee {
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
    // `.first(Some(col_name))` returns the *scalar* value of `col_name` from the
    // first row (a string), NOT a row object. Deserializing into a struct here is
    // a type mismatch that silently fails — the caller (`resolve_event_id_from_token`)
    // then returns None and the claim flow falls back to the "first active event",
    // showing the wrong event's name/NFT/deposit on the claim page.
    //
    // `first::<String>` returns `Option<String>`: `None` when no row matches (or
    // the column is NULL), `Some(value)` otherwise — no struct wrapper needed.
    let stmt = db.prepare("SELECT event_id FROM attendees WHERE claim_token = ?1");
    let event_id = stmt
        .bind_refs(&[D1Type::Text(claim_token)])
        .map_err(|e| format!("D1 get_attendee_event_id_by_claim_token bind: {e:?}"))?
        .first::<String>(Some("event_id"))
        .await
        .map_err(|e| format!("D1 get_attendee_event_id_by_claim_token first: {e:?}"))?;
    Ok(event_id.filter(|s| !s.is_empty()))
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
    let stmt = db.prepare(format!(
        "SELECT COUNT(*) as cnt FROM attendees WHERE event_id = ?1 AND {column} IS NOT NULL"
    ));
    let cnt = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 count_by_status bind: {e:?}"))?
        .first::<i64>(Some("cnt"))
        .await
        .map_err(|e| format!("D1 count_by_status first: {e:?}"))?;
    Ok(cnt.map(|c| c as usize).unwrap_or(0))
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
///
/// Predicate mirrors `dashboard::IN_PERSON_PREDICATE` (canonical match post-
/// backfill; see Issue #059 Step 3.4). Inlined here rather than referencing
/// the dashboard const to avoid a cross-module dependency for this dead-code
/// helper; the two must stay in sync.
#[allow(dead_code)]
pub(crate) async fn count_in_person_attendees(
    db: &D1Database,
    event_id: &str,
) -> Result<usize, String> {
    let stmt = db.prepare(
        "SELECT COUNT(*) as cnt FROM attendees \
         WHERE event_id = ?1 AND \
         (participation_type = 'in_person' \
          OR TRIM(participation_type) = '')",
    );
    let cnt = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 count_in_person bind: {e:?}"))?
        .first::<i64>(Some("cnt"))
        .await
        .map_err(|e| format!("D1 count_in_person first: {e:?}"))?;

    Ok(cnt.map(|c| c as usize).unwrap_or(0))
}

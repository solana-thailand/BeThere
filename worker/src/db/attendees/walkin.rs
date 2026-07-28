//! Walk-in attendee D1 helpers (P3.3: KV-optional fallback).

use event_checkin_domain::models::attendee::WalkinAttendee;
use js_sys::Array;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use worker::D1Database;
use worker::d1::D1Type;

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
    let cnt = stmt
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(email)])
        .map_err(|e| format!("D1 check_walkin_duplicate bind: {e:?}"))?
        .first::<i64>(Some("cnt"))
        .await
        .map_err(|e| format!("D1 check_walkin_duplicate first: {e:?}"))?;

    Ok(cnt.map(|c| c > 0).unwrap_or(false))
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
    let cnt = stmt
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 count_walkin_attendees bind: {e:?}"))?
        .first::<i64>(Some("cnt"))
        .await
        .map_err(|e| format!("D1 count_walkin_attendees first: {e:?}"))?;

    Ok(cnt.map(|c| c as u32).unwrap_or(0))
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

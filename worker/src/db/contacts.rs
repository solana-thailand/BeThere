//! D1 contact query helpers for dual-write (Phase 2a).
//!
//! Every write to Google Sheets contacts also writes to D1.

use worker::D1Database;
use worker::d1::D1Type;

/// Upsert a contact in D1 when a registration occurs.
///
/// On conflict (same email), updates name, last_registered, events_joined,
/// event_count, and contact preferences.
pub(crate) async fn upsert_contact(
    db: &D1Database,
    email: &str,
    name: &str,
    events_joined: &str,
    event_count: i32,
    contact_channel: &str,
    contact_handle: &str,
) -> Result<(), String> {
    let stmt = db.prepare(
        "INSERT INTO contacts (email, name, first_registered, last_registered, \
         events_joined, event_count, contact_channel, contact_handle) \
         VALUES (?1, ?2, datetime('now'), datetime('now'), ?3, ?4, ?5, ?6) \
         ON CONFLICT (email) DO UPDATE SET \
         name = excluded.name, \
         last_registered = datetime('now'), \
         events_joined = ?3, \
         event_count = ?4, \
         contact_channel = excluded.contact_channel, \
         contact_handle = excluded.contact_handle",
    );
    stmt.bind_refs(&[
        D1Type::Text(email),
        D1Type::Text(name),
        D1Type::Text(events_joined),
        D1Type::Integer(event_count),
        D1Type::Text(contact_channel),
        D1Type::Text(contact_handle),
    ])
    .map_err(|e| format!("D1 upsert_contact bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 upsert_contact run: {e:?}"))?;

    Ok(())
}

/// Update deposit credit for a contact (rolling balance across events).
#[allow(dead_code)]
pub(crate) async fn update_deposit_credit(
    db: &D1Database,
    email: &str,
    credit_thb: i64,
    credit_usdc: i64,
) -> Result<(), String> {
    let stmt = db.prepare(
        "UPDATE contacts \
         SET deposit_credit_thb = ?1, deposit_credit_usdc = ?2, \
         deposit_credit_since = datetime('now') \
         WHERE email = ?3",
    );
    stmt.bind_refs(&[
        D1Type::Integer(credit_thb as i32),
        D1Type::Integer(credit_usdc as i32),
        D1Type::Text(email),
    ])
    .map_err(|e| format!("D1 update_deposit_credit bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 update_deposit_credit run: {e:?}"))?;

    Ok(())
}

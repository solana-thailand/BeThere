//! The subscriber tables (migration 0060). Statements are constants, so the
//! SQL guards and the Python tests read the same text the worker runs.

use worker::{D1Database, D1Type};

use crate::db::d1_int::{int_bind, uint_bind};

/// Add or renew a subscriber. A re-subscribe after an unsubscribe clears it
/// and moves the consent time; a repeat while subscribed changes only the
/// language. ?1 email (lowercased), ?2 locale, ?3 now, ?4 new token.
pub const UPSERT_SQL: &str = "INSERT INTO subscribers (email, locale, consent_at, unsub_token, created_at) \
VALUES (?1, ?2, ?3, ?4, ?3) \
ON CONFLICT(email) DO UPDATE SET locale = excluded.locale, \
consent_at = CASE WHEN subscribers.unsubscribed_at IS NULL THEN subscribers.consent_at ELSE excluded.consent_at END, \
unsubscribed_at = NULL";

/// One-click unsubscribe by token; repeating it changes nothing.
/// ?1 token, ?2 now.
pub const UNSUBSCRIBE_SQL: &str = "UPDATE subscribers SET unsubscribed_at = ?2 WHERE unsub_token = ?1 AND unsubscribed_at IS NULL";

/// Note every public event that is open now and not seen before. ?1 now (ISO),
/// ?2 now (epoch ms).
pub const MARK_OPEN_SQL: &str = "INSERT OR IGNORE INTO announced_events (event_id, announced_at) \
SELECT id, ?1 FROM events \
WHERE status = 'active' AND visibility = 'public' AND event_end_ms > ?2";

/// Mails owed: an open public event and a subscriber who asked before it
/// opened, with no announcement row yet. ?1 now (epoch ms), ?2 batch size.
pub const DUE_SQL: &str = "SELECT e.id AS event_id, e.name AS name, e.slug AS slug, \
e.event_start_ms AS start_ms, COALESCE(e.location, '') AS location, \
s.email AS email, s.locale AS locale, s.unsub_token AS unsub_token \
FROM announced_events a \
JOIN events e ON e.id = a.event_id \
JOIN subscribers s ON s.unsubscribed_at IS NULL AND s.consent_at < a.announced_at \
WHERE e.status = 'active' AND e.visibility = 'public' AND e.event_end_ms > ?1 \
AND NOT EXISTS (SELECT 1 FROM event_announcements x WHERE x.event_id = e.id AND x.email = s.email) \
ORDER BY a.announced_at, s.consent_at \
LIMIT ?2";

/// Claim one send before it happens. ?1 event, ?2 email, ?3 now.
pub const CLAIM_SQL: &str =
    "INSERT OR IGNORE INTO event_announcements (event_id, email, claimed_at) VALUES (?1, ?2, ?3)";

/// The send went: ?1 event, ?2 email, ?3 now.
pub const SENT_SQL: &str =
    "UPDATE event_announcements SET sent_at = ?3 WHERE event_id = ?1 AND email = ?2";

/// Gmail refused it outright: free the claim so the next run can try again.
pub const RELEASE_SQL: &str =
    "DELETE FROM event_announcements WHERE event_id = ?1 AND email = ?2 AND sent_at IS NULL";

/// An unsubscribe token: 64 lowercase hex characters (`crypto::random_hex(32)`);
/// nothing else reaches SQL.
pub fn is_token(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// PDPA erasure (`handlers/privacy.rs`): the subscription, then its send log.
pub const ERASE_SQL: [&str; 2] = [
    "DELETE FROM subscribers WHERE email = ?1",
    "DELETE FROM event_announcements WHERE email = ?1",
];

/// `YYYY-MM-DDTHH:MM:SSZ`, the one timestamp form these tables compare.
pub fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

async fn run(db: &D1Database, sql: &str, binds: &[D1Type<'_>]) -> Result<usize, String> {
    let result = db
        .prepare(sql)
        .bind_refs(binds)
        .map_err(|e| format!("D1 subscribers bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 subscribers run: {e:?}"))?;
    Ok(result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0))
}

pub async fn upsert(db: &D1Database, email: &str, locale: &str, token: &str) -> Result<(), String> {
    let now = now_iso();
    run(
        db,
        UPSERT_SQL,
        &[
            D1Type::Text(email),
            D1Type::Text(locale),
            D1Type::Text(&now),
            D1Type::Text(token),
        ],
    )
    .await
    .map(drop)
}

/// Whether a subscriber was unsubscribed now (false: unknown token, or done
/// already; the caller answers the same either way).
pub async fn unsubscribe(db: &D1Database, token: &str) -> Result<bool, String> {
    let now = now_iso();
    run(
        db,
        UNSUBSCRIBE_SQL,
        &[D1Type::Text(token), D1Type::Text(&now)],
    )
    .await
    .map(|n| n > 0)
}

pub async fn mark_open(db: &D1Database, now_ms: i64) -> Result<usize, String> {
    let now = now_iso();
    let now_bind = int_bind("now_ms", now_ms).map_err(|e| e.to_string())?;
    run(db, MARK_OPEN_SQL, &[D1Type::Text(&now), now_bind]).await
}

/// One owed mail.
pub struct Due {
    pub event: super::copy::OpenEvent,
    pub email: String,
    pub thai: bool,
    pub unsub_token: String,
}

pub async fn due(db: &D1Database, now_ms: i64, limit: u32) -> Result<Vec<Due>, String> {
    let now_bind = int_bind("now_ms", now_ms).map_err(|e| e.to_string())?;
    let limit_bind = uint_bind("limit", u64::from(limit)).map_err(|e| e.to_string())?;
    let stmt = db
        .prepare(DUE_SQL)
        .bind_refs(&[now_bind, limit_bind])
        .map_err(|e| format!("D1 subscribers due bind: {e:?}"))?;
    let rows = crate::db::d1_safe::safe_all_rows(&stmt).await?;
    let text = |r: &serde_json::Value, k: &str| {
        r.get(k)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    Ok(rows
        .iter()
        .map(|r| Due {
            event: super::copy::OpenEvent {
                id: text(r, "event_id"),
                name: text(r, "name"),
                slug: text(r, "slug"),
                start_ms: r.get("start_ms").and_then(|v| v.as_f64()).unwrap_or(0.0) as i64,
                location: text(r, "location"),
            },
            email: text(r, "email"),
            thai: text(r, "locale") == "th",
            unsub_token: text(r, "unsub_token"),
        })
        .collect())
}

pub async fn claim(db: &D1Database, event_id: &str, email: &str) -> Result<bool, String> {
    let now = now_iso();
    run(
        db,
        CLAIM_SQL,
        &[
            D1Type::Text(event_id),
            D1Type::Text(email),
            D1Type::Text(&now),
        ],
    )
    .await
    .map(|n| n > 0)
}

pub async fn sent(db: &D1Database, event_id: &str, email: &str) -> Result<(), String> {
    let now = now_iso();
    run(
        db,
        SENT_SQL,
        &[
            D1Type::Text(event_id),
            D1Type::Text(email),
            D1Type::Text(&now),
        ],
    )
    .await
    .map(drop)
}

pub async fn release(db: &D1Database, event_id: &str, email: &str) -> Result<(), String> {
    run(
        db,
        RELEASE_SQL,
        &[D1Type::Text(event_id), D1Type::Text(email)],
    )
    .await
    .map(drop)
}

/// Delete everything these tables hold for `email` (lowercased by the caller).
pub async fn erase(db: &D1Database, email: &str) -> Result<(), String> {
    for sql in ERASE_SQL {
        run(db, sql, &[D1Type::Text(email)]).await?;
    }
    Ok(())
}

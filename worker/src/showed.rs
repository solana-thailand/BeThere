//! "I showed up" links (Super GOAT SG4): `/showed/{token}` names an event a
//! person came to, and nothing about the person. The decision whether they
//! came is `door_history` (`crate::record`); this module stores the links.
//! The token is `crypto::random_hex(16)`, carries no id, and a revoked link
//! is gone: the page 404s and the API answers 404.

use event_checkin_domain::models::record::{SharedLink, ShowedPage};
use serde_json::Value;
use worker::{D1Database, D1Type};

use crate::db::d1_safe::safe_all_rows;

/// The live link of (?1 email, ?2 event id), if any.
pub const LIVE_SQL: &str = "SELECT token FROM showed_links \
     WHERE email = ?1 AND event_id = ?2 AND revoked_at IS NULL ORDER BY created_at LIMIT 1";
/// ?1 token, ?2 email, ?3 event id, ?4 now.
pub const CREATE_SQL: &str = "INSERT INTO showed_links (token, email, event_id, created_at) \
     VALUES (?1, ?2, ?3, ?4)";
/// Revoke ?1 token of ?2 email (another person's token changes nothing); ?3 now.
pub const REVOKE_SQL: &str = "UPDATE showed_links SET revoked_at = ?3 \
     WHERE token = ?1 AND email = ?2 AND revoked_at IS NULL";
/// The person's live links, by event slug.
pub const MINE_SQL: &str = "SELECT e.slug AS slug, l.token AS token \
     FROM showed_links l JOIN events e ON e.id = l.event_id \
     WHERE l.email = ?1 AND l.revoked_at IS NULL ORDER BY e.event_start_ms";
/// The public page of ?1 token: live, and the event still shown.
pub const PAGE_SQL: &str = "SELECT e.id AS id, e.name AS name, e.slug AS slug, \
       e.event_start_ms AS start_ms \
     FROM showed_links l JOIN events e ON e.id = l.event_id \
     WHERE l.token = ?1 AND l.revoked_at IS NULL \
       AND e.status IN ('active', 'completed') AND e.visibility = 'public'";
/// The event id of ?1 slug.
pub const EVENT_ID_SQL: &str = "SELECT id FROM events WHERE slug = ?1 LIMIT 1";
/// PDPA erasure (`handlers/privacy.rs`).
pub const ERASE_SQL: &str = "DELETE FROM showed_links WHERE email = ?1";

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn text(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

async fn rows(db: &D1Database, sql: &str, binds: &[D1Type<'_>]) -> Result<Vec<Value>, String> {
    let stmt = db
        .prepare(sql)
        .bind_refs(binds)
        .map_err(|e| format!("D1 showed bind: {e:?}"))?;
    safe_all_rows(&stmt).await
}

async fn run(db: &D1Database, sql: &str, binds: &[D1Type<'_>]) -> Result<usize, String> {
    let result = db
        .prepare(sql)
        .bind_refs(binds)
        .map_err(|e| format!("D1 showed bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 showed run: {e:?}"))?;
    Ok(result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0))
}

/// The id of the event `slug`, if it exists.
pub async fn event_id(db: &D1Database, slug: &str) -> Result<Option<String>, String> {
    let found = rows(db, EVENT_ID_SQL, &[D1Type::Text(slug)]).await?;
    Ok(found
        .first()
        .map(|r| text(r, "id"))
        .filter(|id| !id.is_empty()))
}

/// The live link of (email, event), made if there is none. The caller has
/// checked that the person came.
pub async fn link(db: &D1Database, email: &str, event_id: &str) -> Result<String, String> {
    let live = rows(db, LIVE_SQL, &[D1Type::Text(email), D1Type::Text(event_id)]).await?;
    if let Some(token) = live.first().map(|r| text(r, "token")) {
        return Ok(token);
    }
    let token = crate::crypto::random_hex(16)?;
    let now = now_iso();
    run(
        db,
        CREATE_SQL,
        &[
            D1Type::Text(&token),
            D1Type::Text(email),
            D1Type::Text(event_id),
            D1Type::Text(&now),
        ],
    )
    .await?;
    Ok(token)
}

/// Revoke the person's own link; false if it was not theirs or not live.
pub async fn revoke(db: &D1Database, email: &str, token: &str) -> Result<bool, String> {
    let now = now_iso();
    let changed = run(
        db,
        REVOKE_SQL,
        &[D1Type::Text(token), D1Type::Text(email), D1Type::Text(&now)],
    )
    .await?;
    Ok(changed > 0)
}

/// The person's live links.
pub async fn mine(db: &D1Database, email: &str) -> Result<Vec<SharedLink>, String> {
    Ok(rows(db, MINE_SQL, &[D1Type::Text(email)])
        .await?
        .iter()
        .map(|r| SharedLink {
            event_slug: text(r, "slug"),
            token: text(r, "token"),
        })
        .collect())
}

/// The public page of `token` and its event id, or `None` (unknown,
/// revoked, or the event no longer shown). `card` is left for the caller.
pub async fn page(db: &D1Database, token: &str) -> Result<Option<(String, ShowedPage)>, String> {
    let found = rows(db, PAGE_SQL, &[D1Type::Text(token)]).await?;
    Ok(found.first().map(|r| {
        (
            text(r, "id"),
            ShowedPage {
                event_name: text(r, "name"),
                event_slug: text(r, "slug"),
                start_ms: r.get("start_ms").and_then(Value::as_i64).unwrap_or(0),
                card: String::new(),
            },
        )
    }))
}

pub async fn erase(db: &D1Database, email: &str) -> Result<(), String> {
    run(db, ERASE_SQL, &[D1Type::Text(email)]).await.map(|_| ())
}

/// The card path for an event: its "I showed up" card, else its event card,
/// else empty. An R2 error counts as absent: a share never fails on art.
pub async fn card(bucket: &worker::Bucket, event_id: &str) -> String {
    use crate::storage::{exists, og_card_key, og_showed_key};
    if exists(bucket, &og_showed_key(event_id))
        .await
        .unwrap_or(false)
    {
        return format!("{}{event_id}", crate::og_meta::OG_SHOWED_PREFIX);
    }
    match exists(bucket, &og_card_key(event_id))
        .await
        .unwrap_or(false)
    {
        true => format!("{}{event_id}", crate::og_meta::OG_CARD_PREFIX),
        false => String::new(),
    }
}

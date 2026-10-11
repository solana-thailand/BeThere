//! Meals the organizer ordered for an event (`event_meals`, migration 0064,
//! .plans/045 R4.11). Read into the public stats as an aggregate only
//! (`sql/public_stats_meals.sql`); written by `PUT /api/events/{id}/meals`.

use worker::D1Database;
use worker::d1::D1Type;

/// The most a count may be (the migration's CHECK).
pub const MAX_MEALS: u32 = 100_000;

/// The event's count, if the organizer entered one.
pub async fn read(db: &D1Database, event_id: &str) -> Result<Option<u32>, String> {
    let stmt = db
        .prepare("SELECT ordered FROM event_meals WHERE event_id = ?1")
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 event_meals read bind: {e:?}"))?;
    let rows = crate::db::d1_safe::safe_all_rows(&stmt).await?;
    Ok(rows
        .first()
        .and_then(|r| r.get("ordered"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|n| u32::try_from(n).ok()))
}

/// Set the event's count; `None` removes it (the event leaves the aggregate).
pub async fn write(
    db: &D1Database,
    event_id: &str,
    ordered: Option<u32>,
    now: &str,
) -> Result<(), String> {
    let stmt = match ordered {
        Some(n) => db
            .prepare(
                "INSERT INTO event_meals (event_id, ordered, updated_at) VALUES (?1, ?2, ?3) \
                 ON CONFLICT(event_id) DO UPDATE SET ordered = excluded.ordered, \
                 updated_at = excluded.updated_at",
            )
            .bind_refs(&[
                D1Type::Text(event_id),
                D1Type::Integer(n.min(MAX_MEALS) as i32),
                D1Type::Text(now),
            ]),
        None => db
            .prepare("DELETE FROM event_meals WHERE event_id = ?1")
            .bind_refs(&[D1Type::Text(event_id)]),
    }
    .map_err(|e| format!("D1 event_meals write bind: {e:?}"))?;
    stmt.run()
        .await
        .map_err(|e| format!("D1 event_meals write: {e:?}"))?;
    Ok(())
}

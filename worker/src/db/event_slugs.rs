//! Slug ownership lookup for event renames.
//!
//! Kept out of `events.rs`, which is already past the file-size budget.

use worker::D1Database;
use worker::d1::D1Type;

use crate::db::d1_safe;

/// Whether an event other than `own_id` holds `slug` as its slug or its id.
///
/// Both columns are indexed (`id` is the primary key, `idx_events_slug`), so
/// this reads only the conflicting rows. The rule itself is
/// `event_checkin_domain::models::event::slug_taken_by_other`.
pub async fn slug_taken_by_other(
    db: &D1Database,
    slug: &str,
    own_id: &str,
) -> Result<bool, String> {
    let stmt = db
        .prepare("SELECT id FROM events WHERE (slug = ?1 OR id = ?1) AND id <> ?2 LIMIT 1")
        .bind_refs(&[D1Type::Text(slug), D1Type::Text(own_id)])
        .map_err(|e| format!("D1 slug_taken_by_other bind: {e:?}"))?;
    Ok(!d1_safe::safe_all_rows(&stmt).await?.is_empty())
}

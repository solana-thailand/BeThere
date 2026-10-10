//! Door clicks (.plans/045 R4.10): page × door × day counts, no cookies, no
//! ids, no IP. The page and the door must be names the site uses, so a
//! caller cannot write anything else into the table.

use worker::{D1Database, D1Type};

/// The pages that show doors (`frontend-leptos` `SitePage`).
pub const PAGES: [&str; 4] = ["home", "events", "organizers", "sponsors"];
/// Where a door can lead: the pages, and the devnet sandbox (the try band).
pub const DOORS: [&str; 5] = ["home", "events", "organizers", "sponsors", "try"];

/// Count one click. ?1 day (YYYY-MM-DD, UTC), ?2 page, ?3 door.
pub const COUNT_SQL: &str = "INSERT INTO door_clicks (day, page, door, n) VALUES (?1, ?2, ?3, 1) \
ON CONFLICT(day, page, door) DO UPDATE SET n = n + 1";

/// Whether `page` → `door` is a click the site can make (not to itself).
pub fn is_valid(page: &str, door: &str) -> bool {
    PAGES.contains(&page) && DOORS.contains(&door) && page != door
}

pub async fn count(db: &D1Database, page: &str, door: &str) -> Result<(), String> {
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    db.prepare(COUNT_SQL)
        .bind_refs(&[D1Type::Text(&day), D1Type::Text(page), D1Type::Text(door)])
        .map_err(|e| format!("D1 door_clicks bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 door_clicks run: {e:?}"))?;
    Ok(())
}

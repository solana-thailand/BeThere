//! Courses (.plans/045 R4.4): a course is an active campaign, its episodes
//! the campaign's public events by date (migration 0062 seeds the first
//! two). This module reads them, and is the only reader and writer of
//! `course_enrolments` and `course_progress` (migration 0061).
//!
//! `.issues/068`: watching a recording is never check-in, attendance, a
//! deposit or badge eligibility. Nothing here touches those tables
//! (`tests/courses_guard.rs` reads this file and fails if it does).

use event_checkin_domain::models::course::{
    CourseDetail, CourseEpisode, CourseSummary, youtube_id,
};
use worker::{D1Database, D1Type};

/// Register once. Repeating it changes nothing. ?1 email, ?2 course, ?3 now.
pub const ENROL_SQL: &str =
    "INSERT OR IGNORE INTO course_enrolments (email, course, enrolled_at) VALUES (?1, ?2, ?3)";

/// Mark one episode watched: only for someone registered for the course,
/// and only an episode of it (a public event in the campaign).
/// ?1 email, ?2 course, ?3 episode (event slug), ?4 now.
pub const WATCHED_SQL: &str = "INSERT OR IGNORE INTO course_progress (email, course, episode, watched_at) \
SELECT ?1, ?2, ?3, ?4 WHERE EXISTS (SELECT 1 FROM course_enrolments WHERE email = ?1 AND course = ?2) \
AND EXISTS (SELECT 1 FROM campaign_events ce JOIN events e ON e.id = ce.event_id \
WHERE ce.campaign_id = ?2 AND e.slug = ?3 AND e.visibility = 'public' AND e.status IN ('active', 'completed'))";

/// Every public episode of every active course, by course then date (no
/// binds; [`fold_courses`] counts what has been held).
pub const COURSES_SQL: &str = "SELECT c.id AS id, c.title AS title, c.description AS description, \
e.event_start_ms AS start_ms, e.event_end_ms AS end_ms \
FROM campaigns c JOIN campaign_events ce ON ce.campaign_id = c.id JOIN events e ON e.id = ce.event_id \
WHERE c.status = 'active' AND e.visibility = 'public' AND e.status IN ('active', 'completed') \
ORDER BY c.id, e.event_start_ms";

/// One active course and its public episodes, oldest first. ?1 course.
pub const COURSE_SQL: &str = "SELECT c.id AS id, c.title AS title, c.description AS description, \
e.slug AS slug, e.name AS name, e.event_start_ms AS start_ms, e.event_end_ms AS end_ms, \
COALESCE(e.video_url, '') AS video_url \
FROM campaigns c JOIN campaign_events ce ON ce.campaign_id = c.id JOIN events e ON e.id = ce.event_id \
WHERE c.id = ?1 AND c.status = 'active' AND e.visibility = 'public' AND e.status IN ('active', 'completed') \
ORDER BY e.event_start_ms";

/// Whether a course is open for registration. ?1 course.
pub const COURSE_OPEN_SQL: &str =
    "SELECT 1 AS open FROM campaigns WHERE id = ?1 AND status = 'active'";

/// Whether they registered. ?1 email, ?2 course.
pub const ENROLLED_SQL: &str =
    "SELECT 1 AS enrolled FROM course_enrolments WHERE email = ?1 AND course = ?2";

/// The episodes they marked watched. ?1 email, ?2 course.
pub const PROGRESS_SQL: &str =
    "SELECT episode FROM course_progress WHERE email = ?1 AND course = ?2 ORDER BY watched_at";

/// PDPA erasure (`handlers/privacy.rs`).
pub const ERASE_SQL: [&str; 2] = [
    "DELETE FROM course_progress WHERE email = ?1",
    "DELETE FROM course_enrolments WHERE email = ?1",
];

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

async fn run(db: &D1Database, sql: &str, binds: &[D1Type<'_>]) -> Result<usize, String> {
    let result = db
        .prepare(sql)
        .bind_refs(binds)
        .map_err(|e| format!("D1 courses bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 courses run: {e:?}"))?;
    Ok(result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0))
}

pub async fn enrol(db: &D1Database, email: &str, course: &str) -> Result<(), String> {
    let now = now_iso();
    run(
        db,
        ENROL_SQL,
        &[
            D1Type::Text(email),
            D1Type::Text(course),
            D1Type::Text(&now),
        ],
    )
    .await
    .map(drop)
}

/// Whether the episode was recorded (false: not registered, or already marked).
pub async fn mark_watched(
    db: &D1Database,
    email: &str,
    course: &str,
    episode: &str,
) -> Result<bool, String> {
    let now = now_iso();
    run(
        db,
        WATCHED_SQL,
        &[
            D1Type::Text(email),
            D1Type::Text(course),
            D1Type::Text(episode),
            D1Type::Text(&now),
        ],
    )
    .await
    .map(|n| n > 0)
}

/// `(enrolled, watched episodes)` for one person and course.
pub async fn progress(
    db: &D1Database,
    email: &str,
    course: &str,
) -> Result<(bool, Vec<String>), String> {
    let binds = [D1Type::Text(email), D1Type::Text(course)];
    let enrolled_stmt = db
        .prepare(ENROLLED_SQL)
        .bind_refs(&binds)
        .map_err(|e| format!("D1 courses enrolled bind: {e:?}"))?;
    let enrolled = !crate::db::d1_safe::safe_all_rows(&enrolled_stmt)
        .await?
        .is_empty();
    let progress_stmt = db
        .prepare(PROGRESS_SQL)
        .bind_refs(&binds)
        .map_err(|e| format!("D1 courses progress bind: {e:?}"))?;
    let watched = crate::db::d1_safe::safe_all_rows(&progress_stmt)
        .await?
        .iter()
        .filter_map(|r| {
            r.get("episode")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .collect();
    Ok((enrolled, watched))
}

pub async fn erase(db: &D1Database, email: &str) -> Result<(), String> {
    for sql in ERASE_SQL {
        run(db, sql, &[D1Type::Text(email)]).await?;
    }
    Ok(())
}

fn text(r: &serde_json::Value, k: &str) -> String {
    r.get(k)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn num(r: &serde_json::Value, k: &str) -> i64 {
    r.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0) as i64
}

/// The courses from [`COURSES_SQL`] rows (one per episode), with the held
/// episodes counted and their start times kept for the cadence line.
pub fn fold_courses(rows: &[serde_json::Value], now_ms: i64) -> Vec<CourseSummary> {
    let mut out: Vec<CourseSummary> = Vec::new();
    for r in rows {
        let id = text(r, "id");
        if out.last().map(|c| c.id != id).unwrap_or(true) {
            out.push(CourseSummary {
                id: id.clone(),
                title: text(r, "title"),
                description: text(r, "description"),
                ..Default::default()
            });
        }
        let course = out.last_mut().expect("pushed above");
        if num(r, "end_ms") <= now_ms {
            course.held += 1;
            course.held_starts_ms.push(num(r, "start_ms"));
        }
    }
    out
}

/// The course page from [`COURSE_SQL`] rows, or `None` for no such course.
pub fn fold_course(rows: &[serde_json::Value]) -> Option<CourseDetail> {
    let first = rows.first()?;
    Some(CourseDetail {
        id: text(first, "id"),
        title: text(first, "title"),
        description: text(first, "description"),
        episodes: rows
            .iter()
            .map(|r| CourseEpisode {
                slug: text(r, "slug"),
                name: text(r, "name"),
                start_ms: num(r, "start_ms"),
                end_ms: num(r, "end_ms"),
                video: youtube_id(&text(r, "video_url")).unwrap_or_default(),
            })
            .collect(),
    })
}

pub async fn courses(db: &D1Database, now_ms: i64) -> Result<Vec<CourseSummary>, String> {
    let stmt = db.prepare(COURSES_SQL);
    let rows = crate::db::d1_safe::safe_all_rows(&stmt).await?;
    Ok(fold_courses(&rows, now_ms))
}

pub async fn course(db: &D1Database, id: &str) -> Result<Option<CourseDetail>, String> {
    let stmt = db
        .prepare(COURSE_SQL)
        .bind_refs(&[D1Type::Text(id)])
        .map_err(|e| format!("D1 course bind: {e:?}"))?;
    let rows = crate::db::d1_safe::safe_all_rows(&stmt).await?;
    Ok(fold_course(&rows))
}

pub async fn is_open(db: &D1Database, id: &str) -> Result<bool, String> {
    let stmt = db
        .prepare(COURSE_OPEN_SQL)
        .bind_refs(&[D1Type::Text(id)])
        .map_err(|e| format!("D1 course open bind: {e:?}"))?;
    Ok(!crate::db::d1_safe::safe_all_rows(&stmt).await?.is_empty())
}

//! Courses (.plans/045 R4.4): who registered for which course and which
//! episodes they marked as watched. The only reader and writer of
//! `course_enrolments` and `course_progress` (migration 0061).
//!
//! `.issues/068`: watching a recording is never check-in, attendance, a
//! deposit or badge eligibility. Nothing here touches those tables
//! (`tests/courses_guard.rs` reads this file and fails if it does).

use worker::{D1Database, D1Type};

/// Register once. Repeating it changes nothing. ?1 email, ?2 course, ?3 now.
pub const ENROL_SQL: &str =
    "INSERT OR IGNORE INTO course_enrolments (email, course, enrolled_at) VALUES (?1, ?2, ?3)";

/// Mark one episode watched, only for someone registered for the course.
/// ?1 email, ?2 course, ?3 episode, ?4 now.
pub const WATCHED_SQL: &str = "INSERT OR IGNORE INTO course_progress (email, course, episode, watched_at) \
SELECT ?1, ?2, ?3, ?4 WHERE EXISTS (SELECT 1 FROM course_enrolments WHERE email = ?1 AND course = ?2)";

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

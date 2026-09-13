//! Aggregate production-readiness queries for organizer-visible events.

use event_checkin_domain::models::api::QuizConfig;
use worker::D1Database;
use worker::d1::D1Type;

use crate::db::d1_safe;

#[derive(Debug, Clone, serde::Serialize)]
pub struct QuizReadinessProblem {
    pub event_id: String,
    pub event_name: String,
    pub status: String,
    pub reason: String,
    pub blocked_attendees: u64,
}

/// Return quiz-gated events whose configuration cannot safely unlock claims.
/// Authorization is applied in SQL before any rows leave D1. Attendees are
/// represented only by an aggregate count; no PII or claim token is selected.
pub async fn quiz_problems(
    db: &D1Database,
    email: Option<&str>,
) -> Result<Vec<QuizReadinessProblem>, String> {
    let is_super_admin = email.is_none();
    let email = email.unwrap_or("").trim().to_lowercase();
    let statement = db
        .prepare(include_str!("sql/quiz_readiness_problems.sql"))
        .bind_refs(&[
            D1Type::Integer(i32::from(is_super_admin)),
            D1Type::Text(&email),
        ])
        .map_err(|error| format!("D1 quiz readiness bind: {error:?}"))?;
    let rows = d1_safe::safe_all_rows(&statement).await?;

    let problems = rows
        .into_iter()
        .filter_map(|row| {
            let event_id = row.get("event_id")?.as_str()?.to_string();
            let event_name = row
                .get("event_name")
                .and_then(|value| value.as_str())
                .unwrap_or(&event_id)
                .to_string();
            let status = row
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
                .to_string();
            let blocked_attendees = row
                .get("blocked_attendees")
                .and_then(|value| value.as_i64())
                .unwrap_or(0)
                .max(0) as u64;
            let reason = match row.get("config_json").and_then(|value| value.as_str()) {
                None => "quiz is enabled but no quiz configuration exists".to_string(),
                Some(raw) => match serde_json::from_str::<QuizConfig>(raw) {
                    Ok(config) => match crate::quiz::validate_ready_config(&config) {
                        Ok(()) => return None,
                        Err(reason) => reason,
                    },
                    Err(_) => "quiz configuration is malformed".to_string(),
                },
            };
            Some(QuizReadinessProblem {
                event_id,
                event_name,
                status,
                reason,
                blocked_attendees,
            })
        })
        .collect();
    Ok(problems)
}

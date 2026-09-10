//! Transactional notification dispatcher. Provider acceptance is not inbox delivery.
//! Never automatically replay an ambiguous send (Cloudflare has no documented idempotency key).
mod content;
mod outbox;
mod policy;
mod prepare;
mod transport;

pub use outbox::{list, list_for_attendee, mark_all_read, mark_read, retry};
use policy::{failure_state, retry_delay};
use prepare::{Prepared, prepare};

use crate::db::d1_safe::safe_all_rows;
use serde::{Deserialize, Serialize};
use worker::{Env, d1::D1Type};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Notification {
    pub id: i64,
    pub event_id: String,
    pub attendee_id: String,
    pub kind: String,
    pub version: String,
    pub status: String,
    pub attempts: i32,
    pub due_at: i64,
    pub attempted_at: Option<i64>,
    pub message_id: Option<String>,
    pub error_code: Option<String>,
    #[serde(default)]
    pub recipient_name: Option<String>,
    #[serde(default)]
    pub recipient_email: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InboxNotification {
    pub id: i64,
    pub kind: String,
    pub event_name: String,
    pub title: String,
    pub body: String,
    pub action_url: String,
    pub action_label: String,
    pub due_at: i64,
    pub read_at: Option<i64>,
}

pub async fn dispatch(env: &Env) -> Result<(), String> {
    if env
        .var("NOTIFICATIONS_ENABLED")
        .map(|v| v.to_string())
        .unwrap_or_default()
        != "1"
    {
        return Ok(());
    }
    // Validate transport before taking any jobs. Disabled/misconfigured systems don't burn attempts.
    let sender = transport::Sender::from_env(env)?;
    let db = env.d1("DB").map_err(|e| e.to_string())?;
    outbox::run_sql(&db, include_str!("sql/recover.sql")).await?;
    // No recipient/payload copies are stored in the queue.
    outbox::run_sql(&db, include_str!("sql/cancel.sql")).await?;
    // A single UPDATE claims each row before sending; overlapping crons cannot take the same job.
    for _ in 0..25 {
        let rows = safe_all_rows(&db.prepare(include_str!("sql/claim.sql"))).await?;
        let Some(value) = rows.into_iter().next() else {
            break;
        };
        let job: Notification = serde_json::from_value(value).map_err(|e| e.to_string())?;
        let result = prepare(&db, &job, &sender.base_url).await;
        let (status, message_id, code, delay) = match result {
            Ok(Prepared::Defer(until)) => {
                // No transport was attempted: rescheduling must not exhaust the retry budget.
                db.prepare(include_str!("sql/defer.sql"))
                    .bind_refs(&[D1Type::Real(until as f64), D1Type::Real(job.id as f64)])
                    .map_err(|e| e.to_string())?
                    .run()
                    .await
                    .map_err(|e| e.to_string())?;
                continue;
            }
            Ok(Prepared::Ready(message)) => match sender.send(&message).await {
                Ok(id) => ("accepted", id, String::new(), 0),
                Err(code) => {
                    let state = failure_state(&code, job.attempts);
                    (state, String::new(), code, retry_delay(job.attempts))
                }
            },
            Ok(Prepared::Cancel) => ("cancelled", String::new(), "NO_LONGER_ELIGIBLE".into(), 0),
            // Read/serialization failures happen before transport: safe to retry.
            Err(_) => (
                if job.attempts < 5 {
                    "pending"
                } else {
                    "failed"
                },
                String::new(),
                "PREPARE_FAILED".into(),
                retry_delay(job.attempts),
            ),
        };
        let settlement = db
            .prepare(include_str!("sql/settle.sql"))
            .bind_refs(&[
                D1Type::Text(status),
                D1Type::Text(&message_id),
                D1Type::Text(&code),
                D1Type::Integer(delay),
                D1Type::Real(job.id as f64),
            ])
            .map_err(|e| e.to_string())?;
        if safe_all_rows(&settlement).await?.is_empty() {
            tracing::warn!(
                notification_id = job.id,
                "notification changed or erased during attempt"
            );
            continue;
        }
        tracing::info!(
            notification_id = job.id,
            status,
            "notification attempt completed"
        );
    }
    Ok(())
}

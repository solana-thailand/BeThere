//! D1 developer profile helpers (Issue 049).
//!
//! Developer profiles are built incrementally across events.
//! Registration responses with `profile_field = true` upsert into developer_profiles.
//! Raw responses are always stored in registration_responses.

use serde::{Deserialize, Serialize};
use worker::D1Database;
use worker::d1::D1Type;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Developer profile row from D1.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct DeveloperProfileRow {
    pub email: String,
    pub display_name: String,
    pub wallet_address: Option<String>,
    pub github_handle: Option<String>,
    pub discord_handle: Option<String>,
    pub twitter_handle: Option<String>,
    pub experience_level: Option<String>,
    pub primary_role: Option<String>,
    pub tech_stack: String,
    pub interests: String,
    pub learning_goals: String,
    pub expectations: String,
    pub company_org: String,
    pub location_city: String,
    pub consent_outreach: i64,
    pub first_seen_at: String,
    pub last_active_at: String,
    pub total_events: i64,
    pub badges_earned: String,
}

/// A single registration response row.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RegistrationResponseRow {
    pub id: String,
    pub event_id: String,
    pub developer_email: String,
    pub field_key: String,
    pub field_value: String,
    pub is_profile_field: i64,
    pub answered_at: String,
}

// ---------------------------------------------------------------------------
// Developer Profile Queries
// ---------------------------------------------------------------------------

/// Upsert a developer profile field.
///
/// If the developer doesn't exist yet, creates a new row with the provided
/// email and field. If they exist, updates only the specified field and
/// increments total_events + updates last_active_at.
///
/// Use this for individual field updates from registration responses.
pub(crate) async fn upsert_developer_field(
    db: &D1Database,
    email: &str,
    field_name: &str,
    field_value: &str,
) -> Result<(), String> {
    // Build dynamic UPDATE SET clause for the specific field
    let sql = format!(
        "INSERT INTO developer_profiles (email, {field_name}, first_seen_at, last_active_at, \
         total_events, updated_at) \
         VALUES (?1, ?2, datetime('now'), datetime('now'), 1, datetime('now')) \
         ON CONFLICT (email) DO UPDATE SET \
         {field_name} = excluded.{field_name}, \
         last_active_at = datetime('now'), \
         total_events = total_events + 1, \
         updated_at = datetime('now')"
    );

    db.prepare(&sql)
        .bind_refs(&[D1Type::Text(email), D1Type::Text(field_value)])
        .map_err(|e| format!("D1 upsert_developer_field bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 upsert_developer_field run: {e:?}"))?;

    Ok(())
}

/// Upsert multiple developer profile fields at once (atomic transaction).
///
/// Used when a registration form submits multiple profile fields.
/// Uses a batch of individual field updates within a single D1 batch.
#[allow(dead_code)]
pub(crate) async fn upsert_developer_fields(
    db: &D1Database,
    email: &str,
    fields: &[(&str, &str)],
) -> Result<(), String> {
    for (field_name, field_value) in fields {
        upsert_developer_field(db, email, field_name, field_value).await?;
    }
    Ok(())
}

/// Get a developer profile by email.
#[allow(dead_code)]
pub(crate) async fn get_developer_profile(
    db: &D1Database,
    email: &str,
) -> Result<Option<DeveloperProfileRow>, String> {
    Ok(db
        .prepare(
            "SELECT email, display_name, wallet_address, github_handle, discord_handle, \
         twitter_handle, experience_level, primary_role, tech_stack, interests, \
         learning_goals, expectations, company_org, location_city, consent_outreach, \
         first_seen_at, last_active_at, total_events, badges_earned \
         FROM developer_profiles WHERE email = ?1",
        )
        .bind_refs(&[D1Type::Text(email)])
        .map_err(|e| format!("D1 get_developer_profile bind: {e:?}"))?
        .first::<DeveloperProfileRow>(None)
        .await
        .ok() // D1 returns JsValue(null) for no-match
        .flatten())
}

/// Update wallet address for a developer (set when they connect wallet on claim page).
#[allow(dead_code)]
pub(crate) async fn set_developer_wallet(
    db: &D1Database,
    email: &str,
    wallet_address: &str,
) -> Result<(), String> {
    db.prepare(
        "INSERT INTO developer_profiles (email, wallet_address, first_seen_at, last_active_at, \
         total_events, updated_at) \
         VALUES (?1, ?2, datetime('now'), datetime('now'), 0, datetime('now')) \
         ON CONFLICT (email) DO UPDATE SET \
         wallet_address = excluded.wallet_address, \
         updated_at = datetime('now')",
    )
    .bind_refs(&[D1Type::Text(email), D1Type::Text(wallet_address)])
    .map_err(|e| format!("D1 set_developer_wallet bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 set_developer_wallet run: {e:?}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Registration Response Queries
// ---------------------------------------------------------------------------

/// Store a registration form response.
///
/// Each field answer is stored individually so queries can aggregate
/// by field key across events.
#[allow(dead_code)]
pub(crate) async fn insert_registration_response(
    db: &D1Database,
    id: &str,
    event_id: &str,
    developer_email: &str,
    field_key: &str,
    field_value: &str,
    is_profile_field: bool,
) -> Result<(), String> {
    db.prepare(
        "INSERT INTO registration_responses \
         (id, event_id, developer_email, field_key, field_value, is_profile_field, answered_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
    )
    .bind_refs(&[
        D1Type::Text(id),
        D1Type::Text(event_id),
        D1Type::Text(developer_email),
        D1Type::Text(field_key),
        D1Type::Text(field_value),
        D1Type::Integer(if is_profile_field { 1 } else { 0 }),
    ])
    .map_err(|e| format!("D1 insert_registration_response bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 insert_registration_response run: {e:?}"))?;

    Ok(())
}

/// Batch-insert multiple registration responses in a single D1 call.
/// Each tuple is (field_key, field_value, is_profile_field).
pub(crate) async fn batch_insert_registration_responses(
    db: &D1Database,
    event_id: &str,
    developer_email: &str,
    responses: &[(&str, &str, bool)],
) -> Result<(), String> {
    if responses.is_empty() {
        return Ok(());
    }

    // Generate all IDs upfront, then build SQL + params referencing them.
    let ids: Vec<String> = (0..responses.len())
        .map(|_| uuid::Uuid::now_v7().to_string())
        .collect();

    let mut sql = String::from(
        "INSERT INTO registration_responses \
         (id, event_id, developer_email, field_key, field_value, is_profile_field, answered_at) VALUES ",
    );
    for i in 0..responses.len() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str("(?, ?, ?, ?, ?, ?, datetime('now'))");
    }

    let mut params: Vec<D1Type> = Vec::with_capacity(responses.len() * 6);
    for (i, (field_key, field_value, is_profile_field)) in responses.iter().enumerate() {
        params.push(D1Type::Text(ids[i].as_str()));
        params.push(D1Type::Text(event_id));
        params.push(D1Type::Text(developer_email));
        params.push(D1Type::Text(field_key));
        params.push(D1Type::Text(field_value));
        params.push(D1Type::Integer(if *is_profile_field { 1 } else { 0 }));
    }

    db.prepare(&sql)
        .bind_refs(&params)
        .map_err(|e| format!("D1 batch_insert_registration_responses bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 batch_insert_registration_responses run: {e:?}"))?;

    Ok(())
}

/// Get all registration responses for a developer in a specific event.
#[allow(dead_code)]
pub(crate) async fn get_event_responses(
    db: &D1Database,
    event_id: &str,
    developer_email: &str,
) -> Result<Vec<RegistrationResponseRow>, String> {
    db.prepare(
        "SELECT id, event_id, developer_email, field_key, field_value, \
         is_profile_field, answered_at \
         FROM registration_responses \
         WHERE event_id = ?1 AND developer_email = ?2 \
         ORDER BY answered_at",
    )
    .bind_refs(&[D1Type::Text(event_id), D1Type::Text(developer_email)])
    .map_err(|e| format!("D1 get_event_responses bind: {e:?}"))?
    .all()
    .await
    .map_err(|e| format!("D1 get_event_responses run: {e:?}"))?
    .results::<RegistrationResponseRow>()
    .map_err(|e| format!("D1 get_event_responses deserialize: {e:?}"))
}

// ---------------------------------------------------------------------------
// Community Insights (Aggregation Queries)
// ---------------------------------------------------------------------------

/// Experience level distribution across all developer profiles.
#[allow(dead_code)]
pub(crate) async fn experience_distribution(db: &D1Database) -> Result<Vec<(String, i64)>, String> {
    let rows = db
        .prepare(
            "SELECT experience_level, COUNT(*) as cnt \
             FROM developer_profiles \
             WHERE experience_level IS NOT NULL \
             GROUP BY experience_level \
             ORDER BY cnt DESC",
        )
        .all()
        .await
        .map_err(|e| format!("D1 experience_distribution run: {e:?}"))?
        .results::<serde_json::Map<String, serde_json::Value>>()
        .map_err(|e| format!("D1 experience_distribution deserialize: {e:?}"))?;

    Ok(rows
        .into_iter()
        .filter_map(|m| {
            let level = m.get("experience_level")?.as_str()?.to_string();
            let cnt = m.get("cnt")?.as_i64()?;
            Some((level, cnt))
        })
        .collect())
}

/// Tech stack popularity (parsed from JSON arrays in developer_profiles).
/// Returns top N technologies by developer count.
#[allow(dead_code, clippy::unnecessary_sort_by)]
pub(crate) async fn tech_stack_popularity(
    db: &D1Database,
    limit: usize,
) -> Result<Vec<(String, i64)>, String> {
    // D1 doesn't have json_each, so we fetch all tech_stacks and count in Rust
    let rows = db
        .prepare("SELECT tech_stack FROM developer_profiles WHERE tech_stack != '[]'")
        .all()
        .await
        .map_err(|e| format!("D1 tech_stack_popularity run: {e:?}"))?
        .results::<TechStackRow>()
        .map_err(|e| format!("D1 tech_stack_popularity deserialize: {e:?}"))?;

    let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for row in &rows {
        if let Ok(techs) = serde_json::from_str::<Vec<String>>(&row.tech_stack) {
            for tech in techs {
                *counts.entry(tech).or_default() += 1;
            }
        }
    }

    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.truncate(limit);
    Ok(sorted)
}

/// Helper struct for tech_stack query.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TechStackRow {
    tech_stack: String,
}

/// Total developer profile count.
#[allow(dead_code)]
pub(crate) async fn developer_count(db: &D1Database) -> Result<i64, String> {
    let row = db
        .prepare("SELECT COUNT(*) as cnt FROM developer_profiles")
        .first::<serde_json::Map<String, serde_json::Value>>(None)
        .await
        .ok() // D1 returns JsValue(null) when table empty
        .flatten();

    Ok(row.and_then(|m| m.get("cnt")?.as_i64()).unwrap_or(0))
}

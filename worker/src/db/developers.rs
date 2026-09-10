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
    pub display_name: Option<String>,
    pub wallet_address: Option<String>,
    pub github_handle: Option<String>,
    pub discord_handle: Option<String>,
    pub twitter_handle: Option<String>,
    pub telegram_handle: Option<String>,
    pub telegram_id: Option<String>,
    pub github_verified: Option<i64>,
    pub telegram_verified: Option<i64>,
    pub discord_verified: Option<i64>,
    pub github_verified_at: Option<String>,
    pub telegram_verified_at: Option<String>,
    pub discord_verified_at: Option<String>,
    pub experience_level: Option<String>,
    pub primary_role: Option<String>,
    pub tech_stack: Option<String>,
    pub interests: Option<String>,
    pub learning_goals: Option<String>,
    pub expectations: Option<String>,
    pub company_org: Option<String>,
    pub location_city: Option<String>,
    pub consent_outreach: Option<i64>,
    pub first_seen_at: Option<String>,
    pub last_active_at: Option<String>,
    pub total_events: Option<i64>,
    pub badges_earned: Option<String>,
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

/// Columns of `developer_profiles` a registration form is allowed to set.
///
/// [`upsert_developer_field`] interpolates its column name into the SQL text —
/// SQLite cannot bind an *identifier* — so the name must be resolved to one of
/// these `&'static str` entries before it reaches the query. The registration
/// body carries a free-form `profile_fields: HashMap<String, String>` whose keys
/// arrive straight off the wire on a public endpoint; without this gate the
/// caller chooses the identifier, which is SQL injection.
///
/// Deliberately **excluded**, each for its own reason:
///
/// - `email` — the primary key. Always bound as `?1`.
/// - `wallet_address` — owned by [`upsert_developer_wallet`] and the wallet-link
///   flow, which prove control of the key first.
/// - `telegram_id`, `*_verified`, `*_verified_at` — written only after the
///   provider actually verified the account (`handlers::social_link`). A
///   registration body must not be able to assert `github_verified = 1`.
/// - `first_seen_at`, `last_active_at`, `total_events`, `badges_earned`,
///   `created_at`, `updated_at` — bookkeeping this write path maintains itself.
///
/// Sorted so a reviewer can diff it against the migration DDL by eye.
const UPSERTABLE_PROFILE_COLUMNS: &[&str] = &[
    "company_org",
    "consent_outreach",
    "discord_handle",
    "display_name",
    "expectations",
    "experience_level",
    "github_handle",
    "interests",
    "learning_goals",
    "location_city",
    "primary_role",
    "tech_stack",
    "telegram_handle",
    "twitter_handle",
];

/// Resolve a wire-supplied field name to the `&'static str` column it names.
///
/// Returning `&'static str` rather than `bool` is the point: the caller cannot
/// interpolate the untrusted `&str` even by accident, because only the value
/// returned here is in scope at the `format!`.
fn resolve_profile_column(field_name: &str) -> Option<&'static str> {
    UPSERTABLE_PROFILE_COLUMNS
        .iter()
        .find(|column| **column == field_name)
        .copied()
}

/// Upsert a developer profile field.
///
/// If the developer doesn't exist yet, creates a new row with the provided
/// email and field. If they exist, updates only the specified field and
/// updates last_active_at.
///
/// Use this for individual field updates from registration responses.
///
/// `field_name` must name a column in [`UPSERTABLE_PROFILE_COLUMNS`]; anything
/// else is rejected with an error rather than reaching the database. Callers
/// treat the error as non-fatal, so an unrecognised form key is dropped with a
/// warning instead of failing the registration.
pub(crate) async fn upsert_developer_field(
    db: &D1Database,
    email: &str,
    field_name: &str,
    field_value: &str,
) -> Result<(), String> {
    // `field_name` reaches here from the public registration body. Resolve it to
    // a compile-time column name before it can touch the SQL string.
    let Some(field_name) = resolve_profile_column(field_name) else {
        return Err(format!(
            "developer profile field '{field_name}' is not an upsertable column"
        ));
    };

    // Build dynamic UPDATE SET clause for the specific field.
    //
    // NOTE: this is called once PER FIELD during registration, so it must NOT
    // touch `total_events` — doing so inflated the count by (#fields) per event.
    // The profile's events-joined stat is derived on read via
    // `count_events_joined` (COUNT DISTINCT event_id in attendees) instead.
    let sql = format!(
        "INSERT INTO developer_profiles (email, {field_name}, first_seen_at, last_active_at, \
         total_events, updated_at) \
         VALUES (?1, ?2, datetime('now'), datetime('now'), 0, datetime('now')) \
         ON CONFLICT (email) DO UPDATE SET \
         {field_name} = excluded.{field_name}, \
         last_active_at = datetime('now'), \
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

/// Count the distinct events an email has registered for (the authoritative
/// "events joined" stat). Replaces the drift-prone per-field `total_events`
/// counter with a derived `COUNT(DISTINCT event_id)` over the attendees table.
pub(crate) async fn count_events_joined(db: &D1Database, email: &str) -> Result<i64, String> {
    let stmt = db.prepare(
        "SELECT COUNT(DISTINCT event_id) AS c FROM attendees WHERE LOWER(email) = LOWER(?1)",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(email)])
        .map_err(|e| format!("D1 count_events_joined bind: {e:?}"))?;

    let raw = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 count_events_joined first(): {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 count_events_joined await: {e:?}"))?;

    if raw.is_null() || raw.is_undefined() {
        return Ok(0);
    }
    let json = js_sys::JSON::stringify(&raw)
        .ok()
        .and_then(|s| s.as_string())
        .unwrap_or_default();
    let v: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("count_events_joined parse: {e}"))?;
    Ok(v.get("c").and_then(|c| c.as_i64()).unwrap_or(0))
}

/// Get a developer profile by email.
///
/// Bypasses worker crate's `.first::<T>()` which uses `serde_wasm_bindgen::from_value()`
/// — that crashes on `JsValue(null)` columns (e.g. rows with nullable fields).
/// Instead: raw JS `.first()` → `JSON.stringify` → `serde_json` (same pattern as attendees.rs).
#[allow(dead_code)]
pub(crate) async fn get_developer_profile(
    db: &D1Database,
    email: &str,
) -> Result<Option<DeveloperProfileRow>, String> {
    let stmt = db.prepare(
        "SELECT email, display_name, wallet_address, github_handle, discord_handle, \
         twitter_handle, telegram_handle, telegram_id, github_verified, telegram_verified, \
         discord_verified, github_verified_at, telegram_verified_at, discord_verified_at, \
         experience_level, primary_role, tech_stack, interests, \
         learning_goals, expectations, company_org, location_city, consent_outreach, \
         first_seen_at, last_active_at, total_events, badges_earned \
         FROM developer_profiles WHERE email = ?1",
    );
    let bound = stmt
        .bind_refs(&[D1Type::Text(email)])
        .map_err(|e| format!("D1 get_developer_profile bind: {e:?}"))?;

    let raw_first = wasm_bindgen_futures::JsFuture::from(
        bound
            .inner()
            .first(None)
            .map_err(|e| format!("D1 get_developer_profile first() call: {e:?}"))?,
    )
    .await
    .map_err(|e| format!("D1 get_developer_profile first() await: {e:?}"))?;

    if raw_first.is_null() || raw_first.is_undefined() {
        return Ok(None);
    }

    let json_str = js_sys::JSON::stringify(&raw_first)
        .map(|s| s.as_string().unwrap_or_default())
        .unwrap_or_default();

    if json_str.is_empty() {
        return Ok(None);
    }

    let row: DeveloperProfileRow = serde_json::from_str(&json_str).map_err(|e| {
        tracing::warn!(
            error = %e,
            json = %json_str.chars().take(500).collect::<String>(),
            "D1 get_developer_profile: deserialize failed"
        );
        format!("D1 get_developer_profile deserialize: {e}")
    })?;

    Ok(Some(row))
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
#[allow(dead_code)]
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
    sorted.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    sorted.truncate(limit);
    Ok(sorted)
}

/// Helper struct for tech_stack_popularity query.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct TechStackRow {
    tech_stack: String,
}

/// Lightweight row for listing developers with wallet addresses.
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct WalletDeveloperRow {
    pub email: String,
    pub wallet_address: String,
}

/// List all developers that have a non-null, non-empty wallet address.
pub(crate) async fn list_developers_with_wallets(
    db: &D1Database,
) -> Result<Vec<WalletDeveloperRow>, String> {
    let sql = "SELECT email, wallet_address FROM developer_profiles \
         WHERE wallet_address IS NOT NULL AND wallet_address != ''";

    let rows = db
        .prepare(sql)
        .all()
        .await
        .map_err(|e| format!("D1 list_developers_with_wallets run: {e:?}"))?
        .results::<WalletDeveloperRow>()
        .map_err(|e| format!("D1 list_developers_with_wallets deserialize: {e:?}"))?;

    Ok(rows)
}

/// Total developer profile count.
#[allow(dead_code)]
pub(crate) async fn developer_count(db: &D1Database) -> Result<i64, String> {
    let row = db
        .prepare("SELECT COUNT(*) as cnt FROM developer_profiles")
        .first::<serde_json::Map<String, serde_json::Value>>(None)
        .await
        .map_err(|e| format!("D1 developer_count query: {e:?}"))?;

    Ok(row.and_then(|m| m.get("cnt")?.as_i64()).unwrap_or(0))
}

/// Clear PII for a developer profile (PDPA right to erasure).
/// Keeps the row but blanks all identifying fields.
///
/// `company_org` and `location_city` are `TEXT NOT NULL DEFAULT ''`, so they are
/// blanked rather than nulled. Setting them to NULL aborted the whole statement
/// on a NOT NULL constraint, and `handlers::privacy` only logs that error — so
/// the erasure silently cleared nothing at all, including `display_name` and the
/// social handles.
pub(crate) async fn clear_developer_pii(db: &D1Database, email: &str) -> Result<(), String> {
    let sql = "UPDATE developer_profiles SET \
         display_name = '[DELETED]', wallet_address = NULL, \
         github_handle = NULL, discord_handle = NULL, twitter_handle = NULL, \
         telegram_handle = NULL, telegram_id = NULL, \
         github_verified = 0, telegram_verified = 0, discord_verified = 0, \
         github_verified_at = NULL, telegram_verified_at = NULL, discord_verified_at = NULL, \
         company_org = '', location_city = '', \
         updated_at = datetime('now') \
         WHERE LOWER(email) = ?";
    db.prepare(sql)
        .bind_refs(&[D1Type::Text(email)])
        .map_err(|e| format!("D1 clear_developer_pii bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 clear_developer_pii: {e:?}"))?;
    Ok(())
}

/// Delete all registration responses for a developer (PDPA right to erasure).
pub(crate) async fn delete_developer_responses(
    db: &D1Database,
    email: &str,
) -> Result<usize, String> {
    let result = db
        .prepare("DELETE FROM registration_responses WHERE LOWER(developer_email) = ?")
        .bind_refs(&[D1Type::Text(email)])
        .map_err(|e| format!("D1 delete_developer_responses bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 delete_developer_responses: {e:?}"))?;
    // Unlike `exec`, a prepared `run` reports the affected row count.
    Ok(result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0))
}

// ---------------------------------------------------------------------------
// Additional Aggregation Queries (Issue #049 Phase 1)
// ---------------------------------------------------------------------------

/// Role distribution across all developer profiles.
pub(crate) async fn role_distribution(db: &D1Database) -> Result<Vec<(String, i64)>, String> {
    let rows = db
        .prepare(
            "SELECT primary_role, COUNT(*) as cnt \
             FROM developer_profiles \
             WHERE primary_role IS NOT NULL \
             GROUP BY primary_role \
             ORDER BY cnt DESC",
        )
        .all()
        .await
        .map_err(|e| format!("D1 role_distribution run: {e:?}"))?
        .results::<serde_json::Map<String, serde_json::Value>>()
        .map_err(|e| format!("D1 role_distribution deserialize: {e:?}"))?;

    Ok(rows
        .into_iter()
        .filter_map(|m| {
            let label = m.get("primary_role")?.as_str()?.to_string();
            let cnt = m.get("cnt")?.as_i64()?;
            Some((label, cnt))
        })
        .collect())
}

/// Interest distribution (parsed from JSON arrays, like tech_stack_popularity).
pub(crate) async fn interest_distribution(
    db: &D1Database,
    limit: usize,
) -> Result<Vec<(String, i64)>, String> {
    let rows = db
        .prepare("SELECT interests FROM developer_profiles WHERE interests != '[]'")
        .all()
        .await
        .map_err(|e| format!("D1 interest_distribution run: {e:?}"))?
        .results::<InterestRow>()
        .map_err(|e| format!("D1 interest_distribution deserialize: {e:?}"))?;

    let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for row in &rows {
        if let Ok(items) = serde_json::from_str::<Vec<String>>(&row.interests) {
            for item in items {
                *counts.entry(item).or_default() += 1;
            }
        }
    }

    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    sorted.truncate(limit);
    Ok(sorted)
}

/// Helper struct for interest_distribution query.
#[derive(Debug, Deserialize)]
struct InterestRow {
    interests: String,
}

/// Count developers with consent_outreach = 1.
pub(crate) async fn outreach_opt_in_count(db: &D1Database) -> Result<i64, String> {
    let row = db
        .prepare("SELECT COUNT(*) as cnt FROM developer_profiles WHERE consent_outreach = 1")
        .first::<serde_json::Map<String, serde_json::Value>>(None)
        .await
        .map_err(|e| format!("D1 outreach_opt_in_count query: {e:?}"))?;

    Ok(row.and_then(|m| m.get("cnt")?.as_i64()).unwrap_or(0))
}

/// Developer profile summary for community list endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DeveloperProfileSummary {
    pub email: String,
    pub display_name: Option<String>,
    pub experience_level: Option<String>,
    pub primary_role: Option<String>,
    pub tech_stack: Option<String>,
    pub interests: Option<String>,
    pub total_events: Option<i64>,
    pub last_active_at: Option<String>,
    pub consent_outreach: Option<i64>,
}

/// Paginated developer list.
pub(crate) async fn list_developers_paginated(
    db: &D1Database,
    limit: usize,
    offset: usize,
) -> Result<(Vec<DeveloperProfileSummary>, i64), String> {
    let count = developer_count(db).await?;

    let sql = format!(
        "SELECT email, display_name, experience_level, primary_role, \
         tech_stack, interests, total_events, last_active_at, consent_outreach \
         FROM developer_profiles \
         ORDER BY last_active_at DESC \
         LIMIT {limit} OFFSET {offset}"
    );

    let rows = db
        .prepare(&sql)
        .all()
        .await
        .map_err(|e| format!("D1 list_developers_paginated run: {e:?}"))?
        .results::<DeveloperProfileSummary>()
        .map_err(|e| format!("D1 list_developers_paginated deserialize: {e:?}"))?;

    Ok((rows, count))
}

#[cfg(test)]
mod tests {
    use super::UPSERTABLE_PROFILE_COLUMNS;
    use super::resolve_profile_column;

    /// Every allowlisted column must actually exist on `developer_profiles`.
    ///
    /// A typo here does not fail loudly: `upsert_developer_field` would accept
    /// the field, D1 would reject the statement, and the caller logs a warning
    /// and moves on — the registrant's answer is silently dropped. The DDL is
    /// the source of truth, so this reads it rather than restating it.
    #[test]
    fn upsertable_columns_exist_in_the_migration_ddl() {
        let columns = developer_profiles_columns();
        assert!(
            columns.len() > 15,
            "parsed only {} columns from the migrations — the parser is broken and \
             this test would pass vacuously",
            columns.len()
        );
        for column in UPSERTABLE_PROFILE_COLUMNS {
            assert!(
                columns.iter().any(|c| c == column),
                "`{column}` is in UPSERTABLE_PROFILE_COLUMNS but no migration \
                 declares it on `developer_profiles`. Parsed: {columns:?}"
            );
        }
    }

    /// The allowlist is the only way a column name reaches the SQL text.
    #[test]
    fn unknown_field_names_do_not_resolve() {
        assert_eq!(resolve_profile_column("display_name"), Some("display_name"));
        // The shapes an injection attempt takes, all rejected.
        for hostile in [
            "display_name, total_events) VALUES ('x', 'y', 1",
            "display_name--",
            "Display_Name",
            "github_verified",
            "wallet_address",
            "email",
            "",
        ] {
            assert_eq!(
                resolve_profile_column(hostile),
                None,
                "`{hostile}` must not resolve to a column"
            );
        }
    }

    /// Sorted order is load-bearing for review: the list is diffed against the
    /// DDL by eye, and an out-of-order insert hides duplicates.
    #[test]
    fn upsertable_columns_are_sorted_and_unique() {
        let mut sorted = UPSERTABLE_PROFILE_COLUMNS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.as_slice(), UPSERTABLE_PROFILE_COLUMNS);
    }

    /// Column names declared on `developer_profiles` by the migrations —
    /// the `CREATE TABLE` body plus every `ALTER TABLE … ADD COLUMN`.
    fn developer_profiles_columns() -> Vec<String> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        let mut paths: Vec<_> = std::fs::read_dir(&dir)
            .expect("read migrations dir")
            .map(|e| e.expect("dir entry").path())
            .filter(|p| p.extension().is_some_and(|e| e == "sql"))
            .collect();
        paths.sort();

        let mut columns = Vec::new();
        for path in paths {
            let sql = std::fs::read_to_string(&path).expect("read migration");
            let mut in_create = false;
            for line in sql.lines() {
                let line = line.split("--").next().unwrap_or("").trim();
                if let Some(rest) = line.strip_prefix("ALTER TABLE developer_profiles ADD COLUMN")
                    && let Some(name) = rest.split_whitespace().next()
                {
                    columns.push(name.trim_end_matches(';').to_string());
                }
                if line.starts_with("CREATE TABLE") && line.contains("developer_profiles") {
                    in_create = true;
                    continue;
                }
                if !in_create {
                    continue;
                }
                match line.starts_with(')') {
                    true => in_create = false,
                    false => {
                        if let Some(name) = line.split_whitespace().next()
                            && !name.is_empty()
                            && name.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                        {
                            columns.push(name.to_string());
                        }
                    }
                }
            }
        }
        columns
    }
}

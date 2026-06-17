//! D1 contact query helpers for dual-write (Phase 2a).
//!
//! Every write to Google Sheets contacts also writes to D1.

use super::d1_safe::safe_all_rows;
use worker::D1Database;
use worker::d1::D1Type;

/// Upsert a contact in D1 when a registration occurs.
///
/// On conflict (same email), updates name, last_registered, events_joined,
/// event_count, and contact preferences.
pub(crate) async fn upsert_contact(
    db: &D1Database,
    email: &str,
    name: &str,
    events_joined: &str,
    event_count: i32,
    contact_channel: &str,
    contact_handle: &str,
) -> Result<(), String> {
    let stmt = db.prepare(
        "INSERT INTO contacts (email, name, first_registered, last_registered, \
         events_joined, event_count, contact_channel, contact_handle) \
         VALUES (?1, ?2, datetime('now'), datetime('now'), ?3, ?4, ?5, ?6) \
         ON CONFLICT (email) DO UPDATE SET \
         name = excluded.name, \
         last_registered = datetime('now'), \
         events_joined = ?3, \
         event_count = ?4, \
         contact_channel = excluded.contact_channel, \
         contact_handle = excluded.contact_handle",
    );
    stmt.bind_refs(&[
        D1Type::Text(email),
        D1Type::Text(name),
        D1Type::Text(events_joined),
        D1Type::Integer(event_count),
        D1Type::Text(contact_channel),
        D1Type::Text(contact_handle),
    ])
    .map_err(|e| format!("D1 upsert_contact bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 upsert_contact run: {e:?}"))?;

    Ok(())
}

/// Update deposit credit for a contact (rolling balance across events).
#[allow(dead_code)]
pub(crate) async fn update_deposit_credit(
    db: &D1Database,
    email: &str,
    credit_thb: i64,
    credit_usdc: i64,
) -> Result<(), String> {
    let stmt = db.prepare(
        "UPDATE contacts \
         SET deposit_credit_thb = ?1, deposit_credit_usdc = ?2, \
         deposit_credit_since = datetime('now') \
         WHERE email = ?3",
    );
    stmt.bind_refs(&[
        D1Type::Integer(credit_thb as i32),
        D1Type::Integer(credit_usdc as i32),
        D1Type::Text(email),
    ])
    .map_err(|e| format!("D1 update_deposit_credit bind: {e:?}"))?
    .run()
    .await
    .map_err(|e| format!("D1 update_deposit_credit run: {e:?}"))?;

    Ok(())
}

/// Clear PII for a contact (PDPA right to erasure).
/// Keeps the row but blanks name and contact fields.
pub(crate) async fn clear_contact_pii(db: &D1Database, email: &str) -> Result<(), String> {
    let sql = format!(
        "UPDATE contacts SET \
         name = '[DELETED]', \
         contact_channel = NULL, contact_handle = NULL, \
         last_registered = datetime('now') \
         WHERE LOWER(email) = '{email}'"
    );
    db.exec(&sql)
        .await
        .map_err(|e| format!("D1 clear_contact_pii: {e:?}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Cross-event audience aggregation (source of truth: attendees table)
// ---------------------------------------------------------------------------

/// One row in the cross-event audience aggregation (one per distinct email).
///
/// Computed fresh from the `attendees` table via `GROUP BY LOWER(email)`, so the
/// counts are always accurate and never suffer from the denormalization drift
/// that the `contacts.events_joined` CSV column is subject to.
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct AudienceRow {
    /// Lowercased email (the group key).
    pub email: String,
    /// Most recent non-empty name seen for this email across registrations.
    #[serde(default)]
    pub name: String,
    /// Distinct events this email registered for (`COUNT(DISTINCT event_id)`).
    #[serde(default)]
    pub events_joined: i64,
    /// Registrations where the attendee actually checked in.
    #[serde(default)]
    pub checked_in_count: i64,
    /// Registrations with `approval_status = 'approved'`.
    #[serde(default)]
    pub approved_count: i64,
    /// Registrations whose participation type is in-person (or empty/legacy).
    #[serde(default)]
    pub in_person_count: i64,
    /// Registrations whose participation type is online/virtual.
    #[serde(default)]
    pub online_count: i64,
    /// Earliest `created_at` across this email's registrations.
    pub first_registered: Option<String>,
    /// Latest `created_at` across this email's registrations.
    pub last_registered: Option<String>,
    /// Comma-separated distinct event IDs this email joined.
    #[serde(default)]
    pub event_ids: String,
    // ── Optional enrichment from `developer_profiles` (NULL when no profile) ──
    pub display_name: Option<String>,
    pub experience_level: Option<String>,
    pub primary_role: Option<String>,
    pub location_city: Option<String>,
    pub wallet_address: Option<String>,
    /// PDPA outreach consent (0/1). Defaults to 0 when no profile row.
    #[serde(default)]
    pub consent_outreach: i64,
}

/// Aggregate the audience across the given events (or ALL events when
/// `event_ids` is `None` or empty).
///
/// Deduplicates by `LOWER(email)` and computes per-email participation metrics,
/// enriched with a LEFT JOIN to `developer_profiles`.
///
/// Source of truth: the `attendees` table. This intentionally does NOT read the
/// `contacts.events_joined` CSV column, which is denormalized and drifts.
///
/// `event_ids` filtering uses a parameterized `IN (...)` clause so the
/// organizer-supplied IDs are never interpolated into SQL text.
pub async fn audience_aggregate(
    db: &D1Database,
    event_ids: Option<&[String]>,
) -> Result<Vec<AudienceRow>, String> {
    // Build the optional `WHERE a.event_id IN (?, ?, ...)` clause together with
    // the matching positional bind list.
    let (where_clause, binds): (String, Vec<D1Type>) = match event_ids {
        Some(ids) if !ids.is_empty() => {
            let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();
            let clause = format!("WHERE a.event_id IN ({})", placeholders.join(", "));
            let binds: Vec<D1Type> = ids
                .iter()
                .map(|id| D1Type::Text(id.as_str()))
                .collect::<Vec<D1Type>>();
            (clause, binds)
        }
        _ => (String::new(), Vec::new()),
    };

    // in-person matching mirrors `Attendee::is_in_person`:
    //   empty/null ⇒ in-person (legacy default), else substring match on the
    //   canonical "in-person" / "in person" / "in_person" spellings.
    let sql = format!(
        "SELECT \
         LOWER(a.email) AS email, \
         MAX(a.name)    AS name, \
         COUNT(DISTINCT a.event_id) AS events_joined, \
         SUM(CASE WHEN a.checked_in_at IS NOT NULL THEN 1 ELSE 0 END) AS checked_in_count, \
         SUM(CASE WHEN LOWER(a.approval_status) = 'approved' THEN 1 ELSE 0 END) AS approved_count, \
         SUM(CASE \
             WHEN a.participation_type IS NULL \
               OR TRIM(a.participation_type) = '' \
               OR LOWER(a.participation_type) LIKE '%in-person%' \
               OR LOWER(a.participation_type) LIKE '%in person%' \
               OR LOWER(a.participation_type) LIKE '%in_person%' \
             THEN 1 ELSE 0 \
         END) AS in_person_count, \
         SUM(CASE \
             WHEN a.participation_type IS NULL \
               OR TRIM(a.participation_type) = '' \
               OR LOWER(a.participation_type) LIKE '%in-person%' \
               OR LOWER(a.participation_type) LIKE '%in person%' \
               OR LOWER(a.participation_type) LIKE '%in_person%' \
             THEN 0 ELSE 1 \
         END) AS online_count, \
         MIN(a.created_at) AS first_registered, \
         MAX(a.created_at) AS last_registered, \
         GROUP_CONCAT(DISTINCT a.event_id) AS event_ids, \
         dp.display_name      AS display_name, \
         dp.experience_level  AS experience_level, \
         dp.primary_role      AS primary_role, \
         dp.location_city     AS location_city, \
         dp.wallet_address    AS wallet_address, \
         COALESCE(dp.consent_outreach, 0) AS consent_outreach \
         FROM attendees a \
         LEFT JOIN developer_profiles dp ON dp.email = LOWER(a.email) \
         {where_clause} \
         GROUP BY LOWER(a.email) \
         ORDER BY events_joined DESC, checked_in_count DESC, email"
    );

    // Skip `bind_refs` entirely when there are no placeholders — matches the
    // established `get_attendees_by_email` pattern and avoids any edge case
    // around binding an empty array on a parameter-less statement.
    let stmt = db.prepare(&sql);
    let bound = if binds.is_empty() {
        stmt
    } else {
        stmt.bind_refs(&binds)
            .map_err(|e| format!("D1 audience_aggregate bind: {e:?}"))?
    };

    let rows = safe_all_rows(&bound)
        .await
        .map_err(|e| format!("D1 audience_aggregate execute: {e}"))?;

    // Each row comes back as serde_json::Value; NULL cells map cleanly to the
    // Option fields on AudienceRow.
    rows.into_iter()
        .map(|v| {
            serde_json::from_value::<AudienceRow>(v)
                .map_err(|e| format!("D1 audience_aggregate deserialize: {e}"))
        })
        .collect()
}

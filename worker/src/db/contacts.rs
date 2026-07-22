//! D1 contact query helpers for dual-write (Phase 2a).
//!
//! Every write to Google Sheets contacts also writes to D1.
//!
//! # Source of truth: `attendees` table (not `contacts.events_joined`)
//!
//! The `contacts.events_joined` CSV column is a denormalized,
//! overwrite-on-every-upsert field that is **not queryable** and drifts from
//! reality. It is kept in sync on writes for backward compatibility with any
//! external reader, but it is **deprecated as a read path**. Any new code
//! asking "which events did this contact attend?" must use [`list_contact_events`],
//! which JOINs `attendees → events` directly. Full column removal is tracked
//! as follow-up tech debt (out of scope for Plan 008).

use super::d1_safe::safe_all_rows;
use worker::D1Database;
use worker::d1::D1Type;

/// Upsert a contact in D1 when a registration occurs.
///
/// On conflict (same email), updates name, last_registered, events_joined,
/// event_count, and contact preferences.
///
/// # Deprecated read path: `events_joined` CSV column
///
/// The `events_joined` column is a comma-separated list of event IDs. It is
/// denormalized, overwritten on every upsert, **not queryable** (no
/// `LIKE`/`member-of` predicate on CSV is reliable), and drifts from the
/// source of truth under any code path that mutates `attendees` without
/// re-upserting the contact row.
///
/// **Read path is deprecated.** The source of truth for "which events did
/// this contact attend" is the `attendees` table — see [`list_contact_events`].
/// This write path continues to update `events_joined` for backward
/// compatibility with any external consumer that reads the CSV directly, but
/// no new read code should depend on it.
///
/// **Full removal (dropping the column + these write paths) is out of scope**
/// for Plan 008 and is tracked as follow-up tech debt: it requires a migration
/// to drop the column plus an audit of every upsert caller.
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

// ---------------------------------------------------------------------------
// Deposit-credit liability aggregate (Issue #061 Phase 2 — option a2 chip)
// ---------------------------------------------------------------------------

/// Total deposit-credit held by the organizer across all contacts.
///
/// The single figure shown in the admin "Total credit held" header chip — the
/// organizer's cash liability from rolling deposit credit (THB held on behalf
/// of attendees who chose credit over refund).
///
/// `contact_count` is the number of contacts with a non-zero balance in EITHER
/// currency (not a sum of per-currency counts) — matches how the chip phrases
/// "across N contacts". Both `total_*` use `COALESCE` so an empty `contacts`
/// table yields `(0, 0, 0)` instead of a NULL row.
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct CreditLiability {
    /// Sum of `deposit_credit_thb` across contacts with any credit.
    #[serde(default)]
    pub total_thb: i64,
    /// Sum of `deposit_credit_usdc` across contacts with any credit.
    #[serde(default)]
    pub total_usdc: i64,
    /// Number of distinct contacts with a non-zero balance in either currency.
    #[serde(default)]
    pub contact_count: i64,
}

/// Aggregate the organizer's total deposit-credit liability.
///
/// One round-trip SUM/COUNT over the `contacts` table — the cheapest possible
/// read for the admin liability chip. Source of truth is the `contacts` table
/// (D1 cols K–M), the same rows `increment_credit` writes. Single-org scope:
/// the table is not org-partitioned (Issue #029 multi-org isolation deferred).
///
/// Returns `CreditLiability::default()` (all zeros) when D1 is unreachable —
/// the chip renders "0 THB" rather than failing the deposits view.
pub async fn credit_liability(db: &D1Database) -> CreditLiability {
    let sql = "\
         SELECT \
           COALESCE(SUM(deposit_credit_thb), 0)  AS total_thb, \
           COALESCE(SUM(deposit_credit_usdc), 0) AS total_usdc, \
           COUNT(*)                               AS contact_count \
         FROM contacts \
         WHERE deposit_credit_thb > 0 OR deposit_credit_usdc > 0";

    let stmt = db.prepare(sql);
    match safe_all_rows(&stmt).await {
        Ok(rows) => rows
            .into_iter()
            .next()
            .and_then(|v| serde_json::from_value::<CreditLiability>(v).ok())
            .unwrap_or_default(),
        // Non-fatal: the deposits view must still render without the chip's
        // number. Logged upstream if needed; here we degrade to zero.
        Err(_) => CreditLiability::default(),
    }
}

// ---------------------------------------------------------------------------
// Per-contact event history (Plan 008 §3.5 — read-side fix)
// ---------------------------------------------------------------------------

/// One row in a contact's event history: the event identity + how this contact
/// participated in it.
///
/// Returned by [`list_contact_events`]. This is deliberately **not** the domain
/// `EventMeta`: a contact-history view needs per-registration detail
/// (`checked_in_at`, `participation_type`, `registered_at`) that the listing
/// summary does not carry. Mirrors the `AudienceRow` convention (db-layer
/// serializable struct tied to the `attendees`-table read).
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct ContactEventRow {
    /// Event identity (FK → `events.id`).
    pub event_id: String,
    /// Display name (e.g. "Solana x AI: Bangkok").
    #[serde(default)]
    pub name: String,
    /// URL-friendly slug.
    #[serde(default)]
    pub slug: String,
    /// Tagline / subtitle. Empty string when unset.
    #[serde(default)]
    pub tagline: String,
    /// Location string. Empty when unset.
    #[serde(default)]
    pub location: String,
    /// Current lifecycle status (draft/active/completed/archived).
    #[serde(default)]
    pub status: String,
    /// In-person / online / hybrid.
    #[serde(default)]
    pub event_format: String,
    /// Event start as Unix epoch milliseconds.
    #[serde(default)]
    pub event_start_ms: i64,
    /// Event end as Unix epoch milliseconds.
    #[serde(default)]
    pub event_end_ms: i64,
    // ── Per-registration detail (from the attendees row) ────────────────
    /// When this contact registered for the event (`attendees.created_at`).
    #[serde(default)]
    pub registered_at: String,
    /// In-person vs online, as stored on the attendees row.
    #[serde(default)]
    pub participation_type: String,
    /// ISO 8601 check-in timestamp. `None` when the contact never checked in —
    /// the signal for "no-show" (within the in-person slice, per Plan 008).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_in_at: Option<String>,
    /// Approval status snapshot. Scoped to `'approved'` by the query, but
    /// surfaced for transparency (e.g. future "pending" history).
    #[serde(default)]
    pub approval_status: String,
}

/// List every event a contact has attended, with per-registration detail.
///
/// JOINs `attendees → events` for the given email, scoped to the canonical
/// "attended" slice per Plan 008 §3.5:
///   - `registration_phase = 'pre_event'` (post-event regs are lead capture,
///     not attendance)
///   - `approval_status = 'approved'` (pending/rejected are not attendance)
///
/// This is the **source of truth** for contact history — the
/// `contacts.events_joined` CSV column is deprecated as a read path (see the
/// crate-level doc). Ordered by `event_start_ms DESC` so the most recent event
/// appears first.
///
/// The email is bound as a positional SQL parameter (not interpolated) to
/// guard against SQL injection, mirroring the parameterized `audience_aggregate`
/// pattern rather than the `exec`-with-format pattern used by `clear_contact_pii`.
pub async fn list_contact_events(
    db: &D1Database,
    email: &str,
) -> Result<Vec<ContactEventRow>, String> {
    let sql = "\
         SELECT \
           e.id               AS event_id, \
           e.name             AS name, \
           e.slug             AS slug, \
           e.tagline          AS tagline, \
           e.location         AS location, \
           e.status           AS status, \
           e.event_format     AS event_format, \
           e.event_start_ms   AS event_start_ms, \
           e.event_end_ms     AS event_end_ms, \
           a.created_at       AS registered_at, \
           a.participation_type AS participation_type, \
           a.checked_in_at    AS checked_in_at, \
           a.approval_status  AS approval_status \
         FROM attendees a \
         JOIN events e ON e.id = a.event_id \
         WHERE LOWER(a.email) = LOWER(?1) \
           AND a.registration_phase = 'pre_event' \
           AND LOWER(a.approval_status) = 'approved' \
         ORDER BY e.event_start_ms DESC, e.id DESC";

    let stmt = db.prepare(sql);
    let bound = stmt
        .bind_refs(&[D1Type::Text(email)])
        .map_err(|e| format!("D1 list_contact_events bind: {e:?}"))?;

    let rows = safe_all_rows(&bound)
        .await
        .map_err(|e| format!("D1 list_contact_events execute: {e}"))?;

    rows.into_iter()
        .map(|v| {
            serde_json::from_value::<ContactEventRow>(v)
                .map_err(|e| format!("D1 list_contact_events deserialize: {e}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Full row with every column populated, including a check-in timestamp.
    /// Locks the column-name → field mapping (the SELECT aliases must line up
    /// exactly with the `ContactEventRow` field names for serde to bind them).
    #[test]
    fn contact_event_row_maps_all_columns() {
        let row = serde_json::json!({
            "event_id": "evt-bangkok-2025",
            "name": "Solana x AI: Bangkok",
            "slug": "solana-bangkok-2025",
            "tagline": "The Road to Mainnet",
            "location": "Bangkok, Thailand",
            "status": "completed",
            "event_format": "in_person",
            "event_start_ms": 1_700_000_000_000_i64,
            "event_end_ms": 1_700_003_600_000_i64,
            "registered_at": "2024-11-10T08:30:00Z",
            "participation_type": "in_person",
            "checked_in_at": "2024-11-14T22:45:00Z",
            "approval_status": "approved"
        });
        let r: ContactEventRow = serde_json::from_value(row).expect("full row must deserialize");
        assert_eq!(r.event_id, "evt-bangkok-2025");
        assert_eq!(r.name, "Solana x AI: Bangkok");
        assert_eq!(r.slug, "solana-bangkok-2025");
        assert_eq!(r.tagline, "The Road to Mainnet");
        assert_eq!(r.location, "Bangkok, Thailand");
        assert_eq!(r.status, "completed");
        assert_eq!(r.event_format, "in_person");
        assert_eq!(r.event_start_ms, 1_700_000_000_000);
        assert_eq!(r.event_end_ms, 1_700_003_600_000);
        assert_eq!(r.registered_at, "2024-11-10T08:30:00Z");
        assert_eq!(r.participation_type, "in_person");
        assert_eq!(r.checked_in_at.as_deref(), Some("2024-11-14T22:45:00Z"));
        assert_eq!(r.approval_status, "approved");
    }

    /// `checked_in_at` is NULL for a registrant who never checked in. This is
    /// the signal for no-show within the in-person slice (Plan 008 §3.1.2), so
    /// NULL must deserialize to `None` cleanly — never panic, never become the
    /// string `"null"`, and never omit the field from the struct entirely.
    #[test]
    fn contact_event_row_null_checked_in_at_becomes_none() {
        let row = serde_json::json!({
            "event_id": "evt-2",
            "name": "Missed Event",
            "slug": "missed",
            "tagline": "",
            "location": "",
            "status": "completed",
            "event_format": "in_person",
            "event_start_ms": 0_i64,
            "event_end_ms": 0_i64,
            "registered_at": "2024-11-10T08:30:00Z",
            "participation_type": "in_person",
            "checked_in_at": null,
            "approval_status": "approved"
        });
        let r: ContactEventRow =
            serde_json::from_value(row).expect("null checked_in_at must deserialize");
        assert!(
            r.checked_in_at.is_none(),
            "NULL checked_in_at must be None, not Some(\"null\") or Some(\"\")"
        );
    }

    /// A row missing optional columns entirely (not just NULL — absent keys).
    /// This guards forward-compat: if the `events` table gains new columns and
    /// the SELECT here doesn't alias them yet, or an older D1 row predates a
    /// column, `#[serde(default)]` keeps deserialization from failing.
    #[test]
    fn contact_event_row_missing_columns_default_cleanly() {
        let row = serde_json::json!({
            "event_id": "evt-3",
            "name": "Legacy Event",
            "slug": "legacy",
            "event_start_ms": 1_000_i64,
            "event_end_ms": 2_000_i64,
            "registered_at": "2024-01-01T00:00:00Z"
        });
        let r: ContactEventRow = serde_json::from_value(row)
            .expect("partial row must deserialize via #[serde(default)]");
        assert_eq!(r.event_id, "evt-3");
        assert_eq!(r.name, "Legacy Event");
        assert_eq!(r.slug, "legacy");
        assert_eq!(r.tagline, "", "missing tagline defaults to empty string");
        assert_eq!(r.location, "");
        assert_eq!(r.status, "");
        assert_eq!(r.event_format, "");
        assert_eq!(r.event_start_ms, 1_000);
        assert_eq!(r.event_end_ms, 2_000);
        assert_eq!(r.registered_at, "2024-01-01T00:00:00Z");
        assert_eq!(r.participation_type, "");
        assert!(
            r.checked_in_at.is_none(),
            "missing checked_in_at defaults to None"
        );
        assert_eq!(r.approval_status, "");
    }

    /// Serializing a row with `checked_in_at = None` must omit the field
    /// entirely (via `skip_serializing_if = "Option::is_none"`), so the wire
    /// payload stays clean for the frontend. A row with a check-in timestamp
    /// must include it. This is the contract the organizer-history UI depends
    /// on: presence of `checked_in_at` === "this contact showed up".
    #[test]
    fn contact_event_row_skip_serializing_when_checked_in_at_none() {
        let without_checkin = ContactEventRow {
            event_id: "evt-4".into(),
            name: "No-Show".into(),
            slug: "no-show".into(),
            event_start_ms: 100,
            event_end_ms: 200,
            checked_in_at: None,
            ..Default::default()
        };
        let json = serde_json::to_string(&without_checkin).expect("must serialize");
        assert!(
            !json.contains("checked_in_at"),
            "None checked_in_at must be omitted from wire payload, got: {json}"
        );

        let with_checkin = ContactEventRow {
            checked_in_at: Some("2024-11-14T22:45:00Z".into()),
            ..without_checkin
        };
        let json = serde_json::to_string(&with_checkin).expect("must serialize");
        assert!(
            json.contains("\"checked_in_at\":\"2024-11-14T22:45:00Z\""),
            "Some checked_in_at must be present in wire payload, got: {json}"
        );
    }
}

//! Append-only, ORG-SCOPED credit ledger — the source of truth for rolling
//! deposit credit.
//!
//! Balance = `SUM(delta)` over `(email, organization_id, currency)`.
//!
//! Replaces the mutable `deposit_credit_thb` cell in the Google Contacts sheet,
//! which lost credit under non-atomic best-effort writes and shadowed it behind
//! duplicate contact rows (credit incident 2026-08-14). Append-only ⇒ auditable
//! and race-free; idempotent per `(deposit_id, reason)` ⇒ a re-fired hold/apply
//! cannot double-move money; org-scoped ⇒ Org A's credit never spends at Org B
//! (Issue #029 multi-org isolation).
//!
//! See migration `0028_credit_ledger.sql`. The sheet stays as a display mirror.

use super::d1_safe::safe_all_rows;
use worker::{D1Database, D1Type};

/// Audit reason label for a hold entry (deposit converted to rolling credit).
/// `apply` (spend) is written inline by [`try_spend`]; `backfill` is used by the
/// SQL backfill.
pub const REASON_HOLD: &str = "hold";
/// Audit reason label for a refund reversal (organizer paid the held credit back
/// out-of-band → remove it from the ledger).
pub const REASON_REFUND: &str = "refund";

/// Record a signed credit movement.
///
/// `delta` is positive to grant (hold / refund-in / backfill) and negative to
/// spend (apply to a new event's deposit). Idempotent per `(deposit_id, reason)`:
/// if a matching row already exists this is a no-op returning `Ok(false)`; a
/// fresh insert returns `Ok(true)`. Pass `deposit_id = None` for manual adjusts
/// (always inserts). `email` is lowercased here.
#[allow(clippy::too_many_arguments)]
pub async fn record(
    db: &D1Database,
    email: &str,
    organization_id: &str,
    currency: &str,
    delta: i64,
    reason: &str,
    event_id: Option<&str>,
    deposit_id: Option<&str>,
    note: Option<&str>,
) -> Result<bool, String> {
    let email_lc = email.to_lowercase();
    let currency_lc = currency.to_lowercase();
    let sql = "INSERT INTO credit_ledger \
               (email, organization_id, currency, delta, reason, event_id, deposit_id, note) \
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
               ON CONFLICT (deposit_id, reason) WHERE deposit_id IS NOT NULL DO NOTHING";
    let result = db
        .prepare(sql)
        .bind_refs(&[
            D1Type::Text(&email_lc),
            D1Type::Text(organization_id),
            D1Type::Text(&currency_lc),
            D1Type::Integer(delta as i32),
            D1Type::Text(reason),
            event_id.map(D1Type::Text).unwrap_or(D1Type::Null),
            deposit_id.map(D1Type::Text).unwrap_or(D1Type::Null),
            note.map(D1Type::Text).unwrap_or(D1Type::Null),
        ])
        .map_err(|e| format!("D1 credit_ledger record bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 credit_ledger record run: {e:?}"))?;
    let changes = result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0);
    Ok(changes > 0)
}

/// Atomically spend `amount` of credit for a new event's deposit.
///
/// Inserts a single `-amount` `apply` entry **iff** the current balance is
/// sufficient — the guard and the insert are one statement, so two concurrent
/// registrations for the same email can't both spend the same credit (no
/// advisory lock or Sheets re-read needed). Idempotent per `(deposit_id, apply)`.
/// Returns `Ok(true)` if the credit was spent, `Ok(false)` if the balance was
/// insufficient (or already spent for this deposit) — caller then charges
/// normally and keeps the credit. `deposit_id` should uniquely identify this
/// registration (e.g. `apply:{event_id}:{email}`).
#[allow(clippy::too_many_arguments)]
pub async fn try_spend(
    db: &D1Database,
    email: &str,
    organization_id: &str,
    currency: &str,
    amount: i64,
    event_id: &str,
    deposit_id: &str,
) -> Result<bool, String> {
    let email_lc = email.to_lowercase();
    let currency_lc = currency.to_lowercase();
    let sql = "INSERT INTO credit_ledger \
               (email, organization_id, currency, delta, reason, event_id, deposit_id) \
               SELECT ?1, ?2, ?3, -1 * ?4, 'apply', ?5, ?6 \
               WHERE (SELECT COALESCE(SUM(delta), 0) FROM credit_ledger \
                      WHERE email = ?1 AND organization_id = ?2 AND currency = ?3) >= ?4 \
               ON CONFLICT (deposit_id, reason) WHERE deposit_id IS NOT NULL DO NOTHING";
    let result = db
        .prepare(sql)
        .bind_refs(&[
            D1Type::Text(&email_lc),
            D1Type::Text(organization_id),
            D1Type::Text(&currency_lc),
            D1Type::Integer(amount as i32),
            D1Type::Text(event_id),
            D1Type::Text(deposit_id),
        ])
        .map_err(|e| format!("D1 credit_ledger try_spend bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 credit_ledger try_spend run: {e:?}"))?;
    let changes = result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0);
    Ok(changes > 0)
}

/// Current credit balance for `(email, organization_id, currency)`.
///
/// Returns the signed `SUM(delta)` (never negative in practice — apply is gated
/// on sufficient balance). D1/read errors bubble up so callers can fail closed
/// (never grant a free deposit on an unknown balance).
pub async fn balance(
    db: &D1Database,
    email: &str,
    organization_id: &str,
    currency: &str,
) -> Result<i64, String> {
    let email_lc = email.to_lowercase();
    let currency_lc = currency.to_lowercase();
    let sql = "SELECT COALESCE(SUM(delta), 0) AS bal FROM credit_ledger \
               WHERE email = ?1 AND organization_id = ?2 AND currency = ?3";
    let stmt = db
        .prepare(sql)
        .bind_refs(&[
            D1Type::Text(&email_lc),
            D1Type::Text(organization_id),
            D1Type::Text(&currency_lc),
        ])
        .map_err(|e| format!("D1 credit_ledger balance bind: {e:?}"))?;
    let rows = safe_all_rows(&stmt).await?;
    let bal = rows
        .into_iter()
        .next()
        .and_then(|v| v.get("bal").and_then(serde_json::Value::as_i64))
        .unwrap_or(0);
    Ok(bal)
}

/// One row of the org-partitioned liability report.
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct OrgLiability {
    #[serde(default)]
    pub organization_id: String,
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub balance: i64,
    #[serde(default)]
    pub holders: i64,
}

/// Total outstanding credit liability, grouped by org + currency. Only groups
/// with a positive net balance are returned (the money still owed). This is the
/// correct source for the admin liability chip (the old one summed a D1 column
/// that hold never wrote, so it always read zero).
pub async fn liability(db: &D1Database) -> Result<Vec<OrgLiability>, String> {
    let sql = "SELECT organization_id, currency, \
                      COALESCE(SUM(delta), 0) AS balance, \
                      COUNT(DISTINCT email)   AS holders \
               FROM credit_ledger \
               GROUP BY organization_id, currency \
               HAVING SUM(delta) > 0 \
               ORDER BY organization_id, currency";
    let stmt = db.prepare(sql);
    let rows = safe_all_rows(&stmt).await?;
    Ok(rows
        .into_iter()
        .filter_map(|v| serde_json::from_value::<OrgLiability>(v).ok())
        .collect())
}

/// Result of a credit-ledger reconciliation sweep (run daily by the cron). An
/// all-zero report is healthy; any nonzero count is a money-integrity alarm.
#[derive(Debug, Default, Clone)]
pub struct ReconcileReport {
    /// Held deposits (`held_as_credit=1, refunded=0`) with NO matching ledger
    /// `hold` entry — credit that was converted but never recorded. This is the
    /// exact 2026-08-14 loss signature.
    pub orphan_holds: i64,
    /// `(email, org, currency)` groups whose net balance is negative — an
    /// over-spend the atomic `try_spend` guard should make impossible; nonzero
    /// means an invariant broke.
    pub negative_balances: i64,
}

impl ReconcileReport {
    pub fn is_clean(&self) -> bool {
        self.orphan_holds == 0 && self.negative_balances == 0
    }
}

async fn count_query(db: &D1Database, sql: &str) -> Result<i64, String> {
    let stmt = db.prepare(sql);
    let rows = safe_all_rows(&stmt).await?;
    Ok(rows
        .into_iter()
        .next()
        .and_then(|v| v.get("n").and_then(serde_json::Value::as_i64))
        .unwrap_or(0))
}

/// Reconcile the ledger against deposit truth. Two cheap COUNT queries — safe to
/// run daily from the cron. Callers alert (Slack) on a non-clean report so a
/// silent credit loss surfaces within a day instead of at the next event.
pub async fn reconcile(db: &D1Database) -> Result<ReconcileReport, String> {
    let orphan_holds = count_query(
        db,
        "SELECT COUNT(*) AS n FROM thb_deposits d \
         WHERE d.held_as_credit = 1 AND d.refunded = 0 \
           AND NOT EXISTS (SELECT 1 FROM credit_ledger l \
                           WHERE l.reason = 'hold' \
                             AND l.deposit_id = d.event_id || ':' || d.attendee_id)",
    )
    .await?;
    let negative_balances = count_query(
        db,
        "SELECT COUNT(*) AS n FROM (\
             SELECT SUM(delta) AS bal FROM credit_ledger \
             GROUP BY email, organization_id, currency HAVING SUM(delta) < 0)",
    )
    .await?;
    Ok(ReconcileReport {
        orphan_holds,
        negative_balances,
    })
}

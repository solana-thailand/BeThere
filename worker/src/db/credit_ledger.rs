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
//!
//! **Balances are per person, rows are per email.** An email linked to others
//! (`db::person`, migration `0043`) shares one balance with them: every read and
//! the spend guard sum over `person_emails_of!`. Rows keep the email that moved
//! the money, so one email's own sum can go negative while the person's is
//! not — the person sum is the truth (plan 025).

use super::d1_safe::safe_all_rows;
use super::person::person_emails_of;
use std::collections::{HashMap, HashSet};
use worker::{D1Database, D1Type};

/// Audit reason label for a hold entry (deposit converted to rolling credit).
/// `apply` (spend) is written inline by [`try_spend`]; `backfill` is used by the
/// SQL backfill.
pub const REASON_HOLD: &str = "hold";
/// Audit reason label for a refund reversal (organizer paid the held credit back
/// out-of-band → remove it from the ledger).
pub const REASON_REFUND: &str = "refund";
/// Rolling credit RETURNED to the attendee after the event it was applied to.
/// +delta, keyed `return:{event_id}:{email}` by both writers, so the two can
/// never double-return:
///
/// - check-in writes it early (attendance, same day);
/// - [`release_ended_applies`] writes it for every apply whose event has
///   ended, **attended or not**.
///
/// Credit is the attendee's cash the organizer still holds, so it stays theirs
/// until it is actually paid back (`refund`). An apply only locks it for one
/// event at a time; a no-show does not forfeit it (owner rule, 2026-09-17 —
/// the earlier "no-show forfeits" Model B silently took ฿500 from two people).
pub const REASON_RETURN: &str = "return";

/// Set-based, idempotent release of every applied credit whose event has ended
/// and has no `return` yet. Shared by [`release_ended_applies`] and the atomic
/// apply batch (`db::credit_coverage`), so a spend always sees released credit.
///
/// `INSERT … SELECT` with an upsert needs the `WHERE` it has here (SQLite parse
/// rule). An event with no end time, or whose row is gone, stays locked: an
/// unknown end is not a past end.
pub(crate) const RELEASE_ENDED_APPLIES_SQL: &str = "INSERT INTO credit_ledger \
     (email, organization_id, currency, delta, reason, event_id, deposit_id, note) \
     SELECT a.email, a.organization_id, a.currency, -a.delta, 'return', a.event_id, \
            'return:' || a.event_id || ':' || a.email, 'event_ended' \
     FROM credit_ledger a JOIN events e ON e.id = a.event_id \
     WHERE a.reason = 'apply' AND a.delta < 0 \
       AND e.event_end_ms > 0 \
       AND e.event_end_ms <= CAST(strftime('%s', 'now') AS INTEGER) * 1000 \
       AND NOT EXISTS (SELECT 1 FROM credit_ledger r WHERE r.reason = 'return' \
                       AND r.event_id = a.event_id AND r.email = a.email) \
     ON CONFLICT (deposit_id, reason) WHERE deposit_id IS NOT NULL DO NOTHING";

/// Record the `return` for every applied credit whose event has ended.
///
/// Called at the top of every balance read in this module (and in the payout
/// queue), so no reader can see a balance that still counts an ended event's
/// lock — the same number the payout reversal removes. Errors propagate: a
/// reader that silently skipped this would under-report credit, and the
/// reversal would then pay out less than the attendee is owed.
pub async fn release_ended_applies(db: &D1Database) -> Result<usize, String> {
    let result = db
        .prepare(RELEASE_ENDED_APPLIES_SQL)
        .run()
        .await
        .map_err(|e| format!("D1 credit_ledger release_ended_applies: {e:?}"))?;
    Ok(result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0))
}

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
    release_ended_applies(db).await?;
    let email_lc = email.to_lowercase();
    let currency_lc = currency.to_lowercase();
    let sql = concat!(
        "INSERT INTO credit_ledger \
         (email, organization_id, currency, delta, reason, event_id, deposit_id) \
         SELECT ?1, ?2, ?3, -1 * ?4, 'apply', ?5, ?6 \
         WHERE (SELECT COALESCE(SUM(delta), 0) FROM credit_ledger \
                WHERE email IN ",
        person_emails_of!("?1"),
        " AND organization_id = ?2 AND currency = ?3) >= ?4 \
         ON CONFLICT (deposit_id, reason) WHERE deposit_id IS NOT NULL DO NOTHING"
    );
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

/// Current credit balance for `(email's person, organization_id, currency)`.
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
    release_ended_applies(db).await?;
    let email_lc = email.to_lowercase();
    let currency_lc = currency.to_lowercase();
    let sql = concat!(
        "SELECT COALESCE(SUM(delta), 0) AS bal FROM credit_ledger WHERE email IN ",
        person_emails_of!("?1"),
        " AND organization_id = ?2 AND currency = ?3"
    );
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

/// One `(organization_id, currency)` bucket of a single email's credit.
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct CreditBucket {
    #[serde(default)]
    pub organization_id: String,
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub balance: i64,
}

/// Every bucket in which one email's person still holds credit, across **all**
/// orgs and currencies. Only positive buckets are returned — the money still
/// owed to them. The payout reversal writes these amounts back under the
/// requesting email, which zeroes the person's balance.
///
/// The ledger is org-scoped so Org A's credit can never be spent at Org B, but
/// two paths are inherently org-blind: the attendee's own balance display and
/// the "return my held credit" exit, both of which hang off the *contact* and
/// carry no event (hence no org) context. Both used to hard-code
/// `organization_id = ""`, which is only correct while every event's org is
/// empty — the moment an organizer fills the Events tab's Org ID column, the
/// held credit becomes invisible to the balance chip and, far worse, invisible
/// to the payout reversal, so the organizer pays the cash out and the attendee
/// keeps spendable credit. Enumerate instead of guessing (plan 022 §6).
pub async fn positive_balances(db: &D1Database, email: &str) -> Result<Vec<CreditBucket>, String> {
    release_ended_applies(db).await?;
    let email_lc = email.to_lowercase();
    let sql = concat!(
        "SELECT organization_id, currency, COALESCE(SUM(delta), 0) AS balance \
         FROM credit_ledger WHERE email IN ",
        person_emails_of!("?1"),
        " GROUP BY organization_id, currency \
         HAVING SUM(delta) > 0 \
         ORDER BY organization_id, currency"
    );
    let stmt = db
        .prepare(sql)
        .bind_refs(&[D1Type::Text(&email_lc)])
        .map_err(|e| format!("D1 credit_ledger positive_balances bind: {e:?}"))?;
    let rows = safe_all_rows(&stmt).await?;
    Ok(rows
        .into_iter()
        .filter_map(|v| serde_json::from_value::<CreditBucket>(v).ok())
        .collect())
}

/// The SQL predicate selecting one person's **locked** credit rows: an `apply`
/// (negative) entry whose event has not written its matching `return` yet.
///
/// The row alias is fixed to `l` and the `return` probe's to `r`, so the caller
/// only supplies the email expression (a literal, same contract as
/// [`person_emails_of!`]). Three readers share it — the attendee's locked-credit
/// breakdown, the payout queue's `locked_thb`, and the queue's `locked_until` —
/// and they must agree: the payout guard refuses to clear a ฿0 request while
/// this predicate still matches, so a drifted copy would either strand a
/// request or let one be cleared against credit that is about to come back.
///
/// [`release_ended_applies`] must run before any of them, or an ended event's
/// credit still reads as locked.
macro_rules! unreturned_apply_of {
    ($email:literal) => {
        concat!(
            "l.reason = 'apply' AND l.delta < 0 AND l.email IN ",
            $crate::db::person::person_emails_of!($email),
            " AND NOT EXISTS (SELECT 1 FROM credit_ledger r \
               WHERE r.reason = 'return' AND r.event_id = l.event_id \
                 AND r.email = l.email)"
        )
    };
}
pub(crate) use unreturned_apply_of;

/// One event's worth of a person's credit that is currently locked — applied to
/// a deposit and not yet returned.
///
/// `event_name` / `event_end_ms` come from the D1 `events` mirror and are empty
/// / `0` when the row is missing; the credit is still locked in that case (an
/// unknown end is not a past end — same rule as [`RELEASE_ENDED_APPLIES_SQL`]).
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
pub struct LockedCredit {
    #[serde(default)]
    pub event_id: String,
    #[serde(default)]
    pub event_name: String,
    #[serde(default)]
    pub currency: String,
    /// Positive: the amount held against that event.
    #[serde(default)]
    pub amount: i64,
    #[serde(default)]
    pub event_end_ms: i64,
}

/// Every event currently holding a slice of this person's credit, newest end
/// first, across all orgs and currencies.
///
/// This is the other half of [`positive_balances`]: that function answers "how
/// much can be paid back right now", this one answers "how much is temporarily
/// committed and when does it come back". A holder whose whole balance is
/// applied to an upcoming event has an empty `positive_balances` and a
/// non-empty result here — the case issue #120 §3 is about, where clearing the
/// payout request reverses nothing and silently drops the request.
pub async fn locked_applies(db: &D1Database, email: &str) -> Result<Vec<LockedCredit>, String> {
    release_ended_applies(db).await?;
    let email_lc = email.to_lowercase();
    let sql = concat!(
        "SELECT l.event_id AS event_id, \
                COALESCE(e.name, '') AS event_name, \
                l.currency AS currency, \
                -COALESCE(SUM(l.delta), 0) AS amount, \
                COALESCE(e.event_end_ms, 0) AS event_end_ms \
         FROM credit_ledger l LEFT JOIN events e ON e.id = l.event_id \
         WHERE ",
        unreturned_apply_of!("?1"),
        " GROUP BY l.event_id, l.currency, e.name, e.event_end_ms \
         ORDER BY COALESCE(e.event_end_ms, 0) DESC, l.event_id"
    );
    let stmt = db
        .prepare(sql)
        .bind_refs(&[D1Type::Text(&email_lc)])
        .map_err(|e| format!("D1 credit_ledger locked_applies bind: {e:?}"))?;
    let rows = safe_all_rows(&stmt).await?;
    Ok(rows
        .into_iter()
        .filter_map(|v| serde_json::from_value::<LockedCredit>(v).ok())
        .collect())
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
    release_ended_applies(db).await?;
    // Holders are people, not emails: a linked person counts once.
    let sql = "SELECT l.organization_id AS organization_id, l.currency AS currency, \
                      COALESCE(SUM(l.delta), 0) AS balance, \
                      COUNT(DISTINCT COALESCE(p.person_id, l.email)) AS holders \
               FROM credit_ledger l LEFT JOIN person_emails p ON p.email = l.email \
               GROUP BY l.organization_id, l.currency \
               HAVING SUM(l.delta) > 0 \
               ORDER BY l.organization_id, l.currency";
    let stmt = db.prepare(sql);
    let rows = safe_all_rows(&stmt).await?;
    Ok(rows
        .into_iter()
        .filter_map(|v| serde_json::from_value::<OrgLiability>(v).ok())
        .collect())
}

/// THB credit balance keyed by (lowercased) email for one org — holders only.
/// One query to annotate the admin attendee list's per-row credit (powers the
/// "Apply Credit" action + a credit badge) without an N+1 per attendee.
///
/// Summed per person, then expanded to **every** email of that person — including
/// emails with no ledger rows, which is exactly the attendee who registered with
/// a second email (issue #122). Person ids are UUIDs and never collide with an
/// email key.
pub async fn thb_balances_by_email(
    db: &D1Database,
    organization_id: &str,
) -> Result<HashMap<String, i64>, String> {
    release_ended_applies(db).await?;
    let sql = "SELECT COALESCE(m.email, b.holder) AS email, b.bal AS bal FROM ( \
                 SELECT COALESCE(p.person_id, l.email) AS holder, SUM(l.delta) AS bal \
                 FROM credit_ledger l LEFT JOIN person_emails p ON p.email = l.email \
                 WHERE l.organization_id = ?1 AND l.currency = 'thb' \
                 GROUP BY holder HAVING SUM(l.delta) > 0) b \
               LEFT JOIN person_emails m ON m.person_id = b.holder";
    let stmt = db
        .prepare(sql)
        .bind_refs(&[D1Type::Text(organization_id)])
        .map_err(|e| format!("D1 credit_ledger thb_balances bind: {e:?}"))?;
    let rows = safe_all_rows(&stmt).await?;
    let mut map = HashMap::new();
    for v in rows {
        if let (Some(email), Some(bal)) = (
            v.get("email").and_then(|x| x.as_str()),
            v.get("bal").and_then(serde_json::Value::as_i64),
        ) {
            map.insert(email.to_lowercase(), bal);
        }
    }
    Ok(map)
}

/// Lowercased emails that APPLIED (spent) rolling credit at a given event — i.e.
/// attendees who "got in using credit". Powers the Credit ✓ badge / credit-used
/// list on the in-person roster. One query; membership lookup in-memory.
pub async fn emails_applied_credit(
    db: &D1Database,
    event_id: &str,
) -> Result<HashSet<String>, String> {
    let sql = "SELECT DISTINCT email FROM credit_ledger \
               WHERE event_id = ?1 AND reason = 'apply'";
    let stmt = db
        .prepare(sql)
        .bind_refs(&[D1Type::Text(event_id)])
        .map_err(|e| format!("D1 credit_ledger emails_applied bind: {e:?}"))?;
    let rows = safe_all_rows(&stmt).await?;
    Ok(rows
        .into_iter()
        .filter_map(|v| {
            v.get("email")
                .and_then(|x| x.as_str())
                .map(|s| s.to_lowercase())
        })
        .collect())
}

/// Delete the credit-return entry for (event, email) — used on undo-check-in
/// so a subsequent re-check-in re-adds it. No-op when none exists, and a no-op
/// once the event has ended: from then on the credit is returned regardless of
/// attendance, so undoing a check-in must not take it back.
pub async fn remove_return(db: &D1Database, event_id: &str, email: &str) -> Result<(), String> {
    let email_lc = email.to_lowercase();
    let sql = "DELETE FROM credit_ledger WHERE reason = 'return' \
               AND event_id = ?1 AND email = ?2 \
               AND NOT EXISTS (SELECT 1 FROM events e WHERE e.id = ?1 \
                   AND e.event_end_ms > 0 \
                   AND e.event_end_ms <= CAST(strftime('%s', 'now') AS INTEGER) * 1000)";
    db.prepare(sql)
        .bind_refs(&[D1Type::Text(event_id), D1Type::Text(&email_lc)])
        .map_err(|e| format!("D1 credit_ledger remove_return bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 credit_ledger remove_return run: {e:?}"))?;
    Ok(())
}

/// Result of a credit-ledger reconciliation sweep (run daily by the cron). An
/// all-zero report is healthy; any nonzero count is a money-integrity alarm.
#[derive(Debug, Default, Clone)]
pub struct ReconcileReport {
    /// Legacy THB statuses safely reclassified to `credit_thb` during this run.
    pub applies_repaired: usize,
    /// Held deposits (`held_as_credit=1, refunded=0`) with NO matching ledger
    /// `hold` entry — credit that was converted but never recorded. This is the
    /// exact 2026-08-14 loss signature.
    pub orphan_holds: i64,
    /// `(person, org, currency)` groups whose net balance is negative — an
    /// over-spend the atomic spend guard should make impossible; nonzero means
    /// an invariant broke. Per person, not per email: a linked email's own sum
    /// is legitimately negative after it spends a sibling's credit.
    pub negative_balances: i64,
    /// Deposits settled BOTH ways (`held_as_credit=1 AND refunded=1`) — the
    /// attendee got the cash back *and* keeps spendable credit. The two settle
    /// paths are mutually exclusive by CAS, so nonzero means the CAS was
    /// bypassed (or a row was hand-edited) and money left twice.
    pub double_settled: i64,
    /// Ledger `hold` entries whose deposit is no longer held as credit — credit
    /// standing against nothing. `orphan_holds` catches the loss direction
    /// (deposit held, credit missing); this catches the creation direction.
    pub phantom_holds: i64,
    /// Applied credit whose verified attendee-facing deposit projection is
    /// missing or inconsistent.
    pub incomplete_applies: i64,
}

impl ReconcileReport {
    pub fn is_clean(&self) -> bool {
        self.orphan_holds == 0
            && self.negative_balances == 0
            && self.double_settled == 0
            && self.phantom_holds == 0
            && self.incomplete_applies == 0
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

/// Reconcile the ledger against deposit truth. Four cheap COUNT queries — safe
/// to run daily from the cron. Callers alert (Slack) on a non-clean report so a
/// silent credit loss surfaces within a day instead of at the next event.
///
/// The checks cover both directions of the money: credit that should exist and
/// does not (`orphan_holds`), and credit that exists and should not
/// (`phantom_holds`, `double_settled`, `negative_balances`). Only the first was
/// checked originally, which left every over-payment path silent.
pub async fn reconcile(db: &D1Database) -> Result<ReconcileReport, String> {
    // Materialise releases first so `negative_balances` and the daily run (the
    // backstop for balances nobody has read yet) both see ended events returned.
    release_ended_applies(db).await?;
    // Before the atomic coverage workflow, credit-backed THB registrations were
    // written with method='thb'. Repair only an exact ledger + marker + amount
    // match, and refuse any attendee/event that also has a cash deposit row.
    let repaired = db
        .prepare(
            "UPDATE deposit_statuses AS s SET method='credit_thb',currency='THB',verified=1,refundable=0,rejected=0 \
             WHERE s.method='thb' AND EXISTS( \
               SELECT 1 FROM attendees a JOIN credit_ledger l \
                 ON l.event_id=a.event_id AND l.email=LOWER(a.email) AND l.reason='apply' \
               JOIN thb_deposits d ON d.event_id=a.event_id AND d.attendee_id=a.id \
                 AND d.slip_url='ROLLING_CREDIT_AUTO_APPLIED' AND d.verified=1 \
               WHERE a.id=s.attendee_id AND a.event_id=s.event_id AND l.currency='thb' \
                 AND l.delta=-s.amount AND d.amount_thb=s.amount) \
             AND NOT EXISTS(SELECT 1 FROM thb_deposits cash WHERE cash.event_id=s.event_id \
               AND cash.attendee_id=s.attendee_id \
               AND COALESCE(cash.slip_url,'')<>'ROLLING_CREDIT_AUTO_APPLIED')",
        )
        .run()
        .await
        .map_err(|e| format!("D1 credit apply projection repair: {e:?}"))?;
    let applies_repaired = repaired
        .meta()
        .ok()
        .flatten()
        .and_then(|meta| meta.changes)
        .unwrap_or(0);
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
             SELECT SUM(l.delta) AS bal \
             FROM credit_ledger l LEFT JOIN person_emails p ON p.email = l.email \
             GROUP BY COALESCE(p.person_id, l.email), l.organization_id, l.currency \
             HAVING SUM(l.delta) < 0)",
    )
    .await?;
    let double_settled = count_query(
        db,
        "SELECT COUNT(*) AS n FROM thb_deposits \
         WHERE held_as_credit = 1 AND refunded = 1",
    )
    .await?;
    // A `hold` entry is keyed `deposit_id = event_id || ':' || attendee_id` by
    // both hold writers, so the join back to deposit truth is exact.
    let phantom_holds = count_query(
        db,
        "SELECT COUNT(*) AS n FROM credit_ledger l \
         WHERE l.reason = 'hold' AND l.deposit_id IS NOT NULL \
           AND NOT EXISTS (SELECT 1 FROM thb_deposits d \
                           WHERE d.held_as_credit = 1 \
                             AND d.event_id || ':' || d.attendee_id = l.deposit_id)",
    )
    .await?;
    let incomplete_applies = count_query(
        db,
        "SELECT COUNT(*) AS n FROM credit_ledger l \
         JOIN attendees a ON a.event_id=l.event_id AND LOWER(a.email)=l.email \
         WHERE l.reason='apply' AND ( \
           NOT EXISTS(SELECT 1 FROM deposit_statuses s \
             WHERE s.event_id=l.event_id AND s.attendee_id=a.id \
               AND s.method='credit_'||l.currency AND s.amount=-l.delta \
               AND s.verified=1 AND s.refundable=0) \
           OR (l.currency='thb' AND NOT EXISTS(SELECT 1 FROM thb_deposits d \
             WHERE d.event_id=l.event_id AND d.attendee_id=a.id \
               AND d.slip_url='ROLLING_CREDIT_AUTO_APPLIED' \
               AND d.amount_thb=-l.delta AND d.verified=1)))",
    )
    .await?;
    Ok(ReconcileReport {
        applies_repaired,
        orphan_holds,
        negative_balances,
        double_settled,
        phantom_holds,
        incomplete_applies,
    })
}

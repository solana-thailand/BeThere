//! Linked emails: several emails, one person (plan 025, issue #122).
//!
//! Email is the identity key everywhere. A `person_emails` row links an email
//! to a person; an email with no row is implicitly its own person. Only credit
//! reads and the credit spend guard resolve over the linked set (owner choice
//! 2026-09-18: shared credit only) — sessions and every other email-keyed table
//! are unchanged.
//!
//! Every person-aware query goes through [`person_emails_of!`], so the set is
//! defined in exactly one place. See migration `0043_person_emails.sql`.

use super::d1_safe::safe_all_rows;
use worker::{D1Database, D1Type};

/// SQL subquery: every email of the person that `$email` belongs to, including
/// `$email` itself (so an unlinked email resolves to just itself).
///
/// `$email` is a SQL expression literal — a bound parameter (`"?1"`) or a
/// correlated column (`"LOWER(c.email)"`) — never user input. It must already
/// be lowercased, as every `person_emails.email` and `credit_ledger.email` is.
/// Use as `WHERE l.email IN person_emails_of!("?1")`.
macro_rules! person_emails_of {
    ($email:literal) => {
        concat!(
            "(SELECT ",
            $email,
            " AS email UNION SELECT m.email FROM person_emails o \
             JOIN person_emails m ON m.person_id = o.person_id WHERE o.email = ",
            $email,
            ")"
        )
    };
}
pub(crate) use person_emails_of;

/// Link statements, run as one D1 batch (a transaction). All bind
/// `?1 = signed-in email, ?2 = fresh person id, ?3 = email being added`; a
/// statement that skips `?2` still takes three bindings, because SQLite sizes
/// the parameter list by the highest index used.
/// Linking is symmetric — the session proves the first email and the Google
/// sign-in proves the second — so whichever side is already linked is joined:
///
/// | first linked | second linked | result |
/// |---|---|---|
/// | no  | no  | new person, first email primary |
/// | yes | no  | second joins first's person |
/// | no  | yes | first joins second's person |
/// | yes, same person | yes | nothing (already linked) |
/// | yes, other person | yes | nothing (conflict — merging two people is not v1) |
///
/// `INSERT … SELECT` with an upsert needs a `WHERE` (SQLite parse rule), which
/// each statement has.
pub(crate) const LINK_FIRST_JOINS_SECOND_SQL: &str = "INSERT INTO person_emails \
     (email, person_id, is_primary, proof) \
     SELECT ?1, (SELECT person_id FROM person_emails WHERE email = ?3), 0, 'google' \
     WHERE EXISTS (SELECT 1 FROM person_emails WHERE email = ?3) \
     ON CONFLICT (email) DO NOTHING";
pub(crate) const LINK_NEW_PERSON_SQL: &str = "INSERT INTO person_emails \
     (email, person_id, is_primary, proof) \
     SELECT ?1, ?2, 1, 'google' \
     WHERE NOT EXISTS (SELECT 1 FROM person_emails WHERE email = ?1) \
       AND NOT EXISTS (SELECT 1 FROM person_emails WHERE email = ?3) \
     ON CONFLICT (email) DO NOTHING";
pub(crate) const LINK_SECOND_JOINS_FIRST_SQL: &str = "INSERT INTO person_emails \
     (email, person_id, is_primary, proof) \
     SELECT ?3, (SELECT person_id FROM person_emails WHERE email = ?1), 0, 'google' \
     WHERE EXISTS (SELECT 1 FROM person_emails WHERE email = ?1) \
     ON CONFLICT (email) DO NOTHING";
pub(crate) const LINK_STATE_SQL: &str = "SELECT \
     COALESCE((SELECT person_id FROM person_emails WHERE email = ?1) \
            = (SELECT person_id FROM person_emails WHERE email = ?3), 0) AS same_person";

/// Result of [`link_google`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOutcome {
    /// The two emails are now one person (this call wrote the link).
    Linked,
    /// They already were one person; nothing changed.
    AlreadyLinked,
    /// The same email on both sides; nothing to link.
    SameEmail,
    /// Each email already belongs to a different person. Merging two linked
    /// people is not supported in v1.
    Conflict,
}

#[derive(serde::Deserialize)]
struct LinkState {
    same_person: i64,
}

/// Link `added_email` into the person of `session_email`.
///
/// The caller must have proven both: `session_email` from a Google-verified
/// session belonging to this browser, `added_email` from a Google sign-in
/// completed in that same browser. Idempotent.
pub async fn link_google(
    db: &D1Database,
    session_email: &str,
    added_email: &str,
) -> Result<LinkOutcome, String> {
    let first = session_email.trim().to_lowercase();
    let second = added_email.trim().to_lowercase();
    if first == second {
        return Ok(LinkOutcome::SameEmail);
    }
    let person_id = uuid::Uuid::now_v7().to_string();
    let binds = [
        D1Type::Text(&first),
        D1Type::Text(&person_id),
        D1Type::Text(&second),
    ];
    let mut statements = Vec::with_capacity(4);
    for sql in [
        LINK_FIRST_JOINS_SECOND_SQL,
        LINK_NEW_PERSON_SQL,
        LINK_SECOND_JOINS_FIRST_SQL,
        LINK_STATE_SQL,
    ] {
        statements.push(
            db.prepare(sql)
                .bind_refs(&binds)
                .map_err(|e| format!("D1 person link bind: {e:?}"))?,
        );
    }
    let results = db
        .batch(statements)
        .await
        .map_err(|e| format!("D1 person link batch: {e:?}"))?;
    let written: usize = results
        .iter()
        .take(3)
        .filter_map(|r| r.meta().ok().flatten().and_then(|m| m.changes))
        .sum();
    let state = results
        .last()
        .ok_or_else(|| "D1 person link batch returned no state".to_string())?
        .results::<LinkState>()
        .map_err(|e| format!("D1 person link state decode: {e:?}"))?
        .into_iter()
        .next()
        .ok_or_else(|| "D1 person link state returned no row".to_string())?;

    Ok(match (state.same_person != 0, written > 0) {
        (true, true) => LinkOutcome::Linked,
        (true, false) => LinkOutcome::AlreadyLinked,
        (false, _) => LinkOutcome::Conflict,
    })
}

/// SQL behind [`claimed_elsewhere`]. Kept as a constant so the SQLite
/// behaviour test runs the real statement.
pub(crate) const CLAIMED_ELSEWHERE_SQL: &str = concat!(
    "SELECT a.claimed_at AS claimed_at FROM attendees a \
     WHERE a.event_id = ?1 AND a.id <> ?3 \
       AND a.claimed_at IS NOT NULL AND a.claimed_at <> '' \
       AND LOWER(a.email) IN ",
    person_emails_of!("?2"),
    " LIMIT 1"
);

/// When another attendee row of the SAME person already claimed this event's
/// badge, its `claimed_at`. One badge per person per event, so linking two
/// emails cannot double a claim (plan 025 §5.2).
///
/// Reads the D1 attendee mirror, which the claim writer updates. A row missing
/// from the mirror makes this return `None`, so it is a second line of defence
/// on top of the per-row `claimed_at` check, not a replacement for it. It can
/// only ever fire for emails someone deliberately linked.
pub async fn claimed_elsewhere(
    db: &D1Database,
    event_id: &str,
    email: &str,
    attendee_id: &str,
) -> Result<Option<String>, String> {
    let email_lc = email.trim().to_lowercase();
    let stmt = db
        .prepare(CLAIMED_ELSEWHERE_SQL)
        .bind_refs(&[
            D1Type::Text(event_id),
            D1Type::Text(&email_lc),
            D1Type::Text(attendee_id),
        ])
        .map_err(|e| format!("D1 person claimed_elsewhere bind: {e:?}"))?;
    let rows = safe_all_rows(&stmt).await?;
    Ok(rows.into_iter().next().and_then(|v| {
        v.get("claimed_at")
            .and_then(|x| x.as_str())
            .map(str::to_string)
    }))
}

/// Every email of `email`'s person, lowercased and sorted; just `[email]` when
/// it is not linked.
pub async fn emails_of(db: &D1Database, email: &str) -> Result<Vec<String>, String> {
    let email_lc = email.trim().to_lowercase();
    let sql = concat!(
        "SELECT e.email AS email FROM ",
        person_emails_of!("?1"),
        " AS e ORDER BY e.email"
    );
    let stmt = db
        .prepare(sql)
        .bind_refs(&[D1Type::Text(&email_lc)])
        .map_err(|e| format!("D1 person emails_of bind: {e:?}"))?;
    let rows = safe_all_rows(&stmt).await?;
    let emails: Vec<String> = rows
        .into_iter()
        .filter_map(|v| v.get("email").and_then(|x| x.as_str()).map(str::to_string))
        .collect();
    Ok(match emails.is_empty() {
        true => vec![email_lc],
        false => emails,
    })
}

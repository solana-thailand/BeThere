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

/// Link statements, run as one D1 batch (a transaction). The three inserts bind
/// `?1 = first email, ?2 = fresh person id, ?3 = second email, ?4 = proof`
/// ([`LinkProof`]); a statement that skips an index still takes every binding
/// up to the highest one it uses, because SQLite sizes the parameter list that
/// way. [`LINK_STATE_SQL`] uses only `?1`/`?3`, so it takes the first three.
/// Linking is symmetric — for a Google link the session proves the first email
/// and the Google sign-in the second; for an admin link a super-admin vouches
/// for both — so whichever side is already linked is joined:
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
     SELECT ?1, (SELECT person_id FROM person_emails WHERE email = ?3), 0, ?4 \
     WHERE EXISTS (SELECT 1 FROM person_emails WHERE email = ?3) \
     ON CONFLICT (email) DO NOTHING";
pub(crate) const LINK_NEW_PERSON_SQL: &str = "INSERT INTO person_emails \
     (email, person_id, is_primary, proof) \
     SELECT ?1, ?2, 1, ?4 \
     WHERE NOT EXISTS (SELECT 1 FROM person_emails WHERE email = ?1) \
       AND NOT EXISTS (SELECT 1 FROM person_emails WHERE email = ?3) \
     ON CONFLICT (email) DO NOTHING";
pub(crate) const LINK_SECOND_JOINS_FIRST_SQL: &str = "INSERT INTO person_emails \
     (email, person_id, is_primary, proof) \
     SELECT ?3, (SELECT person_id FROM person_emails WHERE email = ?1), 0, ?4 \
     WHERE EXISTS (SELECT 1 FROM person_emails WHERE email = ?1) \
     ON CONFLICT (email) DO NOTHING";
pub(crate) const LINK_STATE_SQL: &str = "SELECT \
     COALESCE((SELECT person_id FROM person_emails WHERE email = ?1) \
            = (SELECT person_id FROM person_emails WHERE email = ?3), 0) AS same_person";

/// How ownership of a linked email was established (`person_emails.proof`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkProof {
    /// A Google sign-in for the added email inside a session already signed
    /// in as the person (`handlers::email_link`).
    Google,
    /// A super-admin vouched for both emails, with a reason in the audit log
    /// (plan 025 §6.1). For emails that cannot sign in with Google.
    Admin,
}

impl LinkProof {
    /// The value stored in `person_emails.proof` (the table's CHECK list).
    pub fn as_str(self) -> &'static str {
        match self {
            LinkProof::Google => "google",
            LinkProof::Admin => "admin",
        }
    }
}

/// Result of [`link`].
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

/// Link `second_email` into the person of `first_email`.
///
/// The caller must have established ownership of both, as `proof` records:
/// for [`LinkProof::Google`], `first_email` from a Google-verified session
/// belonging to this browser and `second_email` from a Google sign-in completed
/// in that same browser; for [`LinkProof::Admin`], a super-admin's decision,
/// audited by the caller. Idempotent.
pub async fn link(
    db: &D1Database,
    first_email: &str,
    second_email: &str,
    proof: LinkProof,
) -> Result<LinkOutcome, String> {
    let first = first_email.trim().to_lowercase();
    let second = second_email.trim().to_lowercase();
    if first == second {
        return Ok(LinkOutcome::SameEmail);
    }
    let person_id = uuid::Uuid::now_v7().to_string();
    let binds = [
        D1Type::Text(&first),
        D1Type::Text(&person_id),
        D1Type::Text(&second),
        D1Type::Text(proof.as_str()),
    ];
    let mut statements = Vec::with_capacity(4);
    for sql in [
        LINK_FIRST_JOINS_SECOND_SQL,
        LINK_NEW_PERSON_SQL,
        LINK_SECOND_JOINS_FIRST_SQL,
    ] {
        statements.push(
            db.prepare(sql)
                .bind_refs(&binds)
                .map_err(|e| format!("D1 person link bind: {e:?}"))?,
        );
    }
    statements.push(
        db.prepare(LINK_STATE_SQL)
            .bind_refs(&binds[..3])
            .map_err(|e| format!("D1 person link state bind: {e:?}"))?,
    );
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

/// The person an email belongs to, and whether it is that person's primary.
pub(crate) const UNLINK_MEMBER_SQL: &str = "SELECT person_id FROM person_emails WHERE email = ?1";

/// Unlink statements, run as one D1 batch. [`UNLINK_SQL`] binds `?1 = email`;
/// the two tidy-ups bind `?1 = email, ?2 = person id` (they use only `?2`).
///
/// The delete is refused, inside the same statement, when splitting the person
/// would leave either side owing credit: per organization and currency, the
/// email's own ledger sum or the sum of its siblings must not be negative. A
/// negative side means it spent (or has locked) credit the other side holds,
/// and unlinking would turn that into a debt nobody can see. Removing the row
/// changes no ledger row, so an allowed unlink moves no money.
pub(crate) const UNLINK_SQL: &str = concat!(
    "DELETE FROM person_emails WHERE email = ?1 AND NOT EXISTS (\
     SELECT 1 FROM credit_ledger l WHERE l.email IN ",
    person_emails_of!("?1"),
    " GROUP BY l.organization_id, l.currency \
     HAVING SUM(CASE WHEN l.email = ?1 THEN l.delta ELSE 0 END) < 0 \
         OR SUM(CASE WHEN l.email <> ?1 THEN l.delta ELSE 0 END) < 0)"
);
/// A person left with one email is no person at all: an unlinked email is
/// implicitly its own person, so the lone row is removed.
pub(crate) const UNLINK_DROP_SINGLETON_SQL: &str = "DELETE FROM person_emails \
     WHERE person_id = ?2 AND (SELECT COUNT(*) FROM person_emails WHERE person_id = ?2) = 1";
/// Keep exactly one primary: if the removed email was it, the earliest-linked
/// remaining email takes over.
pub(crate) const UNLINK_PROMOTE_SQL: &str = "UPDATE person_emails SET is_primary = 1 \
     WHERE email = (SELECT email FROM person_emails WHERE person_id = ?2 \
                    ORDER BY linked_at, email LIMIT 1) \
       AND NOT EXISTS (SELECT 1 FROM person_emails WHERE person_id = ?2 AND is_primary = 1)";

/// Result of [`unlink`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnlinkOutcome {
    /// The email is its own person again.
    Unlinked,
    /// The email was not linked to anyone; nothing changed.
    NotLinked,
    /// Refused: one side has spent or locked credit the other side holds.
    CreditWouldGoNegative,
}

/// Take `email` out of its person. Idempotent; refuses when splitting would
/// leave either side with a negative credit balance ([`UNLINK_SQL`]).
pub async fn unlink(db: &D1Database, email: &str) -> Result<UnlinkOutcome, String> {
    let email_lc = email.trim().to_lowercase();
    // `safe_all_rows`, not `.first::<T>()`, which panics on a missing row.
    let stmt = db
        .prepare(UNLINK_MEMBER_SQL)
        .bind_refs(&[D1Type::Text(&email_lc)])
        .map_err(|e| format!("D1 person unlink member bind: {e:?}"))?;
    let person_id = safe_all_rows(&stmt)
        .await?
        .into_iter()
        .next()
        .and_then(|v| {
            v.get("person_id")
                .and_then(|x| x.as_str())
                .map(str::to_string)
        });
    let Some(person_id) = person_id else {
        return Ok(UnlinkOutcome::NotLinked);
    };
    let binds = [D1Type::Text(&email_lc), D1Type::Text(&person_id)];
    let statements = vec![
        db.prepare(UNLINK_SQL)
            .bind_refs(&binds[..1])
            .map_err(|e| format!("D1 person unlink bind: {e:?}"))?,
        db.prepare(UNLINK_DROP_SINGLETON_SQL)
            .bind_refs(&binds)
            .map_err(|e| format!("D1 person unlink singleton bind: {e:?}"))?,
        db.prepare(UNLINK_PROMOTE_SQL)
            .bind_refs(&binds)
            .map_err(|e| format!("D1 person unlink promote bind: {e:?}"))?,
    ];
    let results = db
        .batch(statements)
        .await
        .map_err(|e| format!("D1 person unlink batch: {e:?}"))?;
    let deleted = results
        .first()
        .and_then(|r| r.meta().ok().flatten())
        .and_then(|m| m.changes)
        .unwrap_or(0);
    Ok(match deleted {
        0 => UnlinkOutcome::CreditWouldGoNegative,
        _ => UnlinkOutcome::Unlinked,
    })
}

/// SQL behind [`claimed_elsewhere`]. Kept as a constant so the SQLite
/// behaviour test runs the real statement.
pub(crate) const CLAIMED_ELSEWHERE_SQL: &str = concat!(
    "SELECT a.claimed_at AS claimed_at FROM attendees a \
     WHERE a.event_id = ?1 AND a.id <> ?3 AND COALESCE(a.claim_token, '') <> ?4 \
       AND a.claimed_at IS NOT NULL AND a.claimed_at <> '' \
       AND LOWER(a.email) IN ",
    person_emails_of!("?2"),
    " LIMIT 1"
);

/// When another attendee row of the SAME person already claimed this event's
/// badge, its `claimed_at`. One badge per person per event, so a second
/// registration cannot double a claim (plan 025 §5.2).
///
/// The claiming row is excluded by `attendee_id` **or** `claim_token`, because
/// the walk-in path has only the token (its record carries no row id). Pass an
/// empty string for whichever one you lack.
///
/// Reads the D1 attendee mirror, which the claim writer updates. A row missing
/// from the mirror makes this return `None`, so it is a second line of defence
/// on top of the per-row `claimed_at` check, not a replacement for it.
pub async fn claimed_elsewhere(
    db: &D1Database,
    event_id: &str,
    email: &str,
    attendee_id: &str,
    claim_token: &str,
) -> Result<Option<String>, String> {
    let email_lc = email.trim().to_lowercase();
    let stmt = db
        .prepare(CLAIMED_ELSEWHERE_SQL)
        .bind_refs(&[
            D1Type::Text(event_id),
            D1Type::Text(&email_lc),
            D1Type::Text(attendee_id),
            D1Type::Text(claim_token),
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

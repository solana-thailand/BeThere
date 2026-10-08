//! The attendee's last credit payout, read back from the ledger (`.issues/192`).
//!
//! After an organizer pays held credit out, the balance is zero but the ticket
//! page still mounts the "return my credit" card (it keys on the deposit's
//! `held_as_credit`), so the attendee saw a request button for money they had
//! already been sent. This read gives the card the payout to show instead.
//!
//! The ledger is the record. Every row of one payout is written by the single
//! `TRY_REFUND_SQL` statement (the only writer of `reason = 'refund'`), and a
//! second payout needs credit first, i.e. a positive row in between. So one
//! payout is the person's `refund` rows after their last other row. Not
//! `created_at`: it has one-second resolution. No migration, and no account:
//! the payout deletes it.

use event_checkin_domain::models::credit_payout::CreditPayoutReceipt;
use worker::{D1Database, D1Type};

use super::credit_ledger::unreturned_apply_of;
use super::d1_safe::safe_all_rows;
use super::person::person_emails_of;

/// The person's (`?1`, lowercased) latest payout, **only while it settled
/// everything**: no credit came in after it (a later hold or an event's
/// `return` is a positive row with a higher id) and none is still locked to an
/// event. Otherwise no row, and the card offers the request as before.
///
/// `GROUP BY` the payout's id so "no payout" is no row, not a row of NULLs.
pub(crate) const SETTLED_PAYOUT_SQL: &str = concat!(
    "SELECT COALESCE(SUM(CASE WHEN p.currency = 'thb' THEN -p.delta ELSE 0 END), 0) AS thb, \
            COALESCE(SUM(CASE WHEN p.currency = 'usdc' THEN -p.delta ELSE 0 END), 0) AS usdc, \
            strftime('%Y-%m-%dT%H:%M:%SZ', last.created_at) AS paid_at \
     FROM (SELECT id, created_at FROM credit_ledger \
           WHERE reason = 'refund' AND email IN ",
    person_emails_of!("?1"),
    " ORDER BY id DESC LIMIT 1) last \
     JOIN credit_ledger p ON p.reason = 'refund' AND p.id <= last.id AND p.email IN ",
    person_emails_of!("?1"),
    " AND p.id > COALESCE((SELECT MAX(o.id) FROM credit_ledger o \
        WHERE o.reason <> 'refund' AND o.id < last.id AND o.email IN ",
    person_emails_of!("?1"),
    "), 0) WHERE NOT EXISTS (SELECT 1 FROM credit_ledger n \
        WHERE n.delta > 0 AND n.id > last.id AND n.email IN ",
    person_emails_of!("?1"),
    ") AND NOT EXISTS (SELECT 1 FROM credit_ledger l WHERE ",
    unreturned_apply_of!("?1"),
    ") GROUP BY last.id"
);

/// The settled payout for the person `email` belongs to, if any.
pub async fn settled_payout(
    db: &D1Database,
    email: &str,
) -> Result<Option<CreditPayoutReceipt>, String> {
    let email_lc = email.to_lowercase();
    let stmt = db
        .prepare(SETTLED_PAYOUT_SQL)
        .bind_refs(&[D1Type::Text(&email_lc)])
        .map_err(|e| format!("D1 credit_ledger settled_payout bind: {e:?}"))?;
    let row = safe_all_rows(&stmt).await?.into_iter().next();
    match row {
        None => Ok(None),
        Some(v) => serde_json::from_value::<CreditPayoutReceipt>(v)
            .map(Some)
            .map_err(|e| format!("D1 credit_ledger settled_payout row: {e}")),
    }
}

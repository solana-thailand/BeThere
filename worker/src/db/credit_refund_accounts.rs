//! Where an attendee wants held credit paid back to (`.issues/190`, migration
//! `0059_credit_refund_payout.sql`).
//!
//! One row per contact email, written only together with the request flag
//! (`contacts::set_credit_refund_requested`, one D1 batch) and deleted
//! together with its clear (`contacts::clear_credit_refund_requested`), by the
//! nightly purge ([`purge`]) and by PDPA erasure (`contacts::clear_contact_pii`).
//!
//! The account number is personal data: nothing here or in any caller may log
//! it. The admin payout queue is its only reader.

use event_checkin_domain::models::credit_payout::RefundAccount;
use worker::D1Database;
use worker::d1::{D1PreparedStatement, D1Type};

/// Days an account row may live. A request still open this long is ~12 weeks
/// past the 7-day promise (D3); the attendee is asked again rather than the
/// number kept indefinitely. Matches the deposit purge's 90 days.
pub const RETENTION_DAYS: i64 = 90;

/// Upsert the account for an email **whose request flag is set** — the
/// `WHERE EXISTS` makes a row without an open request impossible to write.
/// Runs in the same batch as the flag `UPDATE`, after it.
///
/// `?1` lowercased email, `?2` method, `?3` promptpay id, `?4` bank name,
/// `?5` bank account, `?6` account name (`NULL` for the other method's
/// fields — the table's CHECK requires it).
pub(crate) const SAVE_SQL: &str = "INSERT INTO credit_refund_accounts \
     (email, method, promptpay_id, bank_name, bank_account, account_name, updated_at) \
     SELECT ?1, ?2, ?3, ?4, ?5, ?6, datetime('now') \
     WHERE EXISTS (SELECT 1 FROM contacts WHERE email = ?1 AND credit_refund_requested = 1) \
     ON CONFLICT (email) DO UPDATE SET method = excluded.method, \
       promptpay_id = excluded.promptpay_id, bank_name = excluded.bank_name, \
       bank_account = excluded.bank_account, account_name = excluded.account_name, \
       updated_at = excluded.updated_at";

/// Delete the account of every email of one person (`?1`). The payout is
/// person-wide, and so is the request clear that runs it.
pub(crate) const DELETE_FOR_PERSON_SQL: &str = concat!(
    "DELETE FROM credit_refund_accounts WHERE email IN ",
    crate::db::person::person_emails_of!("?1")
);

/// The nightly purge: rows past [`RETENTION_DAYS`] (`?1`, as a negative day
/// modifier like `-90 days`), and rows whose request is no longer open — a
/// clear whose best-effort delete failed leaves one of those behind.
pub(crate) const PURGE_SQL: &str = "DELETE FROM credit_refund_accounts \
     WHERE updated_at < datetime('now', ?1) \
        OR NOT EXISTS (SELECT 1 FROM contacts c \
                       WHERE c.email = credit_refund_accounts.email \
                         AND c.credit_refund_requested = 1)";

/// The bound [`SAVE_SQL`] statement for `account` (already normalized).
pub(crate) fn save_statement(
    db: &D1Database,
    email_lower: &str,
    account: &RefundAccount,
) -> Result<D1PreparedStatement, String> {
    let method = account.method().as_str();
    let binds = match account {
        RefundAccount::PromptPay { promptpay_id } => [
            D1Type::Text(email_lower),
            D1Type::Text(method),
            D1Type::Text(promptpay_id),
            D1Type::Null,
            D1Type::Null,
            D1Type::Null,
        ],
        RefundAccount::Bank {
            bank_name,
            bank_account,
            account_name,
        } => [
            D1Type::Text(email_lower),
            D1Type::Text(method),
            D1Type::Null,
            D1Type::Text(bank_name),
            D1Type::Text(bank_account),
            D1Type::Text(account_name),
        ],
    };
    db.prepare(SAVE_SQL)
        .bind_refs(&binds)
        .map_err(|e| format!("D1 credit_refund_accounts save bind: {e:?}"))
}

/// The bound [`DELETE_FOR_PERSON_SQL`] statement.
pub(crate) fn delete_for_person_statement(
    db: &D1Database,
    email_lower: &str,
) -> Result<D1PreparedStatement, String> {
    db.prepare(DELETE_FOR_PERSON_SQL)
        .bind_refs(&[D1Type::Text(email_lower)])
        .map_err(|e| format!("D1 credit_refund_accounts delete bind: {e:?}"))
}

/// Run the nightly purge; returns rows deleted.
pub async fn purge(db: &D1Database) -> Result<usize, String> {
    let modifier = format!("-{RETENTION_DAYS} days");
    let result = db
        .prepare(PURGE_SQL)
        .bind_refs(&[D1Type::Text(&modifier)])
        .map_err(|e| format!("D1 credit_refund_accounts purge bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 credit_refund_accounts purge run: {e:?}"))?;
    Ok(result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0))
}

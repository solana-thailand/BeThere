//! Where an attendee wants held credit paid back to (`.issues/190`, migration
//! `0059_credit_refund_payout.sql`).
//!
//! One row per contact email, from one of two sources:
//!
//! - **deposit** — the refund account the attendee already gave with their THB
//!   deposit, copied when that deposit is held as credit
//!   ([`snapshot_from_deposit`], called by the only writer of
//!   `held_as_credit = 1`, `thb_deposits::try_settle_hold_credit`). A newer
//!   deposit replaces an older deposit row; it never replaces an attendee row.
//! - **attendee** — an account typed on the credit refund card ([`SAVE_SQL`],
//!   written only together with the request flag, one D1 batch). Flagged
//!   `replaced_deposit_account` when it differs from the deposit account.
//!
//! Deleted at payout (`contacts::clear_credit_refund_requested`), by the
//! nightly purge ([`purge`]) once the person has no open request and no held
//! credit, and by PDPA erasure (`contacts::clear_contact_pii`).
//!
//! The account number is personal data: nothing here or in any caller may log
//! it. The attendee API gets a [`SavedAccountPreview`] only; the staff payout
//! queue is the only reader of the full number.

use event_checkin_domain::models::credit_payout::{
    AccountSource, RefundAccount, SavedAccountPreview,
};
use worker::D1Database;
use worker::d1::{D1PreparedStatement, D1Type};

use super::d1_safe::safe_all_rows;

/// SQL subquery: the email whose account row speaks for the person `$email`
/// belongs to. An account the attendee entered beats one copied from a
/// deposit; then the most recent. Shared by the attendee preview and the
/// organizer queue, so both show the same account. Same literal-argument
/// contract as `person_emails_of!`.
macro_rules! chosen_account_email_of {
    ($email:literal) => {
        concat!(
            "(SELECT p.email FROM credit_refund_accounts p WHERE p.email IN ",
            $crate::db::person::person_emails_of!($email),
            " ORDER BY (p.source = 'attendee') DESC, p.captured_at DESC, p.email LIMIT 1)"
        )
    };
}
pub(crate) use chosen_account_email_of;

/// Upsert the account the attendee typed, for an email **whose request flag
/// is set** — the `WHERE EXISTS` makes a row without an open request
/// impossible to write. Runs in the same batch as the flag `UPDATE`, after it.
///
/// `replaced_deposit_account` is 1 when a deposit-sourced account of the
/// person differs from this one, or the person's row already carried the
/// flag: the organizer is then told to confirm with the attendee before
/// paying.
///
/// `?1` lowercased email, `?2` method, `?3` promptpay id, `?4` bank name,
/// `?5` bank account, `?6` account name (`NULL` for the other method's
/// fields — the table's CHECK requires it).
pub(crate) const SAVE_SQL: &str = concat!(
    "INSERT INTO credit_refund_accounts \
     (email, method, promptpay_id, bank_name, bank_account, account_name, \
      source, source_deposit_ref, captured_at, replaced_deposit_account, updated_at) \
     SELECT ?1, ?2, ?3, ?4, ?5, ?6, 'attendee', NULL, \
       strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), \
       EXISTS (SELECT 1 FROM credit_refund_accounts p WHERE p.email IN ",
    crate::db::person::person_emails_of!("?1"),
    " AND (p.replaced_deposit_account = 1 \
            OR (p.source = 'deposit' AND (p.method IS NOT ?2 OR p.promptpay_id IS NOT ?3 \
                OR p.bank_name IS NOT ?4 OR p.bank_account IS NOT ?5 \
                OR p.account_name IS NOT ?6)))), \
       datetime('now') \
     WHERE EXISTS (SELECT 1 FROM contacts WHERE email = ?1 AND credit_refund_requested = 1) \
     ON CONFLICT (email) DO UPDATE SET method = excluded.method, \
       promptpay_id = excluded.promptpay_id, bank_name = excluded.bank_name, \
       bank_account = excluded.bank_account, account_name = excluded.account_name, \
       source = 'attendee', source_deposit_ref = NULL, captured_at = excluded.captured_at, \
       replaced_deposit_account = excluded.replaced_deposit_account, \
       updated_at = excluded.updated_at"
);

/// Copy the refund account of one held deposit (`?2` event id, `?3` attendee
/// id — both, since attendee ids are global) to the credit holder `?1`
/// (lowercased ledger email).
///
/// The mapping, identical to the 0059 backfill (`test_credit_payout.py`
/// compares them):
/// - all three deposit fields non-blank after trim, else nothing is written;
/// - a bank name of "PromptPay" / "พร้อมเพย์" (the deposit form's bank name is
///   free text) is a PromptPay account when the number is a valid PromptPay
///   ID, and nothing is written otherwise;
/// - anything else is a bank account, fields trimmed.
///
/// Only a held, unrefunded deposit qualifies. An existing row is replaced
/// only when it, too, came from a deposit: the attendee's own choice wins.
pub(crate) const SNAPSHOT_FROM_DEPOSIT_SQL: &str = "INSERT INTO credit_refund_accounts \
     (email, method, promptpay_id, bank_name, bank_account, account_name, \
      source, source_deposit_ref, captured_at, replaced_deposit_account, updated_at) \
     SELECT ?1, CASE WHEN x.pp THEN 'promptpay' ELSE 'bank' END, \
       CASE WHEN x.pp THEN x.digits END, \
       CASE WHEN x.pp THEN NULL ELSE x.bn END, \
       CASE WHEN x.pp THEN NULL ELSE x.ba END, \
       CASE WHEN x.pp THEN NULL ELSE x.an END, \
       'deposit', x.ref, x.uploaded_at, 0, datetime('now') \
     FROM (SELECT d.id, d.event_id || ':' || d.attendee_id AS ref, d.uploaded_at, \
             TRIM(COALESCE(d.bank_name, '')) AS bn, \
             TRIM(COALESCE(d.bank_account, '')) AS ba, \
             TRIM(COALESCE(d.account_name, '')) AS an, \
             REPLACE(REPLACE(TRIM(COALESCE(d.bank_account, '')), ' ', ''), '-', '') AS digits, \
             LOWER(REPLACE(TRIM(COALESCE(d.bank_name, '')), ' ', '')) \
               IN ('promptpay', 'พร้อมเพย์') AS pp \
           FROM thb_deposits d \
           WHERE d.event_id = ?2 AND d.attendee_id = ?3 \
             AND d.held_as_credit = 1 AND d.refunded = 0) x \
     WHERE x.bn <> '' AND x.ba <> '' AND x.an <> '' \
       AND (NOT x.pp OR (x.digits NOT GLOB '*[^0-9]*' \
            AND (LENGTH(x.digits) = 13 OR (LENGTH(x.digits) = 10 AND x.digits GLOB '0*')))) \
     ORDER BY x.id \
     ON CONFLICT (email) DO UPDATE SET method = excluded.method, \
       promptpay_id = excluded.promptpay_id, bank_name = excluded.bank_name, \
       bank_account = excluded.bank_account, account_name = excluded.account_name, \
       source_deposit_ref = excluded.source_deposit_ref, \
       captured_at = excluded.captured_at, updated_at = excluded.updated_at \
     WHERE credit_refund_accounts.source = 'deposit'";

/// Delete the account of every email of one person (`?1`). The payout is
/// person-wide, and so is the request clear that runs it.
pub(crate) const DELETE_FOR_PERSON_SQL: &str = concat!(
    "DELETE FROM credit_refund_accounts WHERE email IN ",
    crate::db::person::person_emails_of!("?1")
);

/// The nightly purge. A row stays while its person has an open request, a
/// positive credit bucket, or credit applied to an event and not yet returned
/// (locked); it goes once none of those holds — every baht spent or paid out.
/// The balance is the ledger's own definition (`positive_buckets_of!`,
/// `unreturned_apply_of!`), over the person's linked emails.
pub(crate) const PURGE_SQL: &str = concat!(
    "DELETE FROM credit_refund_accounts \
     WHERE NOT EXISTS (SELECT 1 FROM contacts c \
                       WHERE c.credit_refund_requested = 1 AND c.email IN ",
    crate::db::person::person_emails_of!("credit_refund_accounts.email"),
    ") AND NOT EXISTS (",
    crate::db::credit_ledger::positive_buckets_of!("credit_refund_accounts.email"),
    ") AND NOT EXISTS (SELECT 1 FROM credit_ledger l WHERE ",
    crate::db::credit_ledger::unreturned_apply_of!("credit_refund_accounts.email"),
    ")"
);

/// The person's chosen account (`?1` lowercased email), for the preview and
/// the "use the saved account" request.
pub(crate) const CHOSEN_FOR_PERSON_SQL: &str = concat!(
    "SELECT method, promptpay_id, bank_name, bank_account, account_name, \
       source, captured_at \
     FROM credit_refund_accounts WHERE email = ",
    chosen_account_email_of!("?1")
);

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

/// Copy a just-held deposit's refund account to its credit holder
/// ([`SNAPSHOT_FROM_DEPOSIT_SQL`]). Returns rows written: 0 when the deposit
/// has no usable account or the holder already chose their own.
pub async fn snapshot_from_deposit(
    db: &D1Database,
    email: &str,
    event_id: &str,
    attendee_id: &str,
) -> Result<usize, String> {
    let email_lower = email.trim().to_lowercase();
    let result = db
        .prepare(SNAPSHOT_FROM_DEPOSIT_SQL)
        .bind_refs(&[
            D1Type::Text(&email_lower),
            D1Type::Text(event_id),
            D1Type::Text(attendee_id),
        ])
        .map_err(|e| format!("D1 credit_refund_accounts snapshot bind: {e:?}"))?
        .run()
        .await
        .map_err(|e| format!("D1 credit_refund_accounts snapshot run: {e:?}"))?;
    Ok(result
        .meta()
        .ok()
        .flatten()
        .and_then(|m| m.changes)
        .unwrap_or(0))
}

/// The person's chosen account and where it came from, or `None` when there
/// is none (or the stored row does not parse — "no account on file", never
/// the wrong one).
pub async fn chosen_for_person(
    db: &D1Database,
    email: &str,
) -> Result<Option<(RefundAccount, AccountSource, String)>, String> {
    let email_lower = email.trim().to_lowercase();
    let stmt = db
        .prepare(CHOSEN_FOR_PERSON_SQL)
        .bind_refs(&[D1Type::Text(&email_lower)])
        .map_err(|e| format!("D1 credit_refund_accounts chosen bind: {e:?}"))?;
    let rows = safe_all_rows(&stmt).await?;
    let Some(row) = rows.into_iter().next() else {
        return Ok(None);
    };
    let text = |key: &str| row.get(key).and_then(|v| v.as_str()).map(str::to_string);
    let account = RefundAccount::from_columns(
        &text("method").unwrap_or_default(),
        text("promptpay_id"),
        text("bank_name"),
        text("bank_account"),
        text("account_name"),
    );
    let source = text("source").as_deref().and_then(AccountSource::parse);
    Ok(match (account, source) {
        (Some(account), Some(source)) => {
            Some((account, source, text("captured_at").unwrap_or_default()))
        }
        _ => None,
    })
}

/// The masked preview of the person's chosen account, for the attendee API.
pub async fn preview_for_person(
    db: &D1Database,
    email: &str,
) -> Result<Option<SavedAccountPreview>, String> {
    Ok(chosen_for_person(db, email)
        .await?
        .map(|(account, source, captured_at)| account.preview(source, &captured_at)))
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
    let result = db
        .prepare(PURGE_SQL)
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

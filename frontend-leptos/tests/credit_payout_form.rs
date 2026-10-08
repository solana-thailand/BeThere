//! The held-credit payout form and queue helpers (`.issues/190`).
//!
//! The attendee form validates with the domain rule the worker applies, and
//! the organizer's confirmed amount is parsed strictly: a typo must not turn
//! into a different payout.

use event_checkin_domain::models::credit_payout::{
    AccountSource, PaidAmounts, RefundAccount, RefundAccountError, RefundMethod,
};
use event_checkin_frontend::utils::credit_payout::{
    account_copy_value, account_from_form, account_lines, account_source_badge, age_label,
    age_line, parse_amount, parse_paid, saved_account_label, saved_account_sentence,
};

#[test]
fn the_form_reads_only_the_chosen_method() {
    let pp = account_from_form(RefundMethod::PromptPay, "081-234-5678", "x", "y", "z");
    assert_eq!(
        pp,
        Ok(RefundAccount::PromptPay {
            promptpay_id: "0812345678".into()
        })
    );
    let bank = account_from_form(RefundMethod::Bank, "junk", " KBank ", "123-4", " Somchai ");
    assert_eq!(
        bank,
        Ok(RefundAccount::Bank {
            bank_name: "KBank".into(),
            bank_account: "123-4".into(),
            account_name: "Somchai".into(),
        })
    );
}

#[test]
fn the_form_refuses_what_the_server_refuses() {
    assert_eq!(
        account_from_form(RefundMethod::PromptPay, "12345", "", "", ""),
        Err(RefundAccountError::PromptPayIdInvalid)
    );
    assert_eq!(
        account_from_form(RefundMethod::Bank, "0812345678", "KBank", "", "Somchai"),
        Err(RefundAccountError::BankAccountMissing),
        "a PromptPay ID typed before switching to bank must not stand in for the account"
    );
}

#[test]
fn amounts_are_whole_non_negative_numbers() {
    assert_eq!(parse_amount(""), Ok(0));
    assert_eq!(parse_amount(" 500 "), Ok(500));
    assert_eq!(parse_amount("1,500"), Ok(1500));
    for bad in ["-500", "500.50", "5OO", "฿500", "1e3"] {
        assert!(parse_amount(bad).is_err(), "{bad:?}");
    }
    assert_eq!(parse_paid("500", ""), Ok(PaidAmounts { thb: 500, usdc: 0 }));
    assert!(parse_paid("500", "x").is_err());
}

#[test]
fn the_age_reads_in_hours_then_days_and_flags_the_promise() {
    assert_eq!(age_label(0), "under an hour");
    assert_eq!(age_label(1), "1 hour");
    assert_eq!(age_label(23), "23 hours");
    assert_eq!(age_label(30), "1 day");
    assert_eq!(age_label(24 * 9 + 5), "9 days");
    assert_eq!(age_line(48), "Open 2 days");
    assert_eq!(age_line(24 * 8), "Open 8 days — past the 7-day promise");
}

#[test]
fn the_organizer_sees_and_copies_the_account() {
    let bank = RefundAccount::Bank {
        bank_name: "KBank".into(),
        bank_account: "123-4-56789-0".into(),
        account_name: "Somchai".into(),
    };
    assert_eq!(
        account_lines(&bank),
        vec![
            ("Bank", "KBank".to_string()),
            ("Account", "123-4-56789-0".to_string()),
            ("Name", "Somchai".to_string()),
        ]
    );
    assert_eq!(account_copy_value(&bank), "1234567890");
    let pp = RefundAccount::PromptPay {
        promptpay_id: "0812345678".into(),
    };
    assert_eq!(
        account_lines(&pp),
        vec![("PromptPay", "0812345678".to_string())]
    );
    assert_eq!(account_copy_value(&pp), "0812345678");
}

// -- the one-tap card and the queue badge ------------------------------------

const FROM_DEPOSIT: &str = "We'll send it to {account} — the account from your deposit on {date}.";
const FROM_ATTENDEE: &str = "We'll send it to {account} — the account you gave us on {date}.";

fn deposit_bank() -> RefundAccount {
    RefundAccount::Bank {
        bank_name: "Kasikornbank (KBANK)".into(),
        bank_account: "123-4-56789-0".into(),
        account_name: "Somchai Jaidee".into(),
    }
}

#[test]
fn the_card_names_the_deposit_account_masked() {
    let preview = deposit_bank().preview(AccountSource::Deposit, "2026-08-30T09:00:00Z");
    let label = saved_account_label(&preview, "PromptPay");
    assert_eq!(label, "Kasikornbank (KBANK) •••• 7890 · Somchai J.");
    let sentence =
        saved_account_sentence(&preview, FROM_DEPOSIT, FROM_ATTENDEE, &label, "30 Aug 2026");
    assert_eq!(
        sentence,
        "We'll send it to Kasikornbank (KBANK) •••• 7890 · Somchai J. — the account from \
         your deposit on 30 Aug 2026."
    );
    for secret in ["1234567890", "123-4-56789-0", "Jaidee"] {
        assert!(!sentence.contains(secret), "{secret}");
    }
}

#[test]
fn the_card_names_a_promptpay_account_and_its_source() {
    let preview = RefundAccount::PromptPay {
        promptpay_id: "0812345678".into(),
    }
    .preview(AccountSource::Attendee, "t");
    let label = saved_account_label(&preview, "พร้อมเพย์");
    assert_eq!(label, "พร้อมเพย์ •••• 5678");
    assert_eq!(
        saved_account_sentence(&preview, FROM_DEPOSIT, FROM_ATTENDEE, &label, "1 Oct 2026"),
        "We'll send it to พร้อมเพย์ •••• 5678 — the account you gave us on 1 Oct 2026."
    );
}

#[test]
fn a_short_number_shows_no_digits() {
    let preview = RefundAccount::Bank {
        bank_name: "KBank".into(),
        bank_account: "1234".into(),
        account_name: "S".into(),
    }
    .preview(AccountSource::Deposit, "t");
    assert_eq!(saved_account_label(&preview, "PromptPay"), "KBank •••• · S");
}

#[test]
fn the_queue_badge_says_where_the_account_came_from() {
    assert_eq!(account_source_badge(None, false, ""), None);
    assert_eq!(
        account_source_badge(Some(AccountSource::Deposit), false, "30 Aug 2026"),
        Some((
            "badge badge-info",
            "From deposit on 30 Aug 2026".to_string()
        ))
    );
    assert_eq!(
        account_source_badge(Some(AccountSource::Attendee), false, "x"),
        Some(("badge badge-neutral", "Entered by attendee".to_string()))
    );
    let (class, text) =
        account_source_badge(Some(AccountSource::Attendee), true, "x").expect("warning");
    assert_eq!(class, "badge badge-danger");
    assert!(
        text.contains("confirm with the attendee before paying"),
        "{text}"
    );
}

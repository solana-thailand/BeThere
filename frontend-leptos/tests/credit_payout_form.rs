//! The held-credit payout form and queue helpers (`.issues/190`).
//!
//! The attendee form validates with the domain rule the worker applies, and
//! the organizer's confirmed amount is parsed strictly: a typo must not turn
//! into a different payout.

use event_checkin_domain::models::credit_payout::{
    PaidAmounts, RefundAccount, RefundAccountError, RefundMethod,
};
use event_checkin_frontend::utils::credit_payout::{
    account_copy_value, account_from_form, account_lines, age_label, age_line, parse_amount,
    parse_paid,
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

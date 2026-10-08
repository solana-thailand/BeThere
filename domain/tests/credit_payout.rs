//! Held-credit payout types (`.issues/190`): the refund account the attendee
//! gives, the amount the organizer confirms, and the payout scope.

use std::collections::BTreeSet;

use event_checkin_domain::models::credit_payout::{
    PaidAmounts, PayoutScope, RefundAccount, RefundAccountError, RefundMethod, is_overdue,
    normalize_promptpay_id, payout_mismatch_message, validate_bank_refund_fields,
};

fn bank(name: &str, account: &str, holder: &str) -> RefundAccount {
    RefundAccount::Bank {
        bank_name: name.to_string(),
        bank_account: account.to_string(),
        account_name: holder.to_string(),
    }
}

// -- the bank rule is the deposit refund account's rule ---------------------

#[test]
fn bank_fields_are_all_required_in_the_deposit_order() {
    let ok = validate_bank_refund_fields(Some("123"), Some("KBank"), Some("Somchai"));
    assert_eq!(ok, Ok(()));
    assert_eq!(
        validate_bank_refund_fields(None, None, None),
        Err(RefundAccountError::BankAccountMissing)
    );
    assert_eq!(
        validate_bank_refund_fields(Some("123"), Some("  "), None),
        Err(RefundAccountError::BankNameMissing)
    );
    assert_eq!(
        validate_bank_refund_fields(Some("123"), Some("KBank"), Some("")),
        Err(RefundAccountError::AccountNameMissing)
    );
}

#[test]
fn bank_messages_are_the_ones_the_slip_upload_always_returned() {
    assert_eq!(
        RefundAccountError::BankAccountMissing.to_string(),
        "bank_account is required"
    );
    assert_eq!(
        RefundAccountError::BankNameMissing.to_string(),
        "bank_name is required"
    );
    assert_eq!(
        RefundAccountError::AccountNameMissing.to_string(),
        "account_name is required"
    );
}

#[test]
fn no_message_contains_a_scheme() {
    // The error redactor rewrites anything with "://" (memory:
    // error-redactor-eats-scheme-text).
    for e in [
        RefundAccountError::BankAccountMissing,
        RefundAccountError::BankNameMissing,
        RefundAccountError::AccountNameMissing,
        RefundAccountError::PromptPayIdInvalid,
    ] {
        assert!(!e.to_string().contains("://"), "{e}");
    }
    let msg = payout_mismatch_message(
        PaidAmounts { thb: 500, usdc: 0 },
        PaidAmounts { thb: 0, usdc: 0 },
    );
    assert!(!msg.contains("://"), "{msg}");
}

#[test]
fn a_bank_account_is_trimmed_and_kept() {
    let got = bank(" KBank ", " 123-4-56789-0 ", " Somchai ").normalized();
    assert_eq!(got, Ok(bank("KBank", "123-4-56789-0", "Somchai")));
    assert_eq!(
        bank("KBank", " ", "Somchai").normalized(),
        Err(RefundAccountError::BankAccountMissing)
    );
}

// -- PromptPay --------------------------------------------------------------

#[test]
fn promptpay_takes_a_mobile_or_a_national_id() {
    assert_eq!(
        normalize_promptpay_id("081-234-5678"),
        Ok("0812345678".into())
    );
    assert_eq!(
        normalize_promptpay_id("081 234 5678"),
        Ok("0812345678".into())
    );
    assert_eq!(
        normalize_promptpay_id("1-2345-67890-12-3"),
        Ok("1234567890123".into())
    );
}

#[test]
fn promptpay_refuses_anything_else() {
    for bad in [
        "",
        "   ",
        "812345678",    // 9 digits
        "8123456789",   // 10 digits, no leading 0
        "08123456789",  // 11
        "081234567a",   // a letter
        "+66812345678", // country code
        "081.234.5678", // separator we do not strip
    ] {
        assert_eq!(
            normalize_promptpay_id(bad),
            Err(RefundAccountError::PromptPayIdInvalid),
            "{bad:?}"
        );
    }
}

// -- wire and storage shape -------------------------------------------------

#[test]
fn the_wire_shape_is_tagged_by_method() {
    let pp: RefundAccount =
        serde_json::from_str(r#"{"method":"promptpay","promptpay_id":"0812345678"}"#).unwrap();
    assert_eq!(pp.method(), RefundMethod::PromptPay);
    let b: RefundAccount = serde_json::from_str(
        r#"{"method":"bank","bank_name":"KBank","bank_account":"1","account_name":"S"}"#,
    )
    .unwrap();
    assert_eq!(b, bank("KBank", "1", "S"));
    assert!(serde_json::from_str::<RefundAccount>(r#"{"method":"cash"}"#).is_err());
}

#[test]
fn stored_columns_round_trip_and_unknowns_are_none() {
    assert_eq!(
        RefundAccount::from_columns("promptpay", Some("0812345678".into()), None, None, None),
        Some(RefundAccount::PromptPay {
            promptpay_id: "0812345678".into()
        })
    );
    assert_eq!(
        RefundAccount::from_columns(
            "bank",
            None,
            Some("KBank".into()),
            Some("1".into()),
            Some("S".into())
        ),
        Some(bank("KBank", "1", "S"))
    );
    assert_eq!(
        RefundAccount::from_columns("bank", None, Some("KBank".into()), None, Some("S".into())),
        None
    );
    assert_eq!(
        RefundAccount::from_columns("cash", None, None, None, None),
        None
    );
    for method in [RefundMethod::PromptPay, RefundMethod::Bank] {
        assert_eq!(RefundMethod::parse(method.as_str()), Some(method));
    }
}

// -- confirmed amounts ------------------------------------------------------

#[test]
fn paid_amounts_read_thb_first_and_refuse_negatives() {
    assert_eq!(PaidAmounts { thb: 500, usdc: 0 }.to_string(), "500 THB");
    assert_eq!(PaidAmounts { thb: 0, usdc: 3 }.to_string(), "3 USDC");
    assert_eq!(
        PaidAmounts { thb: 500, usdc: 3 }.to_string(),
        "500 THB + 3 USDC"
    );
    assert_eq!(PaidAmounts::default().to_string(), "0");
    assert!(PaidAmounts { thb: -1, usdc: 0 }.validate().is_err());
    assert!(PaidAmounts { thb: 0, usdc: 0 }.validate().is_ok());
}

#[test]
fn paid_amounts_add_buckets_by_currency() {
    let mut payable = PaidAmounts::default();
    payable.add("thb", 500);
    payable.add("thb", 300);
    payable.add("usdc", 2);
    payable.add("eur", 9);
    assert_eq!(payable, PaidAmounts { thb: 800, usdc: 2 });
    let missing_usdc: PaidAmounts = serde_json::from_str(r#"{"thb":800}"#).unwrap();
    assert_ne!(missing_usdc, payable);
}

// -- scope and age ----------------------------------------------------------

#[test]
fn a_scope_must_cover_every_org_of_the_credit() {
    let orgs = |ids: &[&str]| PayoutScope::Orgs(ids.iter().map(|s| s.to_string()).collect());
    assert!(PayoutScope::All.covers(["", "org-b"]));
    assert!(orgs(&[""]).covers([""]));
    assert!(!orgs(&[""]).covers(["", "org-b"]));
    assert!(orgs(&["", "org-b"]).covers(["org-b"]));
    assert!(orgs(&[]).is_empty());
    assert!(!PayoutScope::Orgs(BTreeSet::from([String::new()])).is_empty());
    assert!(!PayoutScope::All.is_empty());
}

#[test]
fn overdue_is_past_seven_days() {
    assert!(!is_overdue(0));
    assert!(!is_overdue(7 * 24));
    assert!(is_overdue(7 * 24 + 1));
}

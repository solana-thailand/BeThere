//! An account the organizer typed after paying someone with no account on
//! file (`.issues/194`): stricter than the attendee rule, and the audit keeps
//! only the last four digits.

use event_checkin_domain::models::credit_payout::{RefundAccount, RefundAccountError};

fn bank(account: &str) -> RefundAccount {
    RefundAccount::Bank {
        bank_name: " KBank ".to_string(),
        bank_account: account.to_string(),
        account_name: " Somchai ".to_string(),
    }
}

#[test]
fn a_one_digit_account_is_refused() {
    // Pond's deposit account was "2" — the attendee rule let it through.
    assert!(bank("2").normalized().is_ok());
    assert_eq!(
        bank("2").normalized_organizer_typed(),
        Err(RefundAccountError::BankAccountDigits)
    );
}

#[test]
fn ten_to_fifteen_digits_pass_with_dashes_and_spaces() {
    for ok in [
        "1234567890",
        "123-4-56789-0",
        "123 456 789 012",
        "123456789012345",
    ] {
        assert!(bank(ok).normalized_organizer_typed().is_ok(), "{ok}");
    }
    for bad in ["123456789", "1234567890123456", "12345abc90", ""] {
        assert!(bank(bad).normalized_organizer_typed().is_err(), "{bad:?}");
    }
}

#[test]
fn the_other_fields_are_still_required() {
    let missing_name = RefundAccount::Bank {
        bank_name: "KBank".to_string(),
        bank_account: "1234567890".to_string(),
        account_name: "  ".to_string(),
    };
    assert_eq!(
        missing_name.normalized_organizer_typed(),
        Err(RefundAccountError::AccountNameMissing)
    );
}

#[test]
fn promptpay_follows_the_existing_rule() {
    let ok = RefundAccount::PromptPay {
        promptpay_id: "081-234-5678".to_string(),
    };
    assert!(ok.normalized_organizer_typed().is_ok());
    let bad = RefundAccount::PromptPay {
        promptpay_id: "12345".to_string(),
    };
    assert_eq!(
        bad.normalized_organizer_typed(),
        Err(RefundAccountError::PromptPayIdInvalid)
    );
}

#[test]
fn the_audit_keeps_bank_holder_and_last_four_only() {
    let label = bank("123-4-56789-0")
        .normalized_organizer_typed()
        .expect("valid")
        .audit_label();
    assert_eq!(label, "KBank ···7890 (Somchai)");
    assert!(!label.contains("123456789"));
    let pp = RefundAccount::PromptPay {
        promptpay_id: "0812345678".to_string(),
    }
    .audit_label();
    assert_eq!(pp, "PromptPay ···5678");
}

#[test]
fn the_error_message_keeps_scheme_text_out() {
    let msg = RefundAccountError::BankAccountDigits.to_string();
    assert!(!msg.contains("://"));
    assert_eq!(msg, "bank_account must be 10 to 15 digits");
}

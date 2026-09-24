//! The event form's USDC deposit hints and submit path share one verdict
//! (`check_usdc_deposit`). Before, the hints parsed f64 and disagreed with
//! submit: `1e3` showed no warning but was rejected, and a 7-decimal value
//! showed "min 0.01" instead of the decimals error.

use event_checkin_domain::money::{
    USDC_MAX_DEPOSIT_ATOMIC, USDC_MIN_DEPOSIT_ATOMIC, UsdcDepositCheck, check_usdc_deposit,
};

#[test]
fn blank_and_zero_are_empty() {
    for input in ["", "   ", "0", "0.0", "0.000000", ".0"] {
        assert_eq!(
            check_usdc_deposit(input),
            UsdcDepositCheck::Empty,
            "{input:?}"
        );
        assert_eq!(check_usdc_deposit(input).atomic(), Some(0), "{input:?}");
        assert_eq!(check_usdc_deposit(input).error_message(), None, "{input:?}");
    }
}

#[test]
fn f64_only_shapes_are_invalid() {
    for input in [
        "1e3", "1E3", "-1", "+1", "0x10", "inf", "NaN", "1,000", "1.2.3", ".",
    ] {
        assert_eq!(
            check_usdc_deposit(input),
            UsdcDepositCheck::Invalid,
            "{input:?}"
        );
        assert_eq!(check_usdc_deposit(input).atomic(), None, "{input:?}");
    }
}

#[test]
fn seven_decimals_is_invalid_not_below_min() {
    let check = check_usdc_deposit("0.0000001");
    assert_eq!(check, UsdcDepositCheck::Invalid);
    assert!(
        check
            .error_message()
            .is_some_and(|m| m.contains("6 decimals"))
    );
}

#[test]
fn min_boundary_is_inclusive() {
    assert_eq!(check_usdc_deposit("0.009999"), UsdcDepositCheck::BelowMin);
    assert_eq!(check_usdc_deposit("0.000001"), UsdcDepositCheck::BelowMin);
    assert_eq!(
        check_usdc_deposit("0.01"),
        UsdcDepositCheck::Valid(USDC_MIN_DEPOSIT_ATOMIC)
    );
}

#[test]
fn max_boundary_is_inclusive() {
    assert_eq!(
        check_usdc_deposit("1000"),
        UsdcDepositCheck::Valid(USDC_MAX_DEPOSIT_ATOMIC)
    );
    assert_eq!(
        check_usdc_deposit("1000.000001"),
        UsdcDepositCheck::AboveMax
    );
    assert_eq!(
        check_usdc_deposit("99999999999"),
        UsdcDepositCheck::AboveMax
    );
    // Past u64::MAX atomic: unparseable, not a silent wrap.
    assert_eq!(
        check_usdc_deposit("99999999999999999999"),
        UsdcDepositCheck::Invalid
    );
}

#[test]
fn valid_amounts_are_exact() {
    assert_eq!(
        check_usdc_deposit(" 2.01 "),
        UsdcDepositCheck::Valid(2_010_000)
    );
    assert_eq!(
        check_usdc_deposit("10"),
        UsdcDepositCheck::Valid(10_000_000)
    );
    assert_eq!(check_usdc_deposit(".5").atomic(), Some(500_000));
}

#[test]
fn messages_stay_redactor_safe() {
    let unusable = [
        UsdcDepositCheck::Invalid,
        UsdcDepositCheck::BelowMin,
        UsdcDepositCheck::AboveMax,
    ];
    for check in unusable {
        let message = check.error_message().expect("unusable input has a message");
        assert!(!message.contains("://"), "{message}");
    }
}

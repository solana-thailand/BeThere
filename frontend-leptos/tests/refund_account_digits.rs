//! The refund queue copies the account number as digits only.

use event_checkin_frontend::pages::admin_deposit_bank_info::account_digits;

#[test]
fn dashes_and_spaces_are_dropped() {
    assert_eq!(account_digits("123-4-56789-0"), "1234567890");
    assert_eq!(account_digits(" 123 456 7890 "), "1234567890");
}

#[test]
fn no_digits_copies_nothing() {
    assert_eq!(account_digits("n/a"), "");
}

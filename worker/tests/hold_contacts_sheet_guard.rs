//! `.issues/155`: holding a deposit as credit must not depend on the contacts
//! sheet. The D1 ledger records the credit; the sheet only mirrors it. Both
//! hold handlers used to 500 before settling when no contacts sheet was
//! configured (staging has none), so a display mirror blocked a money action.

const HANDLERS: [(&str, &str); 2] = [
    (
        "hold_credit.rs",
        include_str!("../src/handlers/deposit/thb/handlers/hold_credit.rs"),
    ),
    (
        "hold_admin.rs",
        include_str!("../src/handlers/deposit/thb/handlers/hold_admin.rs"),
    ),
];

#[test]
fn missing_contacts_sheet_is_fatal_only_without_d1() {
    for (path, src) in HANDLERS {
        assert!(
            !src.contains("if resolved.sheet_id.is_empty() {\n        return Err("),
            "{path}: an empty contacts sheet must not fail the hold when D1 records it"
        );
        assert!(
            src.contains("if resolved.sheet_id.is_empty() && d1.is_none() {"),
            "{path}: without D1 the sheet is the only record, so it is still required"
        );
    }
}

#[test]
fn sheets_mirror_is_skipped_not_called_with_an_empty_sheet() {
    for (path, src) in HANDLERS {
        let skip = src
            .find("credit Sheets mirror skipped")
            .unwrap_or_else(|| panic!("{path}: log the skipped mirror"));
        let call = src
            .find("crate::sheets::contacts::increment_credit(")
            .unwrap_or_else(|| panic!("{path}: mirror call missing"));
        let ledger = src
            .find("crate::db::credit_ledger::record(")
            .unwrap_or_else(|| panic!("{path}: ledger write missing"));
        assert!(
            ledger < skip && skip < call,
            "{path}: ledger first, then the mirror behind the empty-sheet check"
        );
    }
}

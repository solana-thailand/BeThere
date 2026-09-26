//! `ColumnMapping::pii_column_letters` — the cells a PDPA erasure blanks on
//! the attendee's sheet row (`.issues/151` D). They used to be fixed letters,
//! correct only while nobody moved a column.

use event_checkin_domain::models::attendee::{ColumnMapping, PII_COLUMNS};

/// The letters `clear_sheet_pii` hardcoded before this change.
const LEGACY_LETTERS: [&str; 15] = [
    "B", "C", "D", "E", "J", "K", "L", "S", "T", "U", "V", "Y", "Z", "AA", "AC",
];

const STANDARD_HEADERS: [&str; 33] = [
    "api_id",
    "name",
    "first_name",
    "last_name",
    "email",
    "ticket_name",
    "registration_date",
    "approval_status",
    "participation_type",
    "phone",
    "contact_channel",
    "contact_handle",
    "deposit_agreed",
    "deposit_method",
    "deposit_amount",
    "deposit_tx_signature",
    "deposit_verified",
    "checked_in_at",
    "checked_in_by",
    "solana_address",
    "qr_code_url",
    "claim_token",
    "claimed_at",
    "nft_proof_url",
    "bank_account",
    "bank_name",
    "account_name",
    "refund_status",
    "refund_link",
    "send_email_status",
    "consent_given",
    "photo_consent",
    "consent_marketing",
];

fn headers(names: &[&str]) -> Vec<String> {
    names.iter().map(|h| h.to_string()).collect()
}

#[test]
fn standard_layout_matches_the_legacy_letters() {
    assert_eq!(
        ColumnMapping::hardcoded().pii_column_letters(),
        LEGACY_LETTERS
    );
    let mapping = ColumnMapping::from_headers(&headers(&STANDARD_HEADERS));
    assert_eq!(mapping.pii_column_letters(), LEGACY_LETTERS);
}

#[test]
fn a_moved_column_moves_the_cleared_cell() {
    // An owner-inserted "notes" column at B shifts every column right by one.
    let mut shifted = vec!["api_id", "notes"];
    shifted.extend_from_slice(&STANDARD_HEADERS[1..]);
    let letters = ColumnMapping::from_headers(&headers(&shifted)).pii_column_letters();
    assert_eq!(letters.len(), PII_COLUMNS.len());
    assert!(
        !letters.contains(&"B".to_string()),
        "B is notes now: {letters:?}"
    );
    assert_eq!(letters[0], "C", "name moved to C");
    assert_eq!(
        letters.last().map(String::as_str),
        Some("AD"),
        "refund_link"
    );
}

#[test]
fn a_missing_pii_header_clears_nothing_in_its_old_place() {
    // No phone column: J now holds contact_channel, which is cleared as
    // itself, and nothing is blanked on phone's behalf.
    let without_phone: Vec<&str> = STANDARD_HEADERS
        .iter()
        .copied()
        .filter(|h| *h != "phone")
        .collect();
    let letters = ColumnMapping::from_headers(&headers(&without_phone)).pii_column_letters();
    assert_eq!(letters.len(), PII_COLUMNS.len() - 1);
    assert_eq!(
        &letters[4..6],
        ["J", "K"],
        "contact_channel, contact_handle"
    );
}

#[test]
fn an_unrecognised_header_row_falls_back_to_the_standard_layout() {
    let letters =
        ColumnMapping::from_headers(&headers(&["Col 1", "Col 2", "Col 3"])).pii_column_letters();
    assert_eq!(letters, LEGACY_LETTERS);
}

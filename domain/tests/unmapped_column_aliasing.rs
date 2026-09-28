//! A key the sheet has no header for must not borrow the standard-layout
//! column (.issues/167).
//!
//! The header row below is the real dev sheet: a Luma export. It has
//! `claim_token` at L (index 11) and no `contact_handle` header, and the
//! standard layout puts `contact_handle` at L. Before the fix a
//! self-registration wrote its claim token to L, then overwrote it with the
//! handle, and the duplicate-registration path read `@handle` back as the
//! claim token, so the `/claim/{token}` link broke.

use event_checkin_domain::models::attendee::{
    AttendeeRow, CellWant, ColumnKey, ColumnMapping, plan_backfill,
};

const LUMA_HEADERS: &[&str] = &[
    "api_id",
    "name",
    "first_name",
    "last_name",
    "email",
    "phone_number",
    "created_at",
    "approval_status",
    "checked_in_at",
    "checked_in_by",
    "qr_code_url",
    "claim_token",
    "amount_tax",
    "amount_discount",
    "currency",
    "solana_address",
    "bethere_link",
    "claim_token",
    "claimed_at",
    "survey_response_feedback",
    "ticket_type_id",
    "ticket_name",
    "Professional Role / Background",
    "Primary Skillset",
    "Participation_Type",
    "Rust Proficiency",
    "Solana Experience Level",
    "Your Goal",
    "Who did you receive this invite from?",
    "Preferred Contact Channel / ช่องทางที่สะดวกให้ทีมงานติดต่อกลับ",
    "Contact Handle / โปรดระบุ Username",
    "ยอมรับการจ่ายมัดจำ 500 บาท",
    "payment_status",
];

fn luma_mapping() -> ColumnMapping {
    let headers: Vec<String> = LUMA_HEADERS.iter().map(|h| h.to_string()).collect();
    ColumnMapping::from_headers(&headers)
}

/// The row the self-registration writers build, in their order.
fn registration_row(mapping: &ColumnMapping, claim_token: &str, handle: &str) -> Vec<String> {
    let mut row = vec![String::new(); mapping.total_columns.max(31)];
    mapping.put(&mut row, ColumnKey::ApiId, "att-1".into());
    mapping.put(&mut row, ColumnKey::Email, "a@example.com".into());
    mapping.put(&mut row, ColumnKey::ApprovalStatus, "Approved".into());
    mapping.put(&mut row, ColumnKey::ClaimToken, claim_token.into());
    mapping.put(&mut row, ColumnKey::ContactChannel, "telegram".into());
    mapping.put(&mut row, ColumnKey::ContactHandle, handle.into());
    mapping.put(&mut row, ColumnKey::DepositAgreed, "Yes".into());
    mapping.put(&mut row, ColumnKey::ConsentGiven, "Yes".into());
    mapping.put(&mut row, ColumnKey::ConsentMarketing, "Yes".into());
    row
}

#[test]
fn registration_keeps_its_claim_token_on_a_luma_sheet() {
    let mapping = luma_mapping();
    let token = "0199a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b";
    let row = registration_row(&mapping, token, "@reviewtester");

    assert_eq!(row[11], token, "claim_token column L");

    let parsed = AttendeeRow::from_sheet_values(&[row], 2, &mapping).expect("row parses");
    assert_eq!(parsed.claim_token.as_deref(), Some(token));
    assert_eq!(
        parsed.contact_handle, None,
        "the sheet has no handle column"
    );
}

#[test]
fn unmapped_keys_leave_the_organizers_columns_alone() {
    let mapping = luma_mapping();
    let row = registration_row(&mapping, "tok", "@h");

    // Standard-layout slots of keys this sheet lacks: K qr_code_url (contact
    // channel), M amount_tax (deposit_agreed), AE the handle column
    // (consent_given), AG payment_status (consent_marketing).
    for (idx, what) in [
        (10, "qr_code_url"),
        (12, "amount_tax"),
        (30, "Contact Handle"),
        (32, "payment_status"),
    ] {
        assert_eq!(row[idx], "", "{what} was overwritten");
    }
}

#[test]
fn unmapped_keys_read_as_empty_not_as_a_neighbour() {
    let mapping = luma_mapping();
    let mut row = vec![String::new(); LUMA_HEADERS.len()];
    row[0] = "att-1".into();
    row[10] = "https://qr.example/1".into(); // standard contact_channel slot
    row[14] = "THB".into(); // standard deposit_amount slot (currency)
    row[28] = "a friend".into(); // standard refund_link slot

    let parsed = AttendeeRow::from_sheet_values(&[row], 2, &mapping).expect("row parses");
    assert_eq!(parsed.qr_code_url.as_deref(), Some("https://qr.example/1"));
    assert_eq!(parsed.contact_channel, None);
    assert_eq!(parsed.deposit_amount, None);
    assert_eq!(parsed.refund_link, None);
}

#[test]
fn an_unrecognised_header_row_still_uses_the_standard_layout() {
    let headers: Vec<String> = vec!["Guest".into(), "Mail".into()];
    let mapping = ColumnMapping::from_headers(&headers);
    assert!(!mapping.is_valid());
    assert_eq!(mapping.resolve(ColumnKey::ClaimToken), Some(21));
    assert_eq!(mapping.resolve(ColumnKey::ContactHandle), Some(11));
}

#[test]
fn backfill_skips_a_kind_the_sheet_has_no_column_for() {
    let mapping = luma_mapping();
    let mut data_row = vec![String::new(); LUMA_HEADERS.len()];
    data_row[0] = "att-1".into();
    let header: Vec<String> = LUMA_HEADERS.iter().map(|h| h.to_string()).collect();
    let sheet = vec![header, data_row];

    let plan = plan_backfill(
        &sheet,
        &mapping,
        &[
            CellWant {
                api_id: "att-1".into(),
                column: ColumnKey::DepositMethod,
                value: "thb".into(),
            },
            CellWant {
                api_id: "att-1".into(),
                column: ColumnKey::CheckedInAt,
                value: "2026-09-29T10:00:00Z".into(),
            },
        ],
    );
    assert_eq!(plan.column_missing, 1);
    assert_eq!(plan.writes.len(), 1);
    assert_eq!(plan.writes[0].column, 8, "checked_in_at column I");
}

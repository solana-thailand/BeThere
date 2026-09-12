use super::columns::index_to_column_letter;
use super::*;

fn make_attendee(participation_type: &str) -> Attendee {
    Attendee {
        api_id: "gst-test".to_string(),
        first_name: "Test".to_string(),
        last_name: "User".to_string(),
        name: "Test User".to_string(),
        email: "test@example.com".to_string(),
        ticket_name: "General".to_string(),
        approval_status: CheckInStatus::Approved,
        participation_type: participation_type.to_string(),
        registration_date: None,
        phone: None,
        contact_channel: None,
        contact_handle: None,
        deposit_agreed: None,
        deposit_method: None,
        deposit_amount: None,
        deposit_tx_signature: None,
        deposit_verified: None,
        checked_in_at: None,
        checked_in_by: None,
        solana_address: None,
        qr_code_url: None,
        claim_token: None,
        claimed_at: None,
        nft_proof_url: None,
        bank_account: None,
        bank_name: None,
        account_name: None,
        refund_status: None,
        refund_link: None,
        send_email_status: None,
        row_index: 2,
    }
}

// -----------------------------------------------------------------------
// Domain behavior tests: can_check_in, has_verified_deposit, is_refund_eligible
// -----------------------------------------------------------------------

#[test]
fn test_can_check_in_approved_in_person() {
    let a = make_attendee("In-Person");
    assert!(a.can_check_in().is_ok());
}

#[test]
fn test_can_check_in_already_checked_in() {
    let mut a = make_attendee("In-Person");
    a.checked_in_at = Some("2025-01-01T12:00:00Z".to_string());
    let err = a.can_check_in().unwrap_err();
    assert_eq!(
        err,
        CheckInError::AlreadyCheckedIn("2025-01-01T12:00:00Z".to_string())
    );
}

#[test]
fn test_can_check_in_not_approved() {
    let mut a = make_attendee("In-Person");
    a.approval_status = CheckInStatus::PendingApproval;
    let err = a.can_check_in().unwrap_err();
    assert!(matches!(err, CheckInError::NotApproved(_)));
}

#[test]
fn test_can_check_in_online_attendee() {
    let mut a = make_attendee("Online");
    a.approval_status = CheckInStatus::Approved;
    let err = a.can_check_in().unwrap_err();
    assert_eq!(err, CheckInError::OnlineAttendee);
}

#[test]
fn test_can_check_in_priority_already_checked_in() {
    // Already checked in takes priority over not-approved
    let mut a = make_attendee("In-Person");
    a.checked_in_at = Some("2025-01-01T12:00:00Z".to_string());
    a.approval_status = CheckInStatus::PendingApproval;
    let err = a.can_check_in().unwrap_err();
    assert!(matches!(err, CheckInError::AlreadyCheckedIn(_)));
}

#[test]
fn test_can_check_in_invited_status_allowed() {
    // Invited status is NOT in the Approved|CheckedIn match
    let mut a = make_attendee("In-Person");
    a.approval_status = CheckInStatus::Invited;
    let err = a.can_check_in().unwrap_err();
    assert!(matches!(err, CheckInError::NotApproved(_)));
}

#[test]
fn test_has_verified_deposit_none() {
    let a = make_attendee("In-Person");
    assert!(!a.has_verified_deposit());
}

#[test]
fn test_has_verified_deposit_empty_string() {
    let mut a = make_attendee("In-Person");
    a.deposit_verified = Some(String::new());
    assert!(!a.has_verified_deposit());
}

#[test]
fn test_has_verified_deposit_whitespace_only() {
    let mut a = make_attendee("In-Person");
    a.deposit_verified = Some("   ".to_string());
    assert!(!a.has_verified_deposit());
}

#[test]
fn test_has_verified_deposit_true_string() {
    let mut a = make_attendee("In-Person");
    a.deposit_verified = Some("true".to_string());
    assert!(a.has_verified_deposit());
}

#[test]
fn test_has_verified_deposit_timestamp() {
    let mut a = make_attendee("In-Person");
    a.deposit_verified = Some("2025-01-01T12:00:00Z".to_string());
    assert!(a.has_verified_deposit());
}

#[test]
fn test_is_refund_eligible_no_deposit() {
    let a = make_attendee("In-Person");
    assert!(!a.is_refund_eligible());
}

#[test]
fn test_is_refund_eligible_verified_deposit() {
    let mut a = make_attendee("In-Person");
    a.deposit_verified = Some("true".to_string());
    assert!(a.is_refund_eligible());
}

#[test]
fn test_is_refund_eligible_already_refunded() {
    let mut a = make_attendee("In-Person");
    a.deposit_verified = Some("true".to_string());
    a.refund_status = Some("refunded".to_string());
    assert!(!a.is_refund_eligible());
}

#[test]
fn test_is_refund_eligible_pending_refund() {
    let mut a = make_attendee("In-Person");
    a.deposit_verified = Some("true".to_string());
    a.refund_status = Some("pending".to_string());
    assert!(a.is_refund_eligible());
}

#[test]
fn test_check_in_error_display() {
    assert!(
        CheckInError::NotApproved("pending".to_string())
            .to_string()
            .contains("pending")
    );
    assert!(
        CheckInError::AlreadyCheckedIn("2025-01-01".to_string())
            .to_string()
            .contains("2025-01-01")
    );
    assert!(CheckInError::OnlineAttendee.to_string().contains("online"));
}

// -----------------------------------------------------------------------
// is_in_person tests
// -----------------------------------------------------------------------

#[test]
fn test_is_in_person_exact() {
    assert!(make_attendee("In-Person").is_in_person());
}

#[test]
fn test_is_in_person_case_insensitive() {
    assert!(make_attendee("in-person").is_in_person());
    assert!(make_attendee("IN-PERSON").is_in_person());
    assert!(make_attendee("In-person").is_in_person());
}

#[test]
fn test_is_in_person_with_spaces() {
    assert!(make_attendee("In Person").is_in_person());
    assert!(make_attendee("in person").is_in_person());
    assert!(make_attendee("IN PERSON").is_in_person());
}

#[test]
fn test_is_in_person_long_value() {
    assert!(make_attendee("In-Person (Physical Attendance)").is_in_person());
    assert!(make_attendee("In-Person - On Site").is_in_person());
    assert!(make_attendee("  In-Person  ").is_in_person());
    assert!(make_attendee("In Person Participant").is_in_person());
}

#[test]
fn test_is_not_in_person_online() {
    assert!(!make_attendee("Online").is_in_person());
    assert!(!make_attendee("online").is_in_person());
    assert!(!make_attendee("ONLINE").is_in_person());
}

#[test]
fn test_is_not_in_person_virtual() {
    assert!(!make_attendee("Virtual").is_in_person());
    assert!(!make_attendee("Hybrid").is_in_person());
}

#[test]
fn test_is_in_person_empty_defaults_true() {
    // Empty participation_type defaults to in-person (legacy events)
    assert!(make_attendee("").is_in_person());
    assert!(make_attendee("   ").is_in_person());
}

#[test]
fn test_is_not_in_person_other() {
    assert!(!make_attendee("Unknown").is_in_person());
    assert!(!make_attendee("TBD").is_in_person());
}

// -----------------------------------------------------------------------
// ParticipationType canonicalization
// -----------------------------------------------------------------------

#[test]
fn test_participation_type_parse_prod_variants() {
    // In-person variants observed in prod
    assert_eq!(
        ParticipationType::parse("In-Person"),
        ParticipationType::InPerson
    );
    assert_eq!(
        ParticipationType::parse("in_person"),
        ParticipationType::InPerson
    );
    assert_eq!(
        ParticipationType::parse("in person"),
        ParticipationType::InPerson
    );
    assert_eq!(
        ParticipationType::parse("IN-PERSON (PHYSICAL)"),
        ParticipationType::InPerson
    );
    assert_eq!(
        ParticipationType::parse("physical attendance"),
        ParticipationType::InPerson
    );
    // Online variants observed in prod
    assert_eq!(
        ParticipationType::parse("Online"),
        ParticipationType::Online
    );
    assert_eq!(
        ParticipationType::parse("online"),
        ParticipationType::Online
    );
    assert_eq!(
        ParticipationType::parse("Virtual"),
        ParticipationType::Online
    );
    assert_eq!(
        ParticipationType::parse("retrospective"),
        ParticipationType::Retrospective
    );
    // Empty defaults to in-person (legacy)
    assert_eq!(ParticipationType::parse(""), ParticipationType::InPerson);
    assert_eq!(ParticipationType::parse("   "), ParticipationType::InPerson);
    // Unrecognized prod junk → Other
    assert_eq!(ParticipationType::parse("test"), ParticipationType::Other);
    assert_eq!(ParticipationType::parse("TBD"), ParticipationType::Other);
}

#[test]
fn test_participation_type_as_str_and_default() {
    assert_eq!(ParticipationType::InPerson.as_str(), "in_person");
    assert_eq!(ParticipationType::Online.as_str(), "online");
    assert_eq!(ParticipationType::Retrospective.as_str(), "retrospective");
    assert_eq!(ParticipationType::Other.as_str(), "other");
    assert_eq!(ParticipationType::default(), ParticipationType::InPerson);
    // is_in_person now delegates to the typed enum (behavior preserved)
    assert!(make_attendee("In-Person").is_in_person());
    assert!(!make_attendee("Online").is_in_person());
    assert!(!make_attendee("test").is_in_person());
    assert!(make_attendee("").is_in_person());
}

#[test]
fn test_participation_type_display() {
    // display() is the inverse of parse() for the two participation modes
    // and is what gets written to the Google Sheet (organizer-facing).
    assert_eq!(ParticipationType::InPerson.display(), "In-Person");
    assert_eq!(ParticipationType::Online.display(), "Online");
    assert_eq!(ParticipationType::Retrospective.display(), "Retrospective");
    assert_eq!(ParticipationType::Other.display(), "Other");
    // round-trip: canonical → display → parse → same variant
    for v in ["In-Person", "in_person", "in person", "physical", ""] {
        assert_eq!(
            ParticipationType::parse(ParticipationType::parse(v).display()),
            ParticipationType::parse(v),
            "display round-trip failed for '{v}'"
        );
    }
    for v in ["Online", "online", "Virtual"] {
        assert_eq!(
            ParticipationType::parse(ParticipationType::parse(v).display()),
            ParticipationType::parse(v),
            "display round-trip failed for '{v}'"
        );
    }
}

// -----------------------------------------------------------------------
// ColumnMapping tests
// -----------------------------------------------------------------------

#[test]
fn test_column_mapping_hardcoded() {
    let mapping = ColumnMapping::hardcoded();
    // Section 1: Identity
    assert_eq!(mapping.get(ColumnKey::ApiId), Some(0)); // A
    assert_eq!(mapping.get(ColumnKey::Email), Some(4)); // E
    // Section 2: Registration
    assert_eq!(mapping.get(ColumnKey::ParticipationType), Some(8)); // I
    // Section 3: Contact
    assert_eq!(mapping.get(ColumnKey::Phone), Some(9)); // J
    assert_eq!(mapping.get(ColumnKey::ContactChannel), Some(10)); // K
    assert_eq!(mapping.get(ColumnKey::ContactHandle), Some(11)); // L
    // Section 4: Deposit
    assert_eq!(mapping.get(ColumnKey::DepositAgreed), Some(12)); // M
    assert_eq!(mapping.get(ColumnKey::DepositMethod), Some(13)); // N
    assert_eq!(mapping.get(ColumnKey::DepositTxSignature), Some(15)); // P
    assert_eq!(mapping.get(ColumnKey::DepositVerified), Some(16)); // Q
    // Section 5: Lifecycle
    assert_eq!(mapping.get(ColumnKey::CheckedInAt), Some(17)); // R
    assert_eq!(mapping.get(ColumnKey::SolanaAddress), Some(19)); // T
    assert_eq!(mapping.get(ColumnKey::ClaimToken), Some(21)); // V
    // Section 6: Bank & Refund
    assert_eq!(mapping.get(ColumnKey::NftProofUrl), Some(23)); // X
    assert_eq!(mapping.get(ColumnKey::BankAccount), Some(24)); // Y
    assert_eq!(mapping.get(ColumnKey::BankName), Some(25)); // Z
    assert_eq!(mapping.get(ColumnKey::AccountName), Some(26)); // AA
    assert_eq!(mapping.get(ColumnKey::RefundStatus), Some(27)); // AB
    assert_eq!(mapping.get(ColumnKey::RefundLink), Some(28)); // AC
    assert_eq!(mapping.get(ColumnKey::SendEmailStatus), Some(29)); // AD
    // Section 7: Consent & Compliance
    assert_eq!(mapping.get(ColumnKey::ConsentGiven), Some(30)); // AE
    assert_eq!(mapping.get(ColumnKey::PhotoConsent), Some(31)); // AF
    assert_eq!(mapping.get(ColumnKey::ConsentMarketing), Some(32)); // AG
    assert_eq!(mapping.total_columns, 33);
}

#[test]
fn test_column_mapping_from_headers_exact() {
    let headers: Vec<String> = vec![
        "api_id".into(),
        "name".into(),
        "first_name".into(),
        "last_name".into(),
        "email".into(),
        "ticket_name".into(),
        "registration_date".into(),
        "approval_status".into(),
        "participation_type".into(),
        "phone".into(),
        "contact_channel".into(),
        "contact_handle".into(),
        "deposit_agreed".into(),
        "deposit_method".into(),
        "deposit_amount".into(),
        "deposit_tx_signature".into(),
        "deposit_verified".into(),
        "checked_in_at".into(),
        "checked_in_by".into(),
        "solana_address".into(),
        "qr_code_url".into(),
        "claim_token".into(),
        "claimed_at".into(),
    ];
    let mapping = ColumnMapping::from_headers(&headers);
    assert!(mapping.is_valid());
    assert_eq!(mapping.get(ColumnKey::ApiId), Some(0));
    assert_eq!(mapping.get(ColumnKey::Email), Some(4));
    assert_eq!(mapping.get(ColumnKey::ParticipationType), Some(8));
    assert_eq!(mapping.get(ColumnKey::Phone), Some(9));
    assert_eq!(mapping.get(ColumnKey::CheckedInAt), Some(17));
    assert_eq!(mapping.mapped_count(), 23);
}

#[test]
fn test_column_mapping_from_headers_case_insensitive() {
    let headers: Vec<String> = vec![
        "API_ID".into(),
        "Name".into(),
        "FIRSTNAME".into(),
        "Last Name".into(),
        "Email".into(),
        "Status".into(),
        "Checked In At".into(),
    ];
    let mapping = ColumnMapping::from_headers(&headers);
    assert_eq!(mapping.get(ColumnKey::ApiId), Some(0));
    assert_eq!(mapping.get(ColumnKey::FirstName), Some(2));
    assert_eq!(mapping.get(ColumnKey::LastName), Some(3));
    assert_eq!(mapping.get(ColumnKey::CheckedInAt), Some(6));
    assert_eq!(mapping.get(ColumnKey::ApprovalStatus), Some(5));
}

#[test]
fn test_column_mapping_partial_headers() {
    // Only some columns have headers we recognize
    let headers: Vec<String> = vec![
        "api_id".into(),
        "something".into(),
        "email".into(),
        "another".into(),
        "checked_in_at".into(),
    ];
    let mapping = ColumnMapping::from_headers(&headers);
    assert!(mapping.is_valid());
    assert_eq!(mapping.get(ColumnKey::ApiId), Some(0));
    assert_eq!(mapping.get(ColumnKey::Email), Some(2));
    assert_eq!(mapping.get(ColumnKey::CheckedInAt), Some(4));
    // Unmapped columns return None
    assert_eq!(mapping.get(ColumnKey::Name), None);
    assert_eq!(mapping.get(ColumnKey::ParticipationType), None);
}

#[test]
fn test_column_mapping_get_or_default_fallback() {
    // Mapping with only 2 columns mapped
    let headers: Vec<String> = vec!["api_id".into(), "email".into()];
    let mapping = ColumnMapping::from_headers(&headers);
    // Unmapped keys fall back to hardcoded
    assert_eq!(mapping.get_or_default(ColumnKey::ApiId), 0);
    assert_eq!(mapping.get_or_default(ColumnKey::Email), 1);
    assert_eq!(mapping.get_or_default(ColumnKey::CheckedInAt), 17); // hardcoded fallback
    assert_eq!(mapping.get_or_default(ColumnKey::ParticipationType), 8); // hardcoded fallback
}

#[test]
fn test_index_to_column_letter() {
    assert_eq!(index_to_column_letter(0), "A");
    assert_eq!(index_to_column_letter(1), "B");
    assert_eq!(index_to_column_letter(8), "I");
    assert_eq!(index_to_column_letter(24), "Y");
    assert_eq!(index_to_column_letter(25), "Z");
    assert_eq!(index_to_column_letter(26), "AA");
    assert_eq!(index_to_column_letter(27), "AB");
}

#[test]
fn test_column_letter_via_mapping() {
    let mapping = ColumnMapping::hardcoded();
    assert_eq!(mapping.column_letter(ColumnKey::ApiId), "A");
    assert_eq!(mapping.column_letter(ColumnKey::ParticipationType), "I");
    assert_eq!(mapping.column_letter(ColumnKey::CheckedInAt), "R");
    assert_eq!(mapping.column_letter(ColumnKey::SolanaAddress), "T");
    assert_eq!(mapping.column_letter(ColumnKey::ClaimToken), "V");
}

#[test]
fn test_last_column_letter_hardcoded() {
    let mapping = ColumnMapping::hardcoded();
    // 33 columns → last index 32 → "AG"
    assert_eq!(mapping.last_column_letter(), "AG");
}

#[test]
fn test_last_column_letter_from_headers() {
    let headers: Vec<String> = vec![
        "API ID".into(),
        "Name".into(),
        "Email".into(),
        "Approval Status".into(),
        "Participation Type".into(),
    ];
    let mapping = ColumnMapping::from_headers(&headers);
    // 5 columns → last index 4 → "E"
    assert_eq!(mapping.last_column_letter(), "E");
}

#[test]
fn test_last_column_letter_empty() {
    let mapping = ColumnMapping::from_headers(&[]);
    // 0 columns → fallback "Z"
    assert_eq!(mapping.last_column_letter(), "Z");
}

#[test]
fn test_from_sheet_values_with_mapping() {
    let headers: Vec<String> = vec![
        "api_id".into(),               // 0
        "name".into(),                 // 1
        "first_name".into(),           // 2
        "last_name".into(),            // 3
        "email".into(),                // 4
        "ticket_name".into(),          // 5
        "registration_date".into(),    // 6
        "approval_status".into(),      // 7
        "participation_type".into(),   // 8
        "phone".into(),                // 9
        "contact_channel".into(),      // 10
        "contact_handle".into(),       // 11
        "deposit_agreed".into(),       // 12
        "deposit_method".into(),       // 13
        "deposit_amount".into(),       // 14
        "deposit_tx_signature".into(), // 15
        "deposit_verified".into(),     // 16
        "checked_in_at".into(),        // 17
        "checked_in_by".into(),        // 18
        "solana_address".into(),       // 19
        "qr_code_url".into(),          // 20
        "claim_token".into(),          // 21
        "claimed_at".into(),           // 22
    ];
    let mapping = ColumnMapping::from_headers(&headers);

    let data_rows: Vec<Vec<String>> = vec![vec![
        "gst-123".into(),
        "John Doe".into(),
        "John".into(),
        "Doe".into(),
        "john@test.com".into(),
        "VIP".into(),
        "2025-01-01".into(),
        "Approved".into(),
        "In-Person".into(),
        "".into(),
        "Telegram".into(),
        "@johndoe".into(),
        "Yes".into(),
        "".into(),
        "".into(),
        "".into(),
        "".into(),
        "".into(),
        "".into(),
        "".into(),
        "".into(),
        "tok-abc".into(),
        "".into(),
    ]];

    let row = AttendeeRow::from_sheet_values(&data_rows, 2, &mapping).unwrap();
    assert_eq!(row.api_id, "gst-123");
    assert_eq!(row.name, "John Doe");
    assert_eq!(row.first_name, "John");
    assert_eq!(row.last_name, "Doe");
    assert_eq!(row.email, "john@test.com");
    assert_eq!(row.ticket_name, "VIP");
    assert_eq!(row.approval_status, "Approved");
    assert_eq!(row.participation_type, "In-Person");
    assert_eq!(row.contact_channel, Some("Telegram".into()));
    assert_eq!(row.contact_handle, Some("@johndoe".into()));
    assert_eq!(row.deposit_agreed, Some("Yes".into()));
    assert_eq!(row.claim_token, Some("tok-abc".into()));
    assert_eq!(row.checked_in_at, None);
}

#[test]
fn test_from_sheet_values_hardcoded_compat() {
    // 32-column layout using hardcoded mapping
    let mapping = ColumnMapping::hardcoded();

    let mut row_data = vec![String::new(); 32];
    row_data[0] = "gst-legacy".into(); // A: api_id
    row_data[1] = "Jane Smith".into(); // B: name
    row_data[2] = "Jane".into(); // C: first_name
    row_data[3] = "Smith".into(); // D: last_name
    row_data[4] = "jane@legacy.com".into(); // E: email
    row_data[5] = "General".into(); // F: ticket_name
    row_data[7] = "Approved".into(); // H: approval_status
    row_data[8] = "In-Person".into(); // I: participation_type
    row_data[17] = "2025-01-01T00:00:00Z".into(); // R: checked_in_at
    row_data[19] = "7xKXtg2CW87d97TXJSDpbD5jBkheTqA85T".into(); // T: solana_address
    row_data[21] = "tok-legacy".into(); // V: claim_token
    row_data[30] = "Yes".into(); // AE: consent_given
    row_data[31] = "No".into(); // AF: photo_consent

    let data_rows: Vec<Vec<String>> = vec![row_data];

    let row = AttendeeRow::from_sheet_values(&data_rows, 2, &mapping).unwrap();
    assert_eq!(row.api_id, "gst-legacy");
    assert_eq!(row.name, "Jane Smith");
    assert_eq!(row.email, "jane@legacy.com");
    assert_eq!(row.participation_type, "In-Person");
    assert_eq!(row.checked_in_at, Some("2025-01-01T00:00:00Z".into()));
    assert_eq!(
        row.solana_address,
        Some("7xKXtg2CW87d97TXJSDpbD5jBkheTqA85T".into())
    );
    assert_eq!(row.claim_token, Some("tok-legacy".into()));
}

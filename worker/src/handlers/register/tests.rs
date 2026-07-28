use super::my_registration::is_online_participation;
use super::signup::resolve_participation_type;
use event_checkin_domain::models::attendee::ParticipationType;
use event_checkin_domain::models::event::EventFormat;

#[test]
fn is_online_participation_uses_canonical_enum() {
    // resolve_participation_type produces these for the registration path
    assert!(is_online_participation("Online"));
    assert!(is_online_participation("online"));
    assert!(is_online_participation("Virtual"));
    assert!(!is_online_participation("In-Person"));
    assert!(!is_online_participation("in_person"));
    assert!(!is_online_participation("physical"));
    // Empty defaults to in-person (legacy), not online
    assert!(!is_online_participation(""));
    // walk-in sentinel is neither online nor in-person
    assert!(!is_online_participation("walkin"));
}

#[test]
fn resolve_participation_type_defaults_by_format() {
    // Returns canonical snake_case (stored in D1); the Sheet append path
    // converts to display-case via ParticipationType::display().
    assert_eq!(
        resolve_participation_type(&EventFormat::InPerson, None).unwrap(),
        "in_person"
    );
    assert_eq!(
        resolve_participation_type(&EventFormat::Online, None).unwrap(),
        "online"
    );
    // Hybrid honors the user's choice (normalized to canonical), defaulting
    // to in-person when absent. Accepts both display-case and canonical input.
    assert_eq!(
        resolve_participation_type(&EventFormat::Hybrid, Some("Online")).unwrap(),
        "online"
    );
    assert_eq!(
        resolve_participation_type(&EventFormat::Hybrid, Some("In-Person")).unwrap(),
        "in_person"
    );
    assert_eq!(
        resolve_participation_type(&EventFormat::Hybrid, Some("in_person")).unwrap(),
        "in_person"
    );
    assert_eq!(
        resolve_participation_type(&EventFormat::Hybrid, None).unwrap(),
        "in_person"
    );
}

/// enforce_capacity judges the registering attendee via ParticipationType::parse
/// (the same enum as Attendee::is_in_person), so its in-person detection is
/// covered by the domain crate's participation_type tests. This pins the
/// shared contract that both checks now use.
#[test]
fn registering_in_person_detection_matches_canonical_enum() {
    for v in [
        "In-Person",
        "in_person",
        "in person",
        "IN-PERSON",
        "physical",
        "",
    ] {
        assert_eq!(
            ParticipationType::parse(v),
            ParticipationType::InPerson,
            "expected '{v}' to be in-person"
        );
    }
    for v in ["Online", "online", "Virtual"] {
        assert_eq!(
            ParticipationType::parse(v),
            ParticipationType::Online,
            "expected '{v}' to be online"
        );
    }
    // walk-in sentinel stays out of both tracks
    assert_eq!(ParticipationType::parse("walkin"), ParticipationType::Other);
}

/// Cross-cutting invariant: every write path in 3.2 (registration,
/// manual override, Sheet→D1 sync) must produce a canonical value that,
/// when read back through `Attendee::is_in_person()`, gives the expected
/// in-person/online judgment. This is the contract that prevents the
/// pre-3.2 bug class (registration wrote display-case, sync wrote
/// snake_case, reads were inconsistent).
#[test]
fn write_paths_produce_canonical_values_round_tripping_is_in_person() {
    use event_checkin_domain::models::attendee::CheckInStatus;

    // Registration: resolve_participation_type returns canonical
    for (format, choice, expected_in_person) in [
        (EventFormat::InPerson, None, true),
        (EventFormat::Online, None, false),
        (EventFormat::Hybrid, Some("Online"), false),
        (EventFormat::Hybrid, Some("in_person"), true),
        (EventFormat::Hybrid, None, true),
    ] {
        let stored = resolve_participation_type(&format, choice).unwrap();
        // The canonical value, read back as an Attendee, must match intent
        let attendee = make_minimal_attendee(&stored, CheckInStatus::Approved);
        assert_eq!(
            attendee.is_in_person(),
            expected_in_person,
            "registration stored '{stored}' for {format:?}/choice={choice:?}"
        );
    }
}

/// Minimal Attendee constructor for write-path round-trip tests.
fn make_minimal_attendee(
    participation_type: &str,
    status: event_checkin_domain::models::attendee::CheckInStatus,
) -> event_checkin_domain::models::attendee::Attendee {
    use event_checkin_domain::models::attendee::Attendee;
    Attendee {
        api_id: "test".to_string(),
        first_name: String::new(),
        last_name: String::new(),
        name: "Test".to_string(),
        email: "test@test.com".to_string(),
        ticket_name: String::new(),
        approval_status: status,
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
        row_index: 0,
    }
}

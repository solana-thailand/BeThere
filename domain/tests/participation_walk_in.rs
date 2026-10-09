//! .issues/162: a staff walk-in (`participation_type = 'walkin'`) is in the
//! room. It used to parse as `Other`, so every reader of
//! `Attendee::is_in_person()` treated it as online: the ticket JSON, the
//! ticket note, and the claim path's online timing gate on the Sheets
//! fallback. Capacity alone had a special case (.issues/157).

use event_checkin_domain::models::attendee::{
    Attendee, CheckInStatus, PARTICIPATION_WALK_IN, ParticipationType, TrackCounts,
};

fn attendee(participation_type: &str) -> Attendee {
    Attendee {
        api_id: "att-1".to_string(),
        first_name: "Ada".to_string(),
        last_name: "Lovelace".to_string(),
        name: "Ada Lovelace".to_string(),
        email: "ada@example.com".to_string(),
        ticket_name: "Online".to_string(),
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

#[test]
fn the_stored_sentinel_parses_as_walk_in() {
    assert_eq!(
        ParticipationType::parse(PARTICIPATION_WALK_IN),
        ParticipationType::WalkIn
    );
}

#[test]
fn walk_in_is_in_person_and_never_online() {
    let kind = ParticipationType::WalkIn;
    assert!(kind.is_in_person());
    assert!(!kind.counts_toward_online_track());
    assert!(attendee(PARTICIPATION_WALK_IN).is_in_person());
}

#[test]
fn walk_in_round_trips_to_the_stored_sentinel() {
    // Any path that canonicalizes through the enum must write `walkin` back,
    // or the claim path, the delete path and the duplicate check lose it.
    assert_eq!(ParticipationType::WalkIn.as_str(), PARTICIPATION_WALK_IN);
    assert_eq!(
        ParticipationType::parse(ParticipationType::WalkIn.as_str()),
        ParticipationType::WalkIn
    );
    assert_eq!(
        serde_json::to_string(&ParticipationType::WalkIn).unwrap(),
        "\"walkin\""
    );
    assert_eq!(
        serde_json::from_str::<ParticipationType>("\"walkin\"").unwrap(),
        ParticipationType::WalkIn
    );
}

#[test]
fn a_walk_in_passes_the_in_person_check_in_gate() {
    assert_eq!(attendee(PARTICIPATION_WALK_IN).can_check_in(), Ok(()));
}

#[test]
fn capacity_needs_no_special_case() {
    assert_eq!(
        TrackCounts::from_participation_types([PARTICIPATION_WALK_IN, "online"]),
        TrackCounts {
            in_person: 1,
            online: 1
        }
    );
}

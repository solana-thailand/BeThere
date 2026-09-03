//! `Attendee::can_check_in_virtually` — the gate for the online/virtual
//! check-in paths.
//!
//! Three worker paths flip an attendee to checked-in: the staff scan
//! (`handlers/checkin.rs`), the self-serve adventure quest completion
//! (`handlers/adventure.rs::quest_complete_checkin`) and the claim mint's
//! auto virtual check-in (`claim/mint/execute.rs`).
//!
//! The claim mint path never re-checks approval — it treats a set
//! `checked_in_at` as proof that some approval-gated path produced it. The
//! quest-complete endpoint used to flip `checked_in_at` without that gate,
//! so a pending/invited registrant could check themselves in and inherit the
//! trust. These tests pin the domain rule both handlers now share.

use event_checkin_domain::models::attendee::{Attendee, CheckInError, CheckInStatus};

fn attendee(approval_status: CheckInStatus, checked_in_at: Option<&str>) -> Attendee {
    Attendee {
        api_id: "att-1".to_string(),
        first_name: "Ada".to_string(),
        last_name: "Lovelace".to_string(),
        name: "Ada Lovelace".to_string(),
        email: "ada@example.com".to_string(),
        ticket_name: "Online".to_string(),
        approval_status,
        participation_type: "Online".to_string(),
        registration_date: None,
        phone: None,
        contact_channel: None,
        contact_handle: None,
        deposit_agreed: None,
        deposit_method: None,
        deposit_amount: None,
        deposit_tx_signature: None,
        deposit_verified: None,
        checked_in_at: checked_in_at.map(str::to_string),
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
fn approved_online_attendee_may_check_in_virtually() {
    assert_eq!(
        attendee(CheckInStatus::Approved, None).can_check_in_virtually(),
        Ok(())
    );
}

#[test]
fn unapproved_attendee_may_not_check_in_virtually() {
    // The regression this closes: a pending registrant completing the adventure
    // would otherwise set their own `checked_in_at`, which the claim mint path
    // reads as "an approval-gated path let them through".
    for status in [CheckInStatus::PendingApproval, CheckInStatus::Invited] {
        assert_eq!(
            attendee(status, None).can_check_in_virtually(),
            Err(CheckInError::NotApproved(status.to_string())),
            "{status:?} must not be able to check in virtually"
        );
    }
}

#[test]
fn already_checked_in_is_rejected_before_the_approval_check() {
    // Order matters: an already-checked-in attendee reports as such rather than
    // as unapproved, so the quest-complete endpoint's idempotent branch and this
    // gate never disagree about which condition fired.
    assert_eq!(
        attendee(CheckInStatus::PendingApproval, Some("2026-01-01T00:00:00Z"))
            .can_check_in_virtually(),
        Err(CheckInError::AlreadyCheckedIn(
            "2026-01-01T00:00:00Z".to_string()
        ))
    );
}

#[test]
fn participation_type_is_not_a_gate_on_the_virtual_path() {
    // `can_check_in` rejects online attendees (they must not be scanned on-site);
    // the virtual path inverts that, so an in-person participation type must not
    // be refused here — hybrid events check people in on either track.
    let mut in_person = attendee(CheckInStatus::Approved, None);
    in_person.participation_type = "In-Person".to_string();
    assert_eq!(in_person.can_check_in_virtually(), Ok(()));
    assert_eq!(in_person.can_check_in(), Ok(()));
}

#[test]
fn the_virtual_gate_is_the_on_site_gate_minus_the_in_person_rule() {
    // Pins the relationship rather than the implementation: for every attendee
    // that `can_check_in` accepts, `can_check_in_virtually` must accept too.
    // If someone adds a rule to one, this fails until they decide about the other.
    for status in [
        CheckInStatus::Approved,
        CheckInStatus::CheckedIn,
        CheckInStatus::PendingApproval,
        CheckInStatus::Invited,
    ] {
        for checked_in in [None, Some("2026-01-01T00:00:00Z")] {
            let mut a = attendee(status, checked_in);
            a.participation_type = "In-Person".to_string();
            if a.can_check_in().is_ok() {
                assert_eq!(
                    a.can_check_in_virtually(),
                    Ok(()),
                    "{status:?}/{checked_in:?} passes the on-site gate but not the virtual one"
                );
            }
        }
    }
}

//! The next-step rule (.issues/174): a free event never sends an in-person
//! attendee to the deposit page.

use event_checkin_domain::models::next_step::{NextStepFacts, NextStepKind, next_step_kind};

const IN_PERSON_UNPAID: NextStepFacts = NextStepFacts {
    is_claimed: false,
    is_checked_in: false,
    has_claim_token: true,
    is_online_participant: false,
    format_has_in_person: true,
    deposit_required: true,
    real_deposit: false,
};

#[test]
fn free_event_goes_to_the_ticket_not_the_deposit_page() {
    let free = NextStepFacts {
        deposit_required: false,
        ..IN_PERSON_UNPAID
    };
    assert_eq!(next_step_kind(free), NextStepKind::Ticket);
    // Negative control: the same attendee on a deposit event must pay first.
    assert_eq!(next_step_kind(IN_PERSON_UNPAID), NextStepKind::Deposit);
}

#[test]
fn a_real_deposit_unlocks_the_ticket() {
    let paid = NextStepFacts {
        real_deposit: true,
        ..IN_PERSON_UNPAID
    };
    assert_eq!(next_step_kind(paid), NextStepKind::Ticket);
}

#[test]
fn claim_and_check_in_come_before_the_deposit_question() {
    let claimed = NextStepFacts {
        is_claimed: true,
        ..IN_PERSON_UNPAID
    };
    assert_eq!(next_step_kind(claimed), NextStepKind::Ticket);
    let checked_in = NextStepFacts {
        is_checked_in: true,
        ..IN_PERSON_UNPAID
    };
    assert_eq!(next_step_kind(checked_in), NextStepKind::Claim);
    // Checked in without a claim token has nowhere to claim: back to the rules.
    let no_token = NextStepFacts {
        is_checked_in: true,
        has_claim_token: false,
        ..IN_PERSON_UNPAID
    };
    assert_eq!(next_step_kind(no_token), NextStepKind::Deposit);
}

#[test]
fn online_attendees_and_online_only_events_wait_on_the_ticket() {
    let online = NextStepFacts {
        is_online_participant: true,
        ..IN_PERSON_UNPAID
    };
    assert_eq!(next_step_kind(online), NextStepKind::Waiting);
    let online_event = NextStepFacts {
        format_has_in_person: false,
        ..IN_PERSON_UNPAID
    };
    assert_eq!(next_step_kind(online_event), NextStepKind::Waiting);
}

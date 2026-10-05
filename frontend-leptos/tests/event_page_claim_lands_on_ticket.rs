//! A registered attendee whose next step is the claim lands on the ticket.

use event_checkin_frontend::pages::public_event::registered_state::{landing_step, ticket_url};

#[test]
fn a_ready_claim_lands_on_the_ticket() {
    assert_eq!(landing_step("claim"), "ticket");
}

#[test]
fn other_steps_are_unchanged() {
    for step in ["deposit", "ticket", "waiting", "register"] {
        assert_eq!(landing_step(step), step);
    }
}

#[test]
fn the_ticket_url_names_the_event() {
    assert_eq!(ticket_url("att-1", "evt-9"), "/ticket/att-1?event_id=evt-9");
}

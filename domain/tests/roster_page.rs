//! `roster_page` — paging the approved roster behind `GET /api/attendees`
//! (`.issues/151` C). D1-created attendees all carry `row_index` 0, so the
//! order must not rely on it being unique.

use std::collections::HashSet;

use event_checkin_domain::models::attendee::{
    Attendee, CheckInStatus, ROSTER_PAGE_MAX, roster_page,
};

fn attendee(id: &str, row_index: usize, approval_status: CheckInStatus) -> Attendee {
    Attendee {
        api_id: id.to_string(),
        first_name: String::new(),
        last_name: String::new(),
        name: format!("Name {id}"),
        email: format!("{id}@example.com"),
        ticket_name: String::new(),
        approval_status,
        participation_type: "general".to_string(),
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
        checked_in_by: Some("staff@example.com".to_string()),
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
        row_index,
    }
}

/// Every page from the start, following `next_cursor` until it runs out.
fn walk(attendees: &[Attendee], limit: usize) -> Vec<String> {
    let mut ids = Vec::new();
    let mut cursor = None;
    loop {
        let page = roster_page(attendees, cursor, limit);
        ids.extend(page.items.iter().map(|a| a.api_id.clone()));
        match page.next_cursor {
            Some(next) => {
                assert!(next > cursor.unwrap_or(0), "cursor must advance");
                cursor = Some(next);
            }
            None => return ids,
        }
    }
}

#[test]
fn every_approved_attendee_is_reachable_when_row_index_is_all_zero() {
    let attendees: Vec<Attendee> = (0..450)
        .map(|i| attendee(&format!("a{i:03}"), 0, CheckInStatus::Approved))
        .collect();

    let first = roster_page(&attendees, None, ROSTER_PAGE_MAX);
    assert_eq!(first.items.len(), ROSTER_PAGE_MAX);
    assert_eq!(first.next_cursor, Some(ROSTER_PAGE_MAX));

    let ids = walk(&attendees, ROSTER_PAGE_MAX);
    assert_eq!(ids.len(), 450);
    assert_eq!(
        ids.iter().collect::<HashSet<_>>().len(),
        450,
        "no duplicates"
    );
}

#[test]
fn order_is_row_index_then_api_id() {
    let attendees = vec![
        attendee("c", 0, CheckInStatus::Approved),
        attendee("b", 5, CheckInStatus::Approved),
        attendee("a", 5, CheckInStatus::CheckedIn),
        attendee("d", 0, CheckInStatus::Approved),
    ];
    assert_eq!(walk(&attendees, 1), ["c", "d", "a", "b"]);
}

#[test]
fn only_approved_and_checked_in_are_listed() {
    let attendees = vec![
        attendee("ok", 0, CheckInStatus::Approved),
        attendee("in", 0, CheckInStatus::CheckedIn),
        attendee("wait", 0, CheckInStatus::PendingApproval),
        attendee("inv", 0, CheckInStatus::Invited),
    ];
    assert_eq!(walk(&attendees, ROSTER_PAGE_MAX), ["in", "ok"]);
}

#[test]
fn exact_fit_has_no_next_page() {
    let attendees: Vec<Attendee> = (0..ROSTER_PAGE_MAX)
        .map(|i| attendee(&format!("a{i:03}"), 0, CheckInStatus::Approved))
        .collect();
    let page = roster_page(&attendees, None, ROSTER_PAGE_MAX);
    assert_eq!(page.items.len(), ROSTER_PAGE_MAX);
    assert_eq!(page.next_cursor, None);
}

#[test]
fn limit_is_clamped_both_ways() {
    let attendees: Vec<Attendee> = (0..300)
        .map(|i| attendee(&format!("a{i:03}"), 0, CheckInStatus::Approved))
        .collect();
    let zero = roster_page(&attendees, None, 0);
    assert_eq!(zero.items.len(), 1);
    assert_eq!(zero.next_cursor, Some(1));

    let huge = roster_page(&attendees, None, usize::MAX);
    assert_eq!(huge.items.len(), ROSTER_PAGE_MAX);
}

#[test]
fn cursor_past_the_end_is_an_empty_last_page() {
    let attendees = vec![attendee("a", 0, CheckInStatus::Approved)];
    let page = roster_page(&attendees, Some(usize::MAX), ROSTER_PAGE_MAX);
    assert!(page.items.is_empty());
    assert_eq!(page.next_cursor, None);
}

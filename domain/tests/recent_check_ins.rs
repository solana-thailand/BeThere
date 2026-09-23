//! `recent_check_ins` — the bounded recent list on the admin roster
//! (plan 028 W9). The dashboard filters it by tab and takes
//! `RECENT_CHECK_INS_PER_TYPE`, so the bound must keep every tab's newest.

use event_checkin_domain::models::attendee::{
    Attendee, CheckInStatus, RECENT_CHECK_INS_PER_TYPE, recent_check_ins,
};

fn attendee(id: &str, participation_type: &str, checked_in_at: Option<&str>) -> Attendee {
    Attendee {
        api_id: id.to_string(),
        first_name: String::new(),
        last_name: String::new(),
        name: format!("Name {id}"),
        email: format!("{id}@example.com"),
        ticket_name: String::new(),
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
        checked_in_at: checked_in_at.map(str::to_string),
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
        row_index: 2,
    }
}

/// `n` check-ins of one type at 10:00:00, 10:00:01, … (older ids first).
fn checked_in(prefix: &str, participation_type: &str, n: usize, minute: u32) -> Vec<Attendee> {
    (0..n)
        .map(|i| {
            let ts = format!("2026-09-24T10:{minute:02}:{i:02}+00:00");
            attendee(&format!("{prefix}{i}"), participation_type, Some(&ts))
        })
        .collect()
}

fn ids(list: &[event_checkin_domain::models::api::RecentCheckIn]) -> Vec<&str> {
    list.iter().map(|c| c.api_id.as_str()).collect()
}

#[test]
fn a_busy_type_does_not_crowd_out_an_older_quieter_one() {
    // 3 online check-ins at 10:00, then 15 in person at 10:05. A single
    // overall cap of 10 would return in-person only and empty the Online tab.
    let mut all = checked_in("on", "Online", 3, 0);
    all.extend(checked_in("ip", "In-Person", 15, 5));

    let recent = recent_check_ins(&all, RECENT_CHECK_INS_PER_TYPE);

    assert_eq!(recent.len(), 13);
    let online: Vec<_> = recent
        .iter()
        .filter(|c| c.api_id.starts_with("on"))
        .collect();
    assert_eq!(online.len(), 3, "every online check-in must survive");
}

#[test]
fn each_type_keeps_its_newest_and_the_list_is_newest_first() {
    let all = checked_in("ip", "In-Person", 15, 5);

    let recent = recent_check_ins(&all, RECENT_CHECK_INS_PER_TYPE);

    // ip14 is the newest (10:05:14); the 10 kept are ip14 … ip5.
    let expected: Vec<String> = (5..15).rev().map(|i| format!("ip{i}")).collect();
    assert_eq!(ids(&recent), expected);
}

#[test]
fn under_the_cap_every_check_in_is_returned() {
    let mut all = checked_in("on", "Online", 4, 0);
    all.extend(checked_in("ip", "In-Person", 6, 5));

    assert_eq!(recent_check_ins(&all, RECENT_CHECK_INS_PER_TYPE).len(), 10);
}

#[test]
fn attendees_not_checked_in_are_left_out() {
    let all = vec![
        attendee("in", "In-Person", Some("2026-09-24T10:00:00+00:00")),
        attendee("out", "In-Person", None),
    ];

    assert_eq!(
        ids(&recent_check_ins(&all, RECENT_CHECK_INS_PER_TYPE)),
        ["in"]
    );
}

#[test]
fn an_unrankable_timestamp_is_kept_after_the_ranked_ones() {
    // Not RFC 3339: the browser's Date.parse may still rank it, so the bound
    // must not be what hides it.
    let mut all = checked_in("ip", "In-Person", 12, 5);
    all.push(attendee("odd", "In-Person", Some("24/09/2026 11:00")));

    let recent = recent_check_ins(&all, RECENT_CHECK_INS_PER_TYPE);

    assert_eq!(recent.len(), 11);
    assert_eq!(recent.last().map(|c| c.api_id.as_str()), Some("odd"));
    assert_eq!(recent[0].checked_in_at, "2026-09-24T10:05:11+00:00");
    assert_eq!(
        recent[0].checked_in_by.as_deref(),
        Some("staff@example.com")
    );
}

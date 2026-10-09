//! The "Paid out" list (`credit_payout_history`): who may see a recorded
//! payout, what a row shows, and a slip link that is actually served.

use std::collections::BTreeSet;

use event_checkin_domain::models::credit_payout::PayoutScope;
use event_checkin_worker::credit_payout_history::{record_of, visible};
use event_checkin_worker::storage::{
    credit_payout_key, credit_payout_url, servable_credit_payout_url,
};
use serde_json::json;

fn orgs(ids: &[&str]) -> PayoutScope {
    PayoutScope::Orgs(ids.iter().map(|s| s.to_string()).collect::<BTreeSet<_>>())
}

#[test]
fn super_admin_sees_every_payout_even_without_org_data() {
    assert!(visible(&PayoutScope::All, None));
    assert!(visible(&PayoutScope::All, Some(&json!({"thb": 500}))));
}

#[test]
fn recorded_organizations_must_all_be_in_scope() {
    let meta = json!({"organizations": ["", "org-b"]});
    assert!(visible(&orgs(&["", "org-b", "org-c"]), Some(&meta)));
    assert!(
        !visible(&orgs(&[""]), Some(&meta)),
        "credit that was partly another org's is not this organizer's to see"
    );
    assert!(
        !visible(&orgs(&[""]), Some(&json!({"organizations": []}))),
        "an empty list covers nothing"
    );
}

#[test]
fn older_entries_fall_back_to_the_slip_path_org() {
    let default_org =
        json!({"proof": "/api/credit-payouts/default/6dbfacd27354e670/1791530514759"});
    assert!(
        visible(&orgs(&[""]), Some(&default_org)),
        "`default` is the empty-id org"
    );
    assert!(!visible(&orgs(&["org-b"]), Some(&default_org)));
    let named = json!({"proof": "/api/storage/credit-payouts/org_a____x/abc/1"});
    assert!(
        visible(&orgs(&["Org A/../x"]), Some(&named)),
        "segments compare sanitized"
    );
}

#[test]
fn entries_with_no_org_data_are_super_admin_only() {
    assert!(!visible(&orgs(&[""]), None));
    assert!(!visible(
        &orgs(&[""]),
        Some(&json!({"thb": 500, "proof": null}))
    ));
}

#[test]
fn a_row_shows_amounts_people_initiator_and_a_servable_slip() {
    let meta = json!({
        "thb": 1000, "usdc": 0, "initiated_by": "organizer",
        "proof": "/api/credit-payouts/default/6dbfacd27354e670/1791530514759",
    });
    let row = record_of(
        "2026-10-09 07:21:55",
        "staff@example.com",
        "person@example.com",
        Some(&meta),
    );
    assert_eq!(row.paid_at, "2026-10-09 07:21:55");
    assert_eq!(row.paid_by, "staff@example.com");
    assert_eq!(row.contact, "person@example.com");
    assert_eq!((row.thb, row.usdc), (1000, 0));
    assert_eq!(row.initiated_by, "organizer");
    assert_eq!(
        row.proof_url.as_deref(),
        Some("/api/storage/credit-payouts/default/6dbfacd27354e670/1791530514759"),
        "the legacy path (404) is rewritten to the served one"
    );
    let asked = record_of("t", "a", "b", Some(&json!({"thb": 5})));
    assert_eq!(
        asked.initiated_by, "attendee",
        "entries before .issues/192 were all requests"
    );
    assert_eq!(asked.proof_url, None);
}

#[test]
fn the_slip_url_a_payout_records_is_the_route_that_serves_it() {
    let key = credit_payout_key("", "person@example.com", "2026-10-09 07:21:55");
    let url = credit_payout_url(&key);
    assert!(
        url.starts_with("/api/storage/credit-payouts/default/"),
        "{url}"
    );
    let router =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/handlers/mod.rs"))
            .expect("read router");
    assert!(
        router.contains("\"/storage/credit-payouts/{org}/{owner}/{request}\""),
        "the router no longer serves the path payouts record"
    );
    assert_eq!(
        servable_credit_payout_url(&url),
        url,
        "a current path is left alone"
    );
    assert_eq!(
        servable_credit_payout_url("/api/storage/slips/e/a"),
        "/api/storage/slips/e/a"
    );
}

//! Plan 025 §7.9: the roster's possible-duplicate badge says with whom and
//! why, one line per match, in the order the worker sent them.

use event_checkin_domain::models::attendee::{DuplicateMatch, DuplicateReason};
use event_checkin_frontend::pages::admin_duplicate_hint::duplicate_hint_title;

fn m(name: &str, reason: DuplicateReason) -> DuplicateMatch {
    DuplicateMatch {
        api_id: format!("id-{name}"),
        name: name.to_string(),
        reason,
    }
}

#[test]
fn each_match_names_the_other_row_and_the_reason() {
    let title = duplicate_hint_title(&[
        m("Somchai J.", DuplicateReason::SameName),
        m("Somchai J.", DuplicateReason::SameWallet),
        m("Suda", DuplicateReason::SameHandle),
    ]);
    assert_eq!(
        title,
        "same name as Somchai J.\nsame wallet as Somchai J.\nsame contact handle as Suda"
    );
}

#[test]
fn the_wire_field_defaults_to_empty() {
    let item: event_checkin_frontend::api::AttendeeListItem =
        serde_json::from_str(r#"{"api_id":"a","name":"A","email":"a@x.com"}"#).expect("parse");
    assert!(item.possible_duplicates.is_empty());
}

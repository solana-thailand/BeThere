//! Source guards for one-click registration (.plans/047).
//!
//! `GET /api/my-profile` now returns the name and contact from `contacts`.
//! That table holds every attendee's contact details, so the read must be
//! keyed on the caller's own verified email and nothing else.

const PROFILE: &str = include_str!("../src/handlers/profile.rs");
const DEVELOPERS: &str = include_str!("../src/db/developers.rs");
const POST_EVENT: &str = include_str!("../src/handlers/register/post_event.rs");

#[test]
fn the_contact_prefill_reads_only_the_callers_own_row() {
    assert!(
        PROFILE.contains("get_contact_card(d1, &claims.email)"),
        "the prefill must be looked up by the signed-in email"
    );
    assert_eq!(
        PROFILE.matches("get_contact_card(").count(),
        1,
        "one lookup, in GET only"
    );
}

#[test]
fn the_contact_card_query_is_a_single_bound_email_row() {
    let body = DEVELOPERS
        .split("pub(crate) async fn get_contact_card(")
        .nth(1)
        .and_then(|s| s.split("\n}\n").next())
        .expect("get_contact_card body");
    assert!(body.contains("WHERE email = LOWER(?1)"));
    assert!(body.contains("D1Type::Text(email)"));
    assert!(
        !body.contains("format!(\"SELECT"),
        "no interpolated SQL in the prefill read"
    );
}

#[test]
fn a_failed_prefill_read_does_not_fail_the_profile() {
    let get = PROFILE
        .split("pub async fn get_my_profile(")
        .nth(1)
        .and_then(|s| s.split("pub async fn update_my_profile(").next())
        .expect("get_my_profile body");
    let lookup = get.split("get_contact_card(").nth(1).expect("the lookup");
    let stmt = lookup.split(';').next().expect("the statement");
    assert!(
        stmt.contains("unwrap_or_else") && !stmt.contains('?'),
        "a contacts read error must degrade to no prefill, not a 500"
    );
}

#[test]
fn post_event_registration_names_the_event_for_the_ticket() {
    assert!(
        POST_EVENT.contains("\"event_id\": event_id"),
        "the page opens /ticket/{{id}}?event_id=…; without event_id the ticket API serves the active event"
    );
}

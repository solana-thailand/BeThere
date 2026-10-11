//! The regular rule (Super GOAT SG1b): the qualifying count folds each
//! registrant's rows with `door_history`, and signup waives only for a proven
//! email, an in-person seat at a deposit event, not already staff, and records
//! the seat exactly as a comp.

use event_checkin_worker::regular::count_qualifying;
use serde_json::json;

fn row(reg: &str, slug: &str, came: bool, by: &str) -> serde_json::Value {
    json!({"reg": reg, "slug": slug, "name": slug, "start_ms": 1, "end_ms": 2,
           "pt": "in_person", "cin": came as i64, "cin_by": by, "crew": 0})
}

#[test]
fn qualifying_count_uses_door_history_per_registrant() {
    let rows = vec![
        // a: 3 doors (one event twice via a linked email counts once)
        row("a@x", "rtm1", true, "door"),
        row("a@x", "rtm1", true, "door"),
        row("a@x", "rtm2", true, "door"),
        row("a@x", "rtm3", true, "door"),
        // b: 2 doors and a virtual one
        row("b@x", "rtm1", true, "door"),
        row("b@x", "rtm2", true, "door"),
        row("b@x", "rtm3", true, "virtual"),
        // c: registered, missed
        row("c@x", "rtm1", false, ""),
    ];
    assert_eq!(count_qualifying(&rows, 3, 1_000), 1);
    assert_eq!(count_qualifying(&rows, 2, 1_000), 2);
    assert_eq!(count_qualifying(&rows, 1, 1_000), 2);
    assert_eq!(count_qualifying(&[], 1, 1_000), 0);
}

#[test]
fn signup_waives_only_where_it_should() {
    let src = include_str!("../src/handlers/register/signup.rs");
    let start = src
        .find("let regular_waived = !staff_waived")
        .expect("regular waiver");
    let block = &src[start..start + 600];
    for needle in [
        "config.deposit_enabled",
        "!is_online_participation(&participation_type)",
        "credit_identity_ok",
        "crate::regular::waives(db, &event_id, &email.to_lowercase())",
    ] {
        assert!(block.contains(needle), "regular waiver lost `{needle}`");
    }
    assert!(src.contains("let deposit_waived = staff_waived || regular_waived;"));
    // the seat is recorded by the comp writer, not a new kind of deposit
    assert!(src.contains("record_staff_comp(&state, &event_id, &api_id, name, &now)"));
    // and no rule means no waiver
    let regular = include_str!("../src/regular.rs");
    assert!(
        regular.contains(
            "let Ok(Some(rule)) = rule(db, event_id).await else {\n        return false;"
        )
    );
}

#[test]
fn rule_routes_are_organizer_and_session_scoped() {
    let handlers = include_str!("../src/handlers/mod.rs");
    let me = handlers
        .find(r#".route("/me/regular/{slug}""#)
        .expect("me route");
    let authed = handlers
        .find(r#".route("/me/record""#)
        .expect("authed anchor");
    assert!(!handlers[authed.min(me)..authed.max(me)].contains("Router::new()"));
    let admin = include_str!("../src/handlers/events/regular_rule.rs");
    assert_eq!(admin.matches("enforce_organizer(").count(), 2);
    assert!(admin.contains("RegularRule::problem"));
}

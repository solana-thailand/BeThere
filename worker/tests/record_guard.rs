//! `GET /api/me/record` (Super GOAT SG1) reads only the session's person:
//! it sits in the authed router (a public route would 500 on Claims) and its
//! handler takes no path or query a caller could point at someone else.

const HANDLERS: &str = include_str!("../src/handlers/mod.rs");
const HANDLER: &str = include_str!("../src/handlers/me_record.rs");

#[test]
fn record_route_is_authed_and_unparameterised() {
    let route = HANDLERS
        .find(r#".route("/me/record""#)
        .expect("route registered");
    let authed = HANDLERS
        .find(r#".route("/courses/{course}/progress""#)
        .expect("authed anchor");
    // both in the same attendee-authed block: nothing between them starts a router
    assert!(!HANDLERS[authed.min(route)..authed.max(route)].contains("Router::new()"));
    assert!(HANDLER.contains("Extension(claims): Extension<Claims>"));
    assert!(!HANDLER.contains("Path(") && !HANDLER.contains("Query("));
    assert!(HANDLER.contains("my_record(db, &claims.email.to_lowercase())"));
}

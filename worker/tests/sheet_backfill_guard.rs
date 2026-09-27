//! `.issues/151` backfill endpoint: it can touch every row of an organizer's
//! sheet, so pin its rails. Which cells it writes is tested in
//! `domain/tests/sheet_backfill.rs`; this pins who can run it and when it
//! writes.

const HANDLER: &str = include_str!("../src/handlers/events/sheet_backfill.rs");
const ROUTES: &str = include_str!("../src/handlers/mod.rs");

#[test]
fn only_a_super_admin_gets_past_the_first_check() {
    let role = HANDLER
        .find("if role != crate::auth::UserRole::SuperAdmin")
        .expect("super-admin check");
    for later in [
        "get_attendees_by_event(",
        "fetch_sheet_range(",
        "batch_update_sheet(",
    ] {
        let at = HANDLER
            .find(later)
            .unwrap_or_else(|| panic!("{later} missing"));
        assert!(role < at, "the role check must come before {later}");
    }
}

#[test]
fn it_writes_only_on_apply_and_only_the_planned_cells() {
    let gate = HANDLER
        .find("if query.apply && !plan.writes.is_empty()")
        .expect("dry run by default");
    let write = HANDLER.find("batch_update_sheet(").unwrap();
    assert!(gate < write, "the write must sit behind the apply gate");
    assert!(
        HANDLER.contains("plan.writes.chunks("),
        "write the plan, nothing else"
    );
    assert_eq!(HANDLER.matches("batch_update_sheet(").count(), 1);
}

#[test]
fn the_route_is_behind_auth() {
    let route = ROUTES
        .find("\"/events/{id}/sheet-backfill\"")
        .expect("route registered");
    let protected = ROUTES
        .find("let protected = Router::new()")
        .expect("protected router");
    let auth = ROUTES[protected..]
        .find("crate::auth::require_auth")
        .map(|i| i + protected)
        .expect("protected router carries require_auth");
    assert!(
        protected < route && route < auth,
        "Extension<Claims> handlers must be in the authed router or they 500"
    );
}

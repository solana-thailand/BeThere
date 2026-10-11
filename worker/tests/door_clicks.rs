//! Door clicks (.plans/045 R4.10): counts only. The table holds no personal
//! data, only the site's own page and door names get in, and the route is
//! public, rate-limited and answers the same either way.

use event_checkin_worker::door_clicks::{DOORS, PAGES, is_valid};

fn read(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn the_table_holds_counts_and_names_only() {
    let sql = read("migrations/0063_door_clicks.sql");
    let create = &sql[sql.find("CREATE TABLE").unwrap()..];
    let cols: Vec<&str> = create
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("PRIMARY") && !l.starts_with(')'))
        .map(|l| l.split_whitespace().next().unwrap())
        .collect();
    assert_eq!(cols, ["day", "page", "door", "n"]);
    for personal in [
        "ip", "email", "user", "agent", "session", "cookie", "token", "id ",
    ] {
        assert!(!create.to_lowercase().contains(personal), "{personal}");
    }
}

#[test]
fn only_the_sites_own_names_get_in() {
    assert!(is_valid("home", "events"));
    assert!(is_valid("events", "try"));
    assert!(!is_valid("home", "home"), "a page has no door to itself");
    assert!(!is_valid("try", "home"), "try is a door, not a page");
    assert!(!is_valid("home", "<script>"));
    assert!(!is_valid("", "events"));
    assert_eq!(PAGES.len() + 1, DOORS.len());
}

#[test]
fn the_route_is_public_rate_limited_and_silent() {
    let routes = read("src/handlers/mod.rs");
    let at = routes.find("\"/public/click\"").unwrap();
    assert!(at < routes.find("Developer profile (attendee-authed").unwrap());
    assert!(read("src/middleware/rate_limit.rs").contains("path == \"/api/public/click\""));
    let handler = read("src/handlers/door_clicks.rs");
    assert_eq!(handler.matches("StatusCode::NO_CONTENT").count(), 3);
    assert!(
        !handler.contains("headers"),
        "nothing about the caller is read"
    );
}

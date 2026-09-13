use std::fs;
use std::path::PathBuf;

fn worker_source(path: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(path)).expect("read worker source")
}

#[test]
fn readiness_query_filters_access_before_returning_aggregates() {
    let sql = worker_source("src/db/sql/quiz_readiness_problems.sql");
    assert!(sql.contains("LOWER(e.organizer_emails)"));
    assert!(sql.contains("LOWER(e.staff_emails)"));
    assert!(sql.contains("SUM(CASE"));
    assert!(!sql.to_lowercase().contains("claim_token"));
    assert!(!sql.to_lowercase().contains("a.email"));
    assert!(!sql.to_lowercase().contains("a.name"));
}

#[test]
fn readiness_route_is_registered_before_the_event_id_route() {
    let routes = worker_source("src/handlers/mod.rs");
    let readiness = routes.find("/events/readiness").expect("readiness route");
    let event_id = routes
        .find("\"/events/{id}\",")
        .expect("generic event id route");
    assert!(readiness < event_id);
}

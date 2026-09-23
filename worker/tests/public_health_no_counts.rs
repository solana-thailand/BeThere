//! Issue 147: the public `/api/health` must not return D1 row counts or scan
//! tables. It used to run six `COUNT(*)` sub-selects per anonymous hit, which
//! disclosed attendee/contact volume and spent the free-plan D1 read quota.

use std::{fs, path::Path};

fn health_code() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers/health.rs");
    let code = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    // Drop `//` comments so prose about the old behaviour cannot trip the guard.
    code.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn public_health_runs_no_count_query() {
    let code = health_code().to_uppercase();
    assert!(
        !code.contains("COUNT("),
        "health.rs scans a table with COUNT("
    );
    assert!(!code.contains("FROM "), "health.rs reads from a table");
}

#[test]
fn public_health_returns_no_counts_field() {
    let code = health_code();
    assert!(
        !code.contains("\"counts\""),
        "health.rs returns a counts field"
    );
    assert!(
        code.contains("SELECT 1"),
        "health.rs lost its connectivity probe"
    );
}

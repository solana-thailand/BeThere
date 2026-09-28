//! Guard: the in-person capacity gates must share one walk-in head-count.
//!
//! Two handlers enforce `in_person_capacity` — public registration and staff
//! walk-in registration. Each once carried its own copy of the count, and both
//! copies swallowed a D1 error into a silent zero, which under-counts the
//! event and admits a registration past the cap. `handlers::capacity` is now
//! the single home for that count and it fails closed.
//!
//! Plan 028 W3 (.issues/157) widened it to the whole per-track count: seven
//! sites fetched every attendee row to count them, and they disagreed about
//! walk-ins. `handlers::capacity::count_tracks` is now the only counter.
//!
//! These are source scans rather than behavioural tests because the helper
//! takes an `AppState` (worker bindings), which cannot be built off-wasm. They
//! catch the failure mode that actually recurs here: a second reader growing
//! its own copy of the rule. A scan cannot prove the helper fails closed for
//! every possible rewrite — it bans the recovery idioms that would turn the
//! error back into a number, which is how the original defect was written.

/// Read a worker source file with `//` comment lines stripped, so prose about
/// the pattern cannot satisfy a rule about code.
fn code_of(relative: &str) -> String {
    let path = format!("{}/src/{relative}", env!("CARGO_MANIFEST_DIR"));
    let source =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path} must be readable: {e}"));
    source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `code` without whitespace, so a rule about a call survives rustfmt line breaks.
fn squash(code: &str) -> String {
    code.split_whitespace().collect()
}

const GATES: [&str; 2] = ["handlers/register/capacity.rs", "handlers/walkin.rs"];

#[test]
fn the_walkin_capacity_count_has_exactly_one_home() {
    assert_eq!(
        code_of("handlers/capacity.rs")
            .matches("count_walkin_attendees(")
            .count(),
        1,
        "`handlers::capacity` must call the D1 count exactly once"
    );

    for gate in GATES {
        assert!(
            !code_of(gate).contains("count_walkin_attendees("),
            "{gate} must not call the D1 walk-in count directly; route it through \
             `handlers::capacity::count_tracks_for_cap` so the fail-closed \
             rule cannot drift per gate"
        );
    }
}

#[test]
fn both_capacity_gates_propagate_the_count_error() {
    for gate in GATES {
        let code = code_of(gate);
        assert!(
            squash(&code).contains("count_tracks_for_cap(state,config,kv).await?"),
            "{gate} must propagate a failed walk-in count with `?`; swallowing it \
             under-counts the event and admits past the cap"
        );
    }
}

#[test]
fn the_count_is_not_swallowed_into_a_silent_zero() {
    let helper = code_of("handlers/capacity.rs");
    for recovery in [
        "unwrap_or(0)",
        "unwrap_or_default()",
        "or_else(",
        "unwrap_or_else(",
    ] {
        assert!(
            !helper.contains(recovery),
            "`{recovery}` in the capacity count degrades a failed query back into a \
             number — zero reads as `plenty of room` and opens the gate"
        );
    }
    assert!(
        helper.contains("AppError::Internal"),
        "the error path must surface as an error, not a log line"
    );

    for gate in GATES {
        assert!(
            !code_of(gate).contains("count for capacity failed, skipping"),
            "{gate} still log-and-skips a failed capacity count"
        );
    }
}

/// Every source file under `src/`, as (path relative to `src/`, code).
fn all_sources() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).expect("src must be readable") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let rel = path
                    .strip_prefix(root)
                    .expect("under src")
                    .to_string_lossy()
                    .into_owned();
                out.push((rel.clone(), code_of(&rel)));
            }
        }
    }
    let root = std::path::PathBuf::from(format!("{}/src", env!("CARGO_MANIFEST_DIR")));
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    out
}

#[test]
fn no_site_counts_a_track_by_filtering_the_attendee_list() {
    let sources = all_sources();
    assert!(
        sources.len() > 100,
        "the scan must see the worker tree, saw {}",
        sources.len()
    );
    for (path, code) in &sources {
        for idiom in [
            "filter(|a| a.is_in_person()).count()",
            "counts_toward_online_track() {",
        ] {
            assert!(
                !code.contains(idiom),
                "{path} counts a track from the full attendee list (`{idiom}`); call \
                 `handlers::capacity::count_tracks`, or tally a list already in hand with \
                 `TrackCounts`, so walk-ins take in-person spots"
            );
        }
    }
}

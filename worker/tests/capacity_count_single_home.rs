//! Guard: the in-person capacity gates must share one walk-in head-count.
//!
//! Two handlers enforce `in_person_capacity` — public registration and staff
//! walk-in registration. Each once carried its own copy of the count, and both
//! copies swallowed a D1 error into a silent zero, which under-counts the
//! event and admits a registration past the cap. `handlers::capacity` is now
//! the single home for that count and it fails closed.
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
             `handlers::capacity::count_walkins_against_cap` so the fail-closed \
             rule cannot drift per gate"
        );
    }
}

#[test]
fn both_capacity_gates_propagate_the_count_error() {
    for gate in GATES {
        let code = code_of(gate);
        assert!(
            code.contains("count_walkins_against_cap(state, config).await?"),
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

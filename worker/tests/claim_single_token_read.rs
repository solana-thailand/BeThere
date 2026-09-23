//! The public claim flows read the attendee row by claim token once.
//!
//! Plan 028 W6. `GET /api/claim/{token}` used to read the same indexed row
//! three times (event-id peek, walk-in check, D1 fallback) and
//! `POST /api/claim/{token}` three times (peek, walk-in check, the D1-first
//! Sheets helper). Both now take one `resolve_claim_context` read and reuse
//! it. A local `wrangler dev` A/B counted 3 → 2 row reads for a pre-registered
//! GET, 2 → 1 for a walk-in GET, and 3 → 1 for a POST.
//!
//! This guard keeps a later edit from quietly re-adding a per-step read.

use std::{fs, path::Path};

const CLAIM_FLOWS: &[&str] = &["claim/mint/lookup.rs", "claim/mint/execute.rs"];

/// Calls that each re-read the row the shared context already holds.
const REDUNDANT_READS: &[&str] = &[
    "db::attendees::get_attendee_by_claim_token(",
    "db::attendees::get_attendee_event_id_by_claim_token(",
    "coalesce_event_id(",
];

fn source(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(rel);
    let body = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{rel} is unreadable: {e} — was the module moved?"));
    // Drop `//` comments so prose naming an old call is not read as a call.
    body.lines()
        .map(|line| match line.find("//") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn each_claim_flow_resolves_the_token_row_exactly_once() {
    for flow in CLAIM_FLOWS {
        let code = source(flow);
        let calls = code.matches("resolve_claim_context(").count();
        assert_eq!(
            calls, 1,
            "{flow} calls resolve_claim_context {calls} times; the claim flow \
             should read the token row once and reuse it (plan 028 W6)"
        );
    }
}

#[test]
fn claim_flows_do_not_re_read_the_token_row() {
    for flow in CLAIM_FLOWS {
        let code = source(flow);
        for call in REDUNDANT_READS {
            assert!(
                !code.contains(call),
                "{flow} calls `{call}`, which re-reads the attendee row that \
                 resolve_claim_context already returned (plan 028 W6). Reuse \
                 ClaimContext instead."
            );
        }
    }
}

/// Guard the guard: if the helper is renamed or stops using the combined
/// read, the counts above would pin the wrong thing.
#[test]
fn the_shared_context_uses_the_combined_d1_read() {
    let helpers = source("claim/mint/helpers.rs");
    assert!(
        helpers.contains("async fn resolve_claim_context("),
        "claim/mint/helpers.rs no longer defines resolve_claim_context"
    );
    assert!(
        helpers.contains("get_claim_token_row("),
        "resolve_claim_context no longer uses the one-read get_claim_token_row"
    );
}

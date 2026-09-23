//! The admin roster ships a bounded recent-check-ins list and runs its three
//! annotation queries together.
//!
//! Plan 028 W9. `GET /api/attendees` used to put every checked-in attendee
//! into `stats.recent_check_ins` on every dashboard poll, while the dashboard
//! shows 10 per tab. It also awaited the credit-balance, credit-applied and
//! THB-settlement batch queries one after another. The bound itself is tested
//! in `domain/tests/recent_check_ins.rs`; this guard keeps the handler on it.

use std::{fs, path::Path};

const HANDLER: &str = "handlers/attendee/list.rs";

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
fn roster_builds_recent_check_ins_through_the_bounded_helper() {
    let code = source(HANDLER);
    assert!(
        code.contains("recent_check_ins(&attendees, RECENT_CHECK_INS_PER_TYPE)"),
        "{HANDLER} must build stats.recent_check_ins with the domain helper \
         bounded by RECENT_CHECK_INS_PER_TYPE (plan 028 W9)"
    );
    assert!(
        !code.contains("RecentCheckIn {"),
        "{HANDLER} builds RecentCheckIn values by hand again; an unbounded \
         list goes out on every dashboard poll (plan 028 W9)"
    );
}

#[test]
fn roster_annotation_queries_run_concurrently() {
    let code = source(HANDLER);
    let join = code.find("join!(").unwrap_or_else(|| {
        panic!("{HANDLER} no longer joins its annotation queries (plan 028 W9)")
    });
    let joined = &code[join..];
    let end = joined.find(");").unwrap_or(joined.len());
    for query in [
        "thb_balances_by_email(",
        "emails_applied_credit(",
        "settlement_by_attendee(",
    ] {
        assert!(
            joined[..end].contains(query),
            "{HANDLER}: `{query}` is no longer inside the join! (plan 028 W9)"
        );
    }
}

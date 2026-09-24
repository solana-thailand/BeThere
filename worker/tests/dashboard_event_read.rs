//! The live dashboard poll does not read the whole `events` row.
//!
//! Plan 028 W10. `GET /api/dashboard/live` is polled every 2.5 s. It used to
//! `SELECT *` the event from D1 and then load the same event again, KV-first,
//! for the access check. The access check's `EventConfig` now supplies the
//! event fields, and the active-event fallback reads only the id.
//!
//! Without an events KV binding the resolver hands back the global defaults
//! for any id, so the handler must also check that the id it got back is the
//! one it asked for.

use std::{fs, path::Path};

const HANDLER: &str = "handlers/dashboard.rs";

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
fn dashboard_poll_does_not_select_the_full_event_row() {
    let code = source(HANDLER);
    for call in ["db::events::get_event(", "db::events::get_active_event("] {
        assert!(
            !code.contains(call),
            "{HANDLER} calls `{call}`: a full `events` row read on every 2.5 s \
             poll. Use the EventConfig from resolve_event_with_access and \
             `dashboard::active_event_id` (plan 028 W10)."
        );
    }
    assert!(
        code.contains("dashboard::active_event_id("),
        "{HANDLER} no longer resolves the active event through the id-only query"
    );
}

#[test]
fn dashboard_rejects_a_config_for_another_event() {
    let code = source(HANDLER);
    assert!(
        code.contains("if event.id != event_id"),
        "{HANDLER} must 404 when the resolved EventConfig is not the requested \
         event (the no-KV path returns the global defaults for any id)"
    );
}

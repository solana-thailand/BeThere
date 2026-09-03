//! The organiser-supplied Google Sheets tab names must be validated on write.
//!
//! `sheet_name` / `staff_sheet_name` are free text on the event form and reach
//! a Sheets A1 range on every read and write. `sheets::a1::sheet_ref` quotes
//! them so that any *legal* name works, but it cannot make an *illegal* one
//! exist: Google refuses a tab name over 100 characters, one containing
//! `[ ] * ? / \`, or one starting or ending with an apostrophe. Such a name is
//! stored happily and then fails on every Sheets call — silently, because
//! nearly all of them are detached best-effort work whose errors are logged and
//! dropped. The organiser just sees a tab that never fills in.
//!
//! `event_checkin_domain::models::event::normalize_sheet_name` is the rule; this
//! file pins that the worker's write paths actually apply it.
//!
//! ## Layer 1 — behaviour, through `apply_update`
//!
//! The pure, IO-free half of the update path. Drives it with names that must be
//! rejected, names that must survive, and the blank case.
//!
//! ## Layer 2 — source-scan drift guard
//!
//! `update.rs` carries two near-identical copies of the update logic
//! (`update_event`, async and DB-backed; `apply_update`, pure) — see
//! `escrow_transition_contract.rs`, which pins the same duplication for escrow
//! transitions. Only the pure one is reachable from a test, so Layer 2 asserts
//! textually that both copies route through the shared `apply_sheet_names`
//! helper and that neither assigns a tab name from the request directly.
//!
//! ## Run
//!
//! ```sh
//! cargo test -p event-checkin-worker --test sheet_name_validation
//! ```

use std::fs;
use std::path::Path;

use event_checkin_domain::models::event::{
    DEFAULT_ATTENDEE_SHEET_NAME, EventConfig, UpdateEventRequest,
};
use event_checkin_worker::event_store::apply_update;

/// Path to the file holding both copies of the update logic.
const UPDATE_RS: &str = "src/event_store/write/update.rs";

/// The shared helper both copies must call.
const HELPER: &str = "apply_sheet_names(";

/// Names Google will not accept as a tab title. Each must be refused at the
/// write path rather than stored and left to fail at the API.
const REJECTED: &[&str] = &[
    "day[1]",
    "a/b",
    "a\\back",
    "why?",
    "star*",
    "'Day 1'",
    "trailing'",
    "line\nbreak",
];

/// Legal tab names, including ones that need A1 quoting. Quoting is a separate
/// concern from validity — these must all be stored as typed (trimmed).
const ACCEPTED: &[(&str, &str)] = &[
    ("Attendees", "Attendees"),
    ("Attendee List", "Attendee List"),
    ("  Day 1  ", "Day 1"),
    ("walk-ins", "walk-ins"),
    ("Bob's tab", "Bob's tab"),
    ("ผู้เข้าร่วม", "ผู้เข้าร่วม"),
];

/// A config with both tab names set to the shipped defaults.
///
/// Minimal JSON, as in `escrow_transition_contract.rs`: `escrow_address` is
/// empty so `apply_update` skips the SEC-002 field lock and reaches the tab
/// name handling.
fn make_config() -> EventConfig {
    let json = r#"{"id":"x","name":"E","slug":"x","tagline":"","link":"","status":"draft","event_start_ms":0,"event_end_ms":0,"sheet_id":"s","sheet_name":"Attendees","staff_sheet_name":"staff","created_at":"","updated_at":""}"#;
    serde_json::from_str(json).expect("minimal EventConfig JSON must parse")
}

fn update_rs() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(UPDATE_RS);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

#[test]
fn illegal_tab_names_are_rejected_on_update() {
    for raw in REJECTED {
        let mut config = make_config();
        let req = UpdateEventRequest {
            sheet_name: Some((*raw).to_string()),
            ..Default::default()
        };
        let err = apply_update(&mut config, &req)
            .expect_err(&format!("{raw:?} is not a legal Google tab name"));
        assert!(
            err.contains("sheet name"),
            "{raw:?} rejected with an unhelpful message: {err}"
        );
        assert_eq!(
            config.sheet_name, "Attendees",
            "{raw:?} was rejected but the config was mutated anyway"
        );
    }
}

#[test]
fn the_staff_tab_name_is_validated_too() {
    // Easy to validate one field and forget its twin.
    let mut config = make_config();
    let req = UpdateEventRequest {
        staff_sheet_name: Some("staff/2".to_string()),
        ..Default::default()
    };
    assert!(apply_update(&mut config, &req).is_err());
    assert_eq!(config.staff_sheet_name, "staff");
}

#[test]
fn legal_tab_names_are_stored_trimmed() {
    for (raw, want) in ACCEPTED {
        let mut config = make_config();
        let req = UpdateEventRequest {
            sheet_name: Some((*raw).to_string()),
            ..Default::default()
        };
        apply_update(&mut config, &req).unwrap_or_else(|e| panic!("{raw:?} must be accepted: {e}"));
        assert_eq!(&config.sheet_name, want);
    }
}

#[test]
fn clearing_the_field_keeps_the_events_current_tab() {
    // Not the default: a blank field must not silently retarget a live event
    // from `Registrations` to `Attendees`.
    let mut config = make_config();
    config.sheet_name = "Registrations".to_string();
    let req = UpdateEventRequest {
        sheet_name: Some("   ".to_string()),
        ..Default::default()
    };
    apply_update(&mut config, &req).expect("a blank tab name is not an error");
    assert_eq!(config.sheet_name, "Registrations");
}

#[test]
fn a_legacy_event_with_no_tab_name_falls_back_to_the_default() {
    let mut config = make_config();
    config.sheet_name = String::new();
    let req = UpdateEventRequest {
        sheet_name: Some(String::new()),
        ..Default::default()
    };
    apply_update(&mut config, &req).expect("a blank tab name is not an error");
    assert_eq!(config.sheet_name, DEFAULT_ATTENDEE_SHEET_NAME);
}

#[test]
fn both_copies_of_the_update_logic_route_through_the_helper() {
    // `update_event` is async and DB-backed, so Layer 1 cannot reach it. If a
    // future edit inlines the assignment back into one copy, that copy stops
    // validating and nothing else in the suite notices.
    let src = update_rs();
    let calls = src.matches(HELPER).count();
    assert_eq!(
        calls, 3,
        "expected `{HELPER}` once per definition and once per call site \
         (update_event, apply_update) in {UPDATE_RS}, found {calls}"
    );
}

#[test]
fn the_update_path_never_assigns_a_tab_name_directly() {
    // The shape being guarded against is the one that was there before:
    //     config.sheet_name = sheet_name.clone();
    // Textual, because that is how it gets reintroduced — copied off a
    // neighbouring field's line.
    let src = update_rs();
    for (line_no, line) in src.lines().enumerate() {
        let code = line.split("//").next().unwrap_or(line);
        for field in ["config.sheet_name =", "config.staff_sheet_name ="] {
            assert!(
                !code.contains(field) || code.contains("normalize_sheet_name"),
                "{UPDATE_RS}:{} assigns a tab name without validating it: {}",
                line_no + 1,
                line.trim()
            );
        }
    }
}

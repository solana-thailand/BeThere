//! The booking code on the ticket and in the scanner (`.issues/178`, plan 039
//! `Nº`).
//!
//! The crate is wasm-only, so these are source and catalog guards; the code
//! format itself is behaviour-tested in `domain/tests/display_code.rs` and the
//! lookup's event scoping in `worker/tests/attendee_display_code.rs`.

use std::path::{Path, PathBuf};

use event_checkin_domain::models::attendee::DisplayCode;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel))
        .unwrap_or_else(|e| panic!("{rel} unreadable: {e} — moved? update this guard"))
}

fn read_workspace(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{rel} unreadable: {e}"))
}

fn ticket_code_catalog(locale: &str) -> serde_json::Value {
    let text = read(&format!("locales/{locale}/ticket.json"));
    let json: serde_json::Value = serde_json::from_str(&text).expect("ticket.json");
    json.get("code").cloned().expect("ticket.code group")
}

#[test]
fn the_code_sits_under_the_qr_in_both_ticket_states() {
    let src = read("src/pages/ticket/qr_section.rs");
    // Pre- and post-check-in branches each render the QR once, and the code
    // follows it directly.
    let wrappers = src.matches("class=\"ticket-qr-wrapper\"").count();
    assert_eq!(wrappers, 2);
    let mut rest = src.as_str();
    for _ in 0..wrappers {
        let at = rest.find("class=\"ticket-qr-wrapper\"").expect("wrapper");
        rest = &rest[at..];
        let close = rest.find("</div>").expect("wrapper closes");
        let next = rest[close..].trim_start_matches("</div>").trim_start();
        assert!(
            next.starts_with("<TicketCode code=display_code.clone() />"),
            "the code must follow the QR image directly"
        );
        rest = &rest[close..];
    }
    // Plan 039 prints it as `Nº XXXXXX`, and an empty code renders nothing.
    assert!(src.contains("format!(\"Nº {code}\")"));
    assert!(src.contains("(!code.is_empty()).then("));
    assert!(
        src.contains("translate=\"no\""),
        "a browser must not translate the code"
    );
}

#[test]
fn the_client_reads_the_code_as_a_defaulted_string() {
    let types = read("src/api/types.rs");
    let at = types
        .find("pub display_code: String,")
        .expect("AttendeeData.display_code");
    let before = &types[..at];
    let attr = before.rfind("#[serde(default)]").expect("serde(default)");
    assert!(
        !before[attr..].contains("pub "),
        "#[serde(default)] must sit on display_code itself"
    );
    // The worker never sends null for it (serde(default) does not cover null).
    let worker = read_workspace("worker/src/handlers/attendee/read.rs");
    assert!(worker.contains("\"display_code\": display_code,"));
    assert!(worker.contains("code.unwrap_or_default()"));
}

#[test]
fn the_scanner_routes_a_typed_code_through_the_event_scoped_lookup() {
    let page = read("src/pages/scanner/page.rs");
    assert!(page.contains("match classify_manual_entry(&value)"));
    assert!(page.contains("Some(ManualEntry::Code(code)) => process_display_code("));
    assert!(page.contains("active_event_id.get_untracked()"));

    let logic = read("src/pages/scanner/logic.rs");
    // A code is tried first; anything else stays on the id path.
    let classify = logic
        .find("fn classify_manual_entry(")
        .expect("classify_manual_entry");
    let body = &logic[classify..];
    let parse = body.find("DisplayCode::parse(text)").expect("parse first");
    let id = body.find("extract_attendee_id(text)").expect("id fallback");
    assert!(parse < id);

    // No event selected: refuse rather than let the server fall back to the
    // active event.
    let process = &logic[logic.find("fn process_display_code(").expect("fn")..];
    assert!(process.contains("let Some(event_id) = event_id.filter(|e| !e.is_empty()) else {"));
    // The same event scopes the code lookup and the attendee fetch after it.
    assert!(process.contains("api::lookup_attendee_id_by_code(&code, Some(&event_id))"));
    assert!(
        process.contains("lookup_and_classify(attendee_id, Some(event_id), set_state, set_toast)")
    );

    // The client path is the worker's staff route.
    let api = read("src/api/attendee.rs");
    assert!(api.contains("format!(\"/attendees/by-code/{code}?event_id={eid}\")"));
    let router = read_workspace("worker/src/handlers/mod.rs");
    assert!(router.contains("\"/attendees/by-code/{code}\""));
}

#[test]
fn both_locales_carry_the_code_strings_and_the_example_is_a_real_code() {
    let en = ticket_code_catalog("en");
    let th = ticket_code_catalog("th");
    for key in [
        "label",
        "hint",
        "scanner_placeholder",
        "scanner_not_found",
        "scanner_pick_event",
    ] {
        let e = en.get(key).and_then(|v| v.as_str()).unwrap_or_default();
        let t = th.get(key).and_then(|v| v.as_str()).unwrap_or_default();
        assert!(!e.is_empty() && !t.is_empty(), "ticket.code.{key} missing");
        assert_ne!(e, t, "ticket.code.{key} is not translated");
    }
    // The placeholder's example must itself parse, or staff copy a code the
    // scanner rejects.
    for catalog in [&en, &th] {
        let placeholder = catalog["scanner_placeholder"].as_str().expect("str");
        let example = placeholder
            .split(|c: char| !c.is_ascii_alphanumeric())
            .find(|w| w.len() == 6 && w.chars().any(|c| c.is_ascii_digit()))
            .expect("an example code in the placeholder");
        assert!(
            DisplayCode::parse(example).is_ok(),
            "{example} is not a valid code"
        );
    }
}

#[test]
fn the_code_has_styles() {
    let css = read("styles/style-11-ticket.css");
    for class in [
        ".ticket-code ",
        ".ticket-code-label",
        ".ticket-code-value",
        ".ticket-code-hint",
    ] {
        assert!(css.contains(class), "{class} has no rule");
    }
}

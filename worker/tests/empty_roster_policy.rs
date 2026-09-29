//! `.issues/167` part B: an event created after D1 became the authoritative
//! attendee store reads an empty D1 roster as empty, never as the rows of
//! another event that shares its sheet.

use std::{fs, path::Path};

use event_checkin_worker::empty_roster::{D1_AUTHORITATIVE_SINCE, EmptyRoster};

#[test]
fn events_created_after_the_cutoff_trust_an_empty_roster() {
    // A duplicate made today, in the shape `create_event` writes.
    assert_eq!(
        EmptyRoster::for_created_at("2026-09-30T08:15:02.123456+00:00"),
        EmptyRoster::Trust
    );
    assert_eq!(
        EmptyRoster::for_created_at(D1_AUTHORITATIVE_SINCE),
        EmptyRoster::Trust
    );
    // Same instant as the cutoff, written in Bangkok time.
    assert_eq!(
        EmptyRoster::for_created_at("2026-08-12T01:59:34+07:00"),
        EmptyRoster::Trust
    );
}

#[test]
fn legacy_and_unreadable_timestamps_keep_the_sheet() {
    for created_at in [
        "2026-08-11T18:59:33Z",
        "2026-08-12T01:59:33+07:00",
        "2026-07-01T00:00:00+00:00",
        "",
        "not a date",
        "2026-09-30",
    ] {
        assert_eq!(
            EmptyRoster::for_created_at(created_at),
            EmptyRoster::ReadSheet,
            "{created_at:?}"
        );
    }
}

fn rust_sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).expect("source directory is readable") {
        let path = entry.expect("directory entry is readable").path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Every per-event roster read picks the event's policy. Only the sheet sync,
/// which exists to read the sheet, may force `ReadSheet`.
#[test]
fn every_roster_read_uses_the_event_policy() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_sources(&src.join("handlers"), &mut files);
    let mut calls = 0;
    for path in files {
        let source = fs::read_to_string(&path).expect("source is readable");
        let rel = path
            .strip_prefix(&src)
            .expect("under src")
            .to_string_lossy();
        for (at, _) in source.match_indices("get_attendees_for_event(") {
            calls += 1;
            let call = &source[at..];
            let end = call.find(".await").expect("the call is awaited");
            let args = &call[..end];
            let sync = rel.ends_with("handlers/events/sync.rs");
            match sync {
                true => assert!(
                    args.contains("EmptyRoster::ReadSheet"),
                    "{rel}: the sheet sync must read the sheet"
                ),
                false => assert!(
                    args.contains("EmptyRoster::for_event("),
                    "{rel}: roster read without the event's EmptyRoster policy"
                ),
            }
        }
    }
    assert!(calls >= 7, "found {calls} roster reads; the scan broke");
}

/// The reader returns the empty roster instead of reading the sheet.
#[test]
fn trusted_empty_roster_returns_before_the_sheet() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sheets/attendees.rs");
    let source = fs::read_to_string(&path).expect("attendees.rs is readable");
    let start = source
        .find("async fn get_attendees_inner")
        .expect("get_attendees_inner moved; update this guard");
    let body = &source[start..];
    let trust = body
        .find("EmptyRoster::Trust")
        .expect("get_attendees_inner checks EmptyRoster::Trust");
    let ret = body[trust..]
        .find("return Ok(Vec::new())")
        .map(|offset| trust + offset)
        .expect("a trusted empty roster returns an empty list");
    let sheet = body
        .find("get_cached_access_token")
        .expect("the sheet fallback");
    assert!(
        ret < sheet,
        "the trusted return must come before the sheet read"
    );
}

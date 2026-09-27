//! `.issues/152` "Not done": a failed D1 event dual-write must reach the admin
//! who saved as a response warning, not only a log line.

use event_checkin_worker::event_store::D1Sync;

#[test]
fn only_a_failed_sync_warns() {
    assert_eq!(D1Sync::Synced.warnings(), Vec::<&str>::new());
    assert_eq!(D1Sync::NoDatabase.warnings(), Vec::<&str>::new());
    assert_eq!(D1Sync::Failed.warnings().len(), 1);
}

#[test]
fn failed_warning_survives_the_error_redactor() {
    // The redactor rewrites anything containing "://" to [redacted-url].
    let text = D1Sync::Failed.warning().expect("failed sync warns");
    assert!(!text.contains("://"), "{text}");
    assert!(!text.is_empty());
}

#[test]
fn event_save_handlers_return_the_sync_outcome() {
    // `#[must_use]` stops a bare `.await;`; this pins that each admin-facing
    // handler also puts the outcome in its response.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers/events");
    for (file, expected) in [
        ("update.rs", 1),
        ("create.rs", 1),
        ("seed.rs", 1),
        ("lifecycle.rs", 2),
        ("poster.rs", 2),
    ] {
        let src = std::fs::read_to_string(dir.join(file)).expect(file);
        let n = src.matches("\"warnings\": d1_sync.warnings()").count();
        assert_eq!(
            n, expected,
            "{file}: sync outcome missing from the response"
        );
    }
}

#[test]
fn store_writes_hand_the_sync_outcome_to_the_caller() {
    // The store-level create/update used to `let _ =` the outcome, so the
    // poster, escrow-confirm and duplicate paths could only log a failure.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for file in ["event_store/write/create.rs", "event_store/write/update.rs"] {
        let src = std::fs::read_to_string(root.join(file)).expect(file);
        assert!(
            !src.contains("let _ = sync_event_to_d1"),
            "{file}: D1 sync outcome discarded"
        );
        assert!(
            src.contains("Ok(SavedEvent { config, d1_sync })"),
            "{file}: D1 sync outcome not returned"
        );
    }
    for (file, needle) in [
        (
            "handlers/events/duplicate.rs",
            "warnings.extend(d1_sync.warning()",
        ),
        (
            "handlers/deposit/escrow/status.rs",
            "saved.d1_sync.warnings()",
        ),
    ] {
        let src = std::fs::read_to_string(root.join(file)).expect(file);
        assert!(
            src.contains(needle),
            "{file}: sync outcome missing from the response"
        );
    }
}

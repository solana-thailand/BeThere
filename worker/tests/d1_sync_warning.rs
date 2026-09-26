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
    ] {
        let src = std::fs::read_to_string(dir.join(file)).expect(file);
        let n = src.matches("\"warnings\": d1_sync.warnings()").count();
        assert_eq!(
            n, expected,
            "{file}: sync outcome missing from the response"
        );
    }
}

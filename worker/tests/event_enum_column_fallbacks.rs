//! Plan 022 §8 — enum columns must not degrade silently when D1 holds a value
//! nobody wrote.
//!
//! `D1EventRow::to_event_config` used to parse every enum column as
//! `serde_json::from_value(...).unwrap_or_default()`. That collapsed three
//! very different situations into one silent answer:
//!
//! 1. the column is `NULL` because the row predates the migration that added
//!    it — the row legitimately means "the pre-migration value";
//! 2. the column parses;
//! 3. the column holds something no writer produces (every writer goes through
//!    `as_str()`, so this means an out-of-band D1 edit or a migration default).
//!
//! Case 3 is the dangerous one, and `Default` pointed the wrong way twice:
//! `EventVisibility::default()` is `Public`, so a corrupt `visibility` column
//! **published a private event**; `EscrowStatus::default()` is `None`, whose
//! `is_active()` is false, so a corrupt `escrow_status` unlocked archiving,
//! deleting and repointing an escrow that may still hold funds. Nothing was
//! logged in either case, so the corruption was invisible.
//!
//! These tests pin the per-column fallback *directions*. They are behavioural
//! — they drive `to_event_config` — so they stay honest if the parse is
//! rewritten.

use event_checkin_domain::models::event::{
    EscrowStatus, EventFormat, EventStatus, EventVisibility, OnlineOpenMode,
};
use event_checkin_worker::db::events::D1EventRow;

/// Build a row with every column at its `NULL` default, then override one.
fn row_with(column: &str, value: Option<&str>) -> D1EventRow {
    let mut obj = serde_json::Map::new();
    obj.insert("id".to_string(), serde_json::json!("evt-under-test"));
    obj.insert(
        column.to_string(),
        match value {
            Some(v) => serde_json::json!(v),
            None => serde_json::Value::Null,
        },
    );
    serde_json::from_value(serde_json::Value::Object(obj))
        .expect("D1EventRow is #[serde(default)] — a partial object must deserialize")
}

// ---------------------------------------------------------------------------
// visibility — the fallback that used to publish private events
// ---------------------------------------------------------------------------

#[test]
fn null_visibility_stays_public() {
    // Rows written before the visibility column existed were all public.
    // Failing these closed would silently hide every legacy event.
    assert_eq!(
        row_with("visibility", None).to_event_config().visibility,
        EventVisibility::Public
    );
}

#[test]
fn empty_visibility_stays_public() {
    assert_eq!(
        row_with("visibility", Some(""))
            .to_event_config()
            .visibility,
        EventVisibility::Public
    );
}

#[test]
fn stored_visibility_round_trips() {
    assert_eq!(
        row_with("visibility", Some("private"))
            .to_event_config()
            .visibility,
        EventVisibility::Private
    );
    assert_eq!(
        row_with("visibility", Some("public"))
            .to_event_config()
            .visibility,
        EventVisibility::Public
    );
}

#[test]
fn padded_values_still_parse() {
    // A hand-edited cell with a stray space is a typo, not corruption: trim
    // first, so the stored intent survives. Asserted on `status`, where the
    // parsed value (`Active`) and the fail-closed fallback (`Draft`) differ —
    // on `visibility` both roads lead to `Private` and the assertion is blind.
    assert_eq!(
        row_with("status", Some(" active "))
            .to_event_config()
            .status,
        EventStatus::Active
    );
    assert_eq!(
        row_with("visibility", Some(" private "))
            .to_event_config()
            .visibility,
        EventVisibility::Private
    );
}

#[test]
fn unrecognised_visibility_fails_closed_to_private() {
    for bogus in ["Private", "PRIVATE", "hidden", "unlisted", "?"] {
        assert_eq!(
            row_with("visibility", Some(bogus))
                .to_event_config()
                .visibility,
            EventVisibility::Private,
            "unrecognised visibility {bogus:?} must not publish the event"
        );
    }
}

// ---------------------------------------------------------------------------
// escrow_status — the fallback that used to unlock a live escrow
// ---------------------------------------------------------------------------

#[test]
fn null_escrow_status_is_none() {
    let config = row_with("escrow_status", None).to_event_config();
    assert_eq!(config.escrow_status, EscrowStatus::None);
    assert!(!config.escrow_status.is_active());
}

#[test]
fn stored_escrow_status_round_trips() {
    assert_eq!(
        row_with("escrow_status", Some("initialized"))
            .to_event_config()
            .escrow_status,
        EscrowStatus::Initialized
    );
    assert_eq!(
        row_with("escrow_status", Some("deactivated"))
            .to_event_config()
            .escrow_status,
        EscrowStatus::Deactivated
    );
}

#[test]
fn unrecognised_escrow_status_fails_closed_to_active() {
    for bogus in ["Initialized", "initialised", "live", "?"] {
        let config = row_with("escrow_status", Some(bogus)).to_event_config();
        assert!(
            config.escrow_status.is_active(),
            "unrecognised escrow_status {bogus:?} must read as active so \
             archive/delete/repoint stay blocked, got {:?}",
            config.escrow_status
        );
    }
}

// ---------------------------------------------------------------------------
// status / online_open_mode / event_format
// ---------------------------------------------------------------------------

#[test]
fn null_and_unrecognised_status_both_read_as_draft() {
    assert_eq!(
        row_with("status", None).to_event_config().status,
        EventStatus::Draft
    );
    assert_eq!(
        row_with("status", Some("live")).to_event_config().status,
        EventStatus::Draft,
        "a corrupt status must keep the event out of every listing"
    );
    assert_eq!(
        row_with("status", Some("active")).to_event_config().status,
        EventStatus::Active
    );
}

#[test]
fn null_online_open_mode_keeps_auto_on_full() {
    assert_eq!(
        row_with("online_open_mode", None)
            .to_event_config()
            .online_open_mode,
        OnlineOpenMode::AutoOnFull
    );
}

#[test]
fn unrecognised_online_open_mode_fails_closed_to_manual() {
    // `Always` — the `Default` — would fling online registration open on a
    // hybrid event whose organizer never asked for that.
    assert_eq!(
        row_with("online_open_mode", Some("whenever"))
            .to_event_config()
            .online_open_mode,
        OnlineOpenMode::Manual
    );
}

#[test]
fn event_format_falls_back_to_in_person() {
    assert_eq!(
        row_with("event_format", None)
            .to_event_config()
            .event_format,
        EventFormat::InPerson
    );
    assert_eq!(
        row_with("event_format", Some("irl"))
            .to_event_config()
            .event_format,
        EventFormat::InPerson
    );
    assert_eq!(
        row_with("event_format", Some("hybrid"))
            .to_event_config()
            .event_format,
        EventFormat::Hybrid
    );
}

// ---------------------------------------------------------------------------
// Drift guard — the same parse used to exist in three copies
// ---------------------------------------------------------------------------

/// `to_event_config`, `list_past_events_raw` and `list_public_events_raw` each
/// carried their own `from_value(...).unwrap_or_default()` block. The listing
/// readers are the *worst* place for it: `handlers/public_event.rs:74` filters
/// the landing page on the `visibility` string they emit, so a fallback of
/// `Public` there puts a private event on the front page. Per-reader copies of
/// a rule are how this codebase's guards keep diverging (plan 022), so pin the
/// parse to a single home.
#[test]
fn the_enum_parse_has_exactly_one_home() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/db/events.rs"))
        .expect("worker/src/db/events.rs must be readable");
    // Prose about the pattern must not satisfy a rule about code.
    let code: String = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(
        code.matches("serde_json::from_value(serde_json::Value::String")
            .count(),
        1,
        "enum strings must be parsed only through `parse_enum_column`; a second \
         string-to-enum conversion means a reader grew its own copy of the rule"
    );
    assert!(
        code.matches("parse_enum_column(").count() >= 11,
        "expected all three readers to route their enum columns through the \
         helper (5 + 3 + 3 columns)"
    );
    assert!(
        !code.contains(".unwrap_or_else(|| \"public\".to_string())"),
        "the legacy-NULL fallback belongs in `parse_enum_column`'s `legacy` \
         argument, not open-coded at a call site"
    );
}

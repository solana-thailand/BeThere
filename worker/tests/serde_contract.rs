//! Serde contract tests — validate backend enums serialize as expected snake_case JSON.
//!
//! This test uses the **actual** domain types from `event-checkin-domain` to verify
//! that every enum with `#[serde(rename_all = "snake_case")]` round-trips correctly.
//!
//! The frontend enums in `frontend-leptos/src/api/` must produce/consume the same JSON.
//! The mirrored test in `frontend-leptos/tests/serde_contract.rs` validates the frontend side.
//!
//! Run: `cargo test --test serde_contract` (from worker directory)

use serde::{Deserialize, Serialize};

use event_checkin_domain::models::adventure::AdventureStatus;
use event_checkin_domain::models::api::{QrGenerationStatus, QuizStatus};
use event_checkin_domain::models::attendee::CheckInStatus;
use event_checkin_domain::models::deposit::DepositMethod;
use event_checkin_domain::models::event::{
    CreateEventRequest, DuplicateEventRequest, EscrowStatus, EventConfig, EventFormat, EventStatus,
    EventVisibility, OnlineOpenMode, UpdateEventRequest,
};

// ================================================================================================
// Helpers
// ================================================================================================

/// Serialize a value, then deserialize back and verify round-trip against expected JSON.
fn assert_round_trip<T>(expected_json: &str, value: T)
where
    T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    let serialized = serde_json::to_string(&value)
        .unwrap_or_else(|e| panic!("Failed to serialize {:?}: {e}", value));
    assert_eq!(
        serialized, expected_json,
        "serialization mismatch for {:?}",
        value
    );

    let deserialized: T = serde_json::from_str(expected_json).unwrap_or_else(|e| {
        panic!(
            "Failed to deserialize {expected_json} into {}: {e}",
            std::any::type_name::<T>()
        )
    });
    assert_eq!(deserialized, value, "round-trip deserialization mismatch");
}

/// Verify that PascalCase JSON is rejected (proves rename_all = "snake_case" is active).
fn assert_rejects_pascal_case<T>(pascal_json: &str)
where
    T: for<'de> Deserialize<'de> + std::fmt::Debug,
{
    let result = serde_json::from_str::<T>(pascal_json);
    assert!(
        result.is_err(),
        "Expected deserialization failure for {pascal_json}, but got: {:?}",
        result.unwrap()
    );
}

// ================================================================================================
// EventStatus — draft, active, completed, archived
// ================================================================================================

#[test]
fn event_status_round_trip() {
    assert_round_trip(r#""draft""#, EventStatus::Draft);
    assert_round_trip(r#""active""#, EventStatus::Active);
    assert_round_trip(r#""completed""#, EventStatus::Completed);
    assert_round_trip(r#""archived""#, EventStatus::Archived);
}

#[test]
fn event_status_rejects_pascal_case() {
    assert_rejects_pascal_case::<EventStatus>(r#""Draft""#);
    assert_rejects_pascal_case::<EventStatus>(r#""Active""#);
}

// ================================================================================================
// EscrowStatus — none, initialized, deactivated, closed, cancelled
// ================================================================================================

#[test]
fn escrow_status_round_trip() {
    assert_round_trip(r#""none""#, EscrowStatus::None);
    assert_round_trip(r#""initialized""#, EscrowStatus::Initialized);
    assert_round_trip(r#""deactivated""#, EscrowStatus::Deactivated);
    assert_round_trip(r#""closed""#, EscrowStatus::Closed);
    assert_round_trip(r#""cancelled""#, EscrowStatus::Cancelled);
}

#[test]
fn escrow_status_rejects_pascal_case() {
    assert_rejects_pascal_case::<EscrowStatus>(r#""None""#);
    assert_rejects_pascal_case::<EscrowStatus>(r#""Initialized""#);
}

// ================================================================================================
// EventFormat — in_person, online, hybrid
// ================================================================================================

#[test]
fn event_format_round_trip() {
    assert_round_trip(r#""in_person""#, EventFormat::InPerson);
    assert_round_trip(r#""online""#, EventFormat::Online);
    assert_round_trip(r#""hybrid""#, EventFormat::Hybrid);
}

#[test]
fn event_format_rejects_pascal_and_camel_case() {
    // Without rename_all, serde would produce "InPerson" — this must fail
    assert_rejects_pascal_case::<EventFormat>(r#""InPerson""#);
    assert_rejects_pascal_case::<EventFormat>(r#""inPerson""#);
}

// ================================================================================================
// OnlineOpenMode — always, auto_on_full, manual
// (This was the enum that had the missing rename_all bug on the frontend)
// ================================================================================================

#[test]
fn online_open_mode_round_trip() {
    assert_round_trip(r#""always""#, OnlineOpenMode::Always);
    assert_round_trip(r#""auto_on_full""#, OnlineOpenMode::AutoOnFull);
    assert_round_trip(r#""manual""#, OnlineOpenMode::Manual);
}

#[test]
fn online_open_mode_rejects_pascal_case() {
    // This is exactly the bug: without rename_all, AutoOnFull → "AutoOnFull"
    assert_rejects_pascal_case::<OnlineOpenMode>(r#""AutoOnFull""#);
    assert_rejects_pascal_case::<OnlineOpenMode>(r#""autoOnFull""#);
}

// ================================================================================================
// QuizStatus — not_required, not_started, in_progress, passed
// ================================================================================================

#[test]
fn quiz_status_round_trip() {
    assert_round_trip(r#""not_required""#, QuizStatus::NotRequired);
    assert_round_trip(r#""not_started""#, QuizStatus::NotStarted);
    assert_round_trip(r#""in_progress""#, QuizStatus::InProgress);
    assert_round_trip(r#""passed""#, QuizStatus::Passed);
}

#[test]
fn quiz_status_rejects_pascal_case() {
    assert_rejects_pascal_case::<QuizStatus>(r#""NotRequired""#);
    assert_rejects_pascal_case::<QuizStatus>(r#""InProgress""#);
}

// ================================================================================================
// AdventureStatus — not_required, not_started, in_progress, passed
// ================================================================================================

#[test]
fn adventure_status_round_trip() {
    assert_round_trip(r#""not_required""#, AdventureStatus::NotRequired);
    assert_round_trip(r#""not_started""#, AdventureStatus::NotStarted);
    assert_round_trip(r#""in_progress""#, AdventureStatus::InProgress);
    assert_round_trip(r#""passed""#, AdventureStatus::Passed);
}

#[test]
fn adventure_status_rejects_pascal_case() {
    assert_rejects_pascal_case::<AdventureStatus>(r#""NotRequired""#);
    assert_rejects_pascal_case::<AdventureStatus>(r#""InProgress""#);
}

// ================================================================================================
// DepositMethod — usdc, thb
// ================================================================================================

#[test]
fn deposit_method_round_trip() {
    assert_round_trip(r#""usdc""#, DepositMethod::Usdc);
    assert_round_trip(r#""thb""#, DepositMethod::Thb);
}

#[test]
fn deposit_method_rejects_pascal_case() {
    assert_rejects_pascal_case::<DepositMethod>(r#""Usdc""#);
    assert_rejects_pascal_case::<DepositMethod>(r#""Thb""#);
}

// ================================================================================================
// CheckInStatus — pending_approval, approved, invited, checked_in
// ================================================================================================

#[test]
fn check_in_status_round_trip() {
    assert_round_trip(r#""pending_approval""#, CheckInStatus::PendingApproval);
    assert_round_trip(r#""approved""#, CheckInStatus::Approved);
    assert_round_trip(r#""invited""#, CheckInStatus::Invited);
    assert_round_trip(r#""checked_in""#, CheckInStatus::CheckedIn);
}

#[test]
fn check_in_status_rejects_pascal_case() {
    assert_rejects_pascal_case::<CheckInStatus>(r#""PendingApproval""#);
    assert_rejects_pascal_case::<CheckInStatus>(r#""CheckedIn""#);
}

// ================================================================================================
// QrGenerationStatus — generated, skipped
// ================================================================================================

#[test]
fn qr_generation_status_round_trip() {
    assert_round_trip(r#""generated""#, QrGenerationStatus::Generated);
    assert_round_trip(r#""skipped""#, QrGenerationStatus::Skipped);
}

#[test]
fn qr_generation_status_rejects_pascal_case() {
    assert_rejects_pascal_case::<QrGenerationStatus>(r#""Generated""#);
    assert_rejects_pascal_case::<QrGenerationStatus>(r#""Skipped""#);
}

// ================================================================================================
// EventVisibility — public, private
// ================================================================================================

#[test]
fn event_visibility_round_trip() {
    assert_round_trip(r#""public""#, EventVisibility::Public);
    assert_round_trip(r#""private""#, EventVisibility::Private);
}

#[test]
fn event_visibility_rejects_pascal_case() {
    assert_rejects_pascal_case::<EventVisibility>(r#""Public""#);
    assert_rejects_pascal_case::<EventVisibility>(r#""Private""#);
}

// ================================================================================================
// Integration — verify a full JSON payload with enum fields round-trips
// ================================================================================================

#[test]
fn full_event_meta_json_round_trips() {
    use event_checkin_domain::models::event::EventMeta;

    let json = r#"{
        "id": "test-event",
        "name": "Test Event",
        "slug": "test-event",
        "status": "active",
        "event_start_ms": 1700000000000,
        "event_end_ms": 1700003600000,
        "time_tba": false,
        "sheet_id": "sheet-123",
        "created_at": "2024-01-01T00:00:00Z",
        "organizer_emails": ["admin@example.com"],
        "deposit_enabled": true,
        "max_refundable_deposits": 0,
        "escrow_address": "",
        "escrow_status": "initialized",
        "event_format": "hybrid",
        "visibility": "public"
    }"#;

    let meta: EventMeta = serde_json::from_str(json).expect("should parse EventMeta");
    assert_eq!(meta.status, EventStatus::Active);
    assert_eq!(meta.escrow_status, EscrowStatus::Initialized);
    assert_eq!(meta.event_format, EventFormat::Hybrid);
    assert_eq!(meta.visibility, EventVisibility::Public);
}

// ================================================================================================
// DuplicateEventRequest — optional body for POST /api/events/{id}/duplicate (Issue #055)
//
// The handler accepts `Option<Json<DuplicateEventRequest>>` so it can be called
// with no body at all (frontend one-click duplicate). These tests pin the
// deserialization contract: empty/missing fields must default to empty strings,
// matching the `Default` impl used by `.unwrap_or_default()` in the handler.
// ================================================================================================

#[test]
fn duplicate_event_request_empty_object_uses_defaults() {
    let req: DuplicateEventRequest =
        serde_json::from_str("{}").expect("empty object should parse with defaults");
    assert_eq!(req.new_sheet_id, "");
    assert_eq!(req.new_name, "");
}

#[test]
fn duplicate_event_request_parses_overrides() {
    let json = r#"{"new_sheet_id":"new-sheet-abc","new_name":"Copied Event Name"}"#;
    let req: DuplicateEventRequest =
        serde_json::from_str(json).expect("payload with overrides should parse");
    assert_eq!(req.new_sheet_id, "new-sheet-abc");
    assert_eq!(req.new_name, "Copied Event Name");
}

#[test]
fn duplicate_event_request_partial_payload_defaults_missing_fields() {
    // Only new_name provided; new_sheet_id should default to empty.
    let json = r#"{"new_name":"Just A Name"}"#;
    let req: DuplicateEventRequest =
        serde_json::from_str(json).expect("partial payload should parse");
    assert_eq!(req.new_sheet_id, "");
    assert_eq!(req.new_name, "Just A Name");
}

#[test]
fn duplicate_event_request_default_matches_empty_object_parse() {
    // The handler uses `.unwrap_or_default()` when no body is provided; this
    // verifies the Default impl and the empty-object deserialization agree.
    let from_default = DuplicateEventRequest::default();
    let from_empty: DuplicateEventRequest =
        serde_json::from_str("{}").expect("empty object should parse");
    assert_eq!(from_default.new_sheet_id, from_empty.new_sheet_id);
    assert_eq!(from_default.new_name, from_empty.new_name);
}

// ================================================================================================
// poster_url — Plan 009 (event marketing poster).
//
// `poster_url` is a new field on EventConfig / EventMeta / CreateEventRequest /
// UpdateEventRequest. These tests pin the contract: it defaults to "" when
// absent (so existing payloads + pre-migration D1 rows keep working), and it
// round-trips through the request types. Mirrors the nft_image_url plumbing.
// ================================================================================================

#[test]
fn create_event_request_poster_url_defaults_empty_when_absent() {
    // Minimal create payload (only required `name` + `sheet_id` + timestamps).
    let json = r#"{"name":"E","sheet_id":"s","event_start_ms":1,"event_end_ms":2}"#;
    let req: CreateEventRequest = serde_json::from_str(json).expect("minimal payload should parse");
    assert_eq!(
        req.poster_url, "",
        "poster_url must default to empty when absent"
    );
}

#[test]
fn create_event_request_poster_url_round_trips() {
    let json = r#"{"name":"E","sheet_id":"s","event_start_ms":1,"event_end_ms":2,"poster_url":"/api/storage/posters/abc"}"#;
    let req: CreateEventRequest =
        serde_json::from_str(json).expect("payload with poster_url should parse");
    assert_eq!(req.poster_url, "/api/storage/posters/abc");
}

#[test]
fn update_event_request_poster_url_defaults_none_when_absent() {
    let req: UpdateEventRequest = serde_json::from_str("{}").expect("empty object should parse");
    assert_eq!(
        req.poster_url, None,
        "poster_url must default to None when absent"
    );
}

#[test]
fn update_event_request_poster_url_round_trips_set() {
    let json = r#"{"poster_url":"https://cdn.example.com/x.png"}"#;
    let req: UpdateEventRequest =
        serde_json::from_str(json).expect("payload with poster_url should parse");
    assert_eq!(
        req.poster_url.as_deref(),
        Some("https://cdn.example.com/x.png")
    );
}

#[test]
fn update_event_request_poster_url_round_trips_clear() {
    // Empty string is the documented "clear the field" sentinel.
    let json = r#"{"poster_url":""}"#;
    let req: UpdateEventRequest =
        serde_json::from_str(json).expect("payload with empty poster_url should parse");
    assert_eq!(req.poster_url.as_deref(), Some(""));
}

#[test]
fn event_config_poster_url_defaults_empty_when_absent() {
    // EventConfig has several required fields (no #[serde(default)]) — provide
    // the minimal set, omit poster_url, and confirm it defaults to "".
    let json = r#"{"id":"x","name":"E","slug":"x","tagline":"","link":"","status":"draft","event_start_ms":0,"event_end_ms":0,"sheet_id":"","sheet_name":"","staff_sheet_name":"","created_at":"","updated_at":""}"#;
    let cfg: EventConfig = serde_json::from_str(json).expect("minimal EventConfig should parse");
    assert_eq!(
        cfg.poster_url, "",
        "poster_url must default to empty when absent"
    );
    // Round-trip: empty poster_url is skip_serializing_if, so it won't appear in output.
    let reser = serde_json::to_string(&cfg).expect("serialize EventConfig");
    assert!(
        !reser.contains("poster_url"),
        "empty poster_url should be skipped on serialize, got: {reser}"
    );
}

#[test]
fn event_config_poster_url_round_trips_non_empty() {
    let json = r#"{"id":"x","name":"E","slug":"x","tagline":"","link":"","status":"draft","event_start_ms":0,"event_end_ms":0,"sheet_id":"","sheet_name":"","staff_sheet_name":"","created_at":"","updated_at":"","poster_url":"/api/storage/posters/x"}"#;
    let cfg: EventConfig =
        serde_json::from_str(json).expect("EventConfig with poster_url should parse");
    assert_eq!(cfg.poster_url, "/api/storage/posters/x");
    let reser = serde_json::to_string(&cfg).expect("serialize EventConfig");
    assert!(
        reser.contains("\"poster_url\":\"/api/storage/posters/x\""),
        "non-empty poster_url should appear in serialized output, got: {reser}"
    );
}

#[test]
fn update_event_request_default_impl_all_none() {
    // Plan 009 added `Default` to UpdateEventRequest so handlers can build a
    // partial update with `..Default::default()`. Verify every field is None.
    let req = UpdateEventRequest::default();
    assert_eq!(req.poster_url, None);
    assert_eq!(req.nft_image_url, None);
    assert_eq!(req.name, None);
}

// ================================================================================================
// recap_published — Plan 008 Phase 2 (event public recap).
//
// `recap_published` is a denormalized boolean on `EventConfig` + `EventMeta`
// mirroring `events.recap_published` (migration 0020). It defaults to `false`
// when absent so existing payloads + pre-migration D1 rows keep working.
// ================================================================================================

#[test]
fn event_config_recap_published_defaults_false_when_absent() {
    // Minimal EventConfig payload without `recap_published` — must deserialize
    // cleanly and default to `false`. Critical for backward compatibility with
    // any KV-stored configs written before Plan 008 Phase 2 landed.
    let json = r#"{"id":"x","name":"E","slug":"x","tagline":"","link":"","status":"draft","event_start_ms":0,"event_end_ms":0,"sheet_id":"","sheet_name":"","staff_sheet_name":"","created_at":"","updated_at":""}"#;
    let cfg: EventConfig = serde_json::from_str(json).expect("minimal EventConfig should parse");
    assert!(
        !cfg.recap_published,
        "recap_published must default to false when absent"
    );
}

#[test]
fn event_config_recap_published_round_trips_true() {
    // When explicitly set to true, the flag must round-trip through serde.
    // Unlike poster_url (skip_serializing_if empty), `recap_published: bool`
    // always serializes — verify it appears in the output.
    let json = r#"{"id":"x","name":"E","slug":"x","tagline":"","link":"","status":"draft","event_start_ms":0,"event_end_ms":0,"sheet_id":"","sheet_name":"","staff_sheet_name":"","created_at":"","updated_at":"","recap_published":true}"#;
    let cfg: EventConfig =
        serde_json::from_str(json).expect("EventConfig with recap_published should parse");
    assert!(cfg.recap_published);
    let reser = serde_json::to_string(&cfg).expect("serialize EventConfig");
    assert!(
        reser.contains("\"recap_published\":true"),
        "recap_published:true must appear in serialized output, got: {reser}"
    );
}

#[test]
fn event_config_recap_published_round_trips_false_explicit() {
    // Explicit `false` must round-trip too (distinguishes "set to false" from
    // "absent" on the wire — both deserialize to false, but the serialized
    // form should still carry the field for client visibility).
    let json = r#"{"id":"x","name":"E","slug":"x","tagline":"","link":"","status":"draft","event_start_ms":0,"event_end_ms":0,"sheet_id":"","sheet_name":"","staff_sheet_name":"","created_at":"","updated_at":"","recap_published":false}"#;
    let cfg: EventConfig =
        serde_json::from_str(json).expect("EventConfig with recap_published=false should parse");
    assert!(!cfg.recap_published);
    let reser = serde_json::to_string(&cfg).expect("serialize EventConfig");
    assert!(
        reser.contains("\"recap_published\":false"),
        "recap_published:false must appear in serialized output, got: {reser}"
    );
}

// ================================================================================================
// EventRecap — Plan 008 Phase 2 (recap content payload).
//
// `EventRecap` is returned by `GET /api/events/{id}/recap` (organizer) and
// embedded into `GET /api/public/event/{slug}/recap`. These tests pin the
// contract: defaults to empty/None when fields are absent, and round-trips
// the published + draft states.
// ================================================================================================

#[test]
fn event_recap_defaults_empty_when_fields_absent() {
    use event_checkin_domain::models::event_summary::EventRecap;

    // Only event_id is required; everything else should default.
    let json = r#"{"event_id":"evt-1"}"#;
    let recap: EventRecap = serde_json::from_str(json).expect("minimal EventRecap should parse");
    assert_eq!(recap.event_id, "evt-1");
    assert!(recap.recap_markdown.is_empty());
    assert!(recap.recap_image_url.is_empty());
    assert!(recap.recap_published_at.is_none());
    assert!(recap.frozen_at.is_none());
}

#[test]
fn event_recap_round_trips_published_state() {
    use event_checkin_domain::models::event_summary::EventRecap;

    // A fully-populated published recap. All five fields must round-trip.
    // Avoid `\n` (raw strings don't process escapes — would inject a literal
    // newline into the JSON and break parsing) and avoid `#` runs (would
    // close the raw string early). Plain text exercises the same code path.
    let json = r#"{"event_id":"evt-1","recap_markdown":"Great event with 50 devs.","recap_image_url":"https://cdn.example.com/hero.png","recap_published_at":"2026-07-01T12:00:00Z","frozen_at":"2026-06-30T23:59:59Z"}"#;
    let recap: EventRecap = serde_json::from_str(json).expect("published EventRecap should parse");
    assert_eq!(recap.event_id, "evt-1");
    assert_eq!(recap.recap_markdown, "Great event with 50 devs.");
    assert_eq!(recap.recap_image_url, "https://cdn.example.com/hero.png");
    assert_eq!(
        recap.recap_published_at.as_deref(),
        Some("2026-07-01T12:00:00Z")
    );
    assert_eq!(recap.frozen_at.as_deref(), Some("2026-06-30T23:59:59Z"));

    // Round-trip: skip_serializing_if only fires on None for the Option fields,
    // so all five should appear in the serialized output.
    let reser = serde_json::to_string(&recap).expect("serialize EventRecap");
    assert!(
        reser.contains("\"recap_published_at\":\"2026-07-01T12:00:00Z\""),
        "recap_published_at must appear in serialized output, got: {reser}"
    );
    assert!(
        reser.contains("\"frozen_at\":\"2026-06-30T23:59:59Z\""),
        "frozen_at must appear in serialized output, got: {reser}"
    );
}

#[test]
fn event_recap_round_trips_draft_state() {
    use event_checkin_domain::models::event_summary::EventRecap;

    // A draft (unpublished) recap. `recap_published_at` is None, so it must
    // be skipped on serialize (per `skip_serializing_if = "Option::is_none"`).
    let json = r#"{"event_id":"evt-1","recap_markdown":"Draft text","recap_image_url":"","recap_published_at":null,"frozen_at":null}"#;
    let recap: EventRecap = serde_json::from_str(json).expect("draft EventRecap should parse");
    assert_eq!(recap.recap_markdown, "Draft text");
    assert!(recap.recap_image_url.is_empty());
    assert!(recap.recap_published_at.is_none());
    assert!(recap.frozen_at.is_none());

    let reser = serde_json::to_string(&recap).expect("serialize draft EventRecap");
    assert!(
        !reser.contains("recap_published_at"),
        "draft recap must skip recap_published_at on serialize, got: {reser}"
    );
    assert!(
        !reser.contains("frozen_at"),
        "draft recap must skip frozen_at on serialize, got: {reser}"
    );
}

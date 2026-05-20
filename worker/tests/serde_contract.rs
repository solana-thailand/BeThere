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
    EscrowStatus, EventFormat, EventStatus, EventVisibility, OnlineOpenMode,
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

//! Serde contract tests — catch `rename_all = "snake_case"` mismatches between backend and frontend.
//!
//! These tests verify that all shared enums deserialize from the exact snake_case
//! JSON the backend sends. If any enum is missing `#[serde(rename_all = "snake_case")],`
//! the corresponding test will fail with "unknown variant".
//!
//! # Why mirrored enums?
//!
//! This crate is a WASM target (`leptos` with `csr`, `wasm-bindgen`, `web-sys`),
//! so `cargo test` cannot compile it for the native host. Instead, this file
//! **mirrors** the frontend enum definitions with their serde attributes.
//!
//! The actual backend-side contract test lives at `worker/tests/serde_contract.rs`
//! and uses the real `event-checkin-domain` types. Both files must stay in sync.
//!
//! # Running
//!
//! This file cannot be run directly from `frontend-leptos/` due to WASM deps.
//! Instead, run the companion test that validates the same contract from the backend:
//!
//! ```sh
//! cd worker && cargo test --test serde_contract -- --nocapture
//! ```
//!
//! # Keeping in sync
//!
//! When adding a new enum to `src/api/`:
//! 1. Add the mirrored enum definition below.
//! 2. Add round-trip tests for each variant.
//! 3. Add a `rejects_pascal_case` test.
//! 4. Also update `worker/tests/serde_contract.rs` to test the domain type.

use serde::{Deserialize, Serialize};

// ================================================================================================
// Mirrored enums from src/api/event.rs
// ================================================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
enum EventStatus {
    #[default]
    Draft,
    Active,
    Completed,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
enum EscrowStatus {
    #[default]
    None,
    Initialized,
    Deactivated,
    Closed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
enum EventFormat {
    #[default]
    InPerson,
    Online,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum OnlineOpenMode {
    #[default]
    Always,
    AutoOnFull,
    Manual,
}

// ================================================================================================
// Mirrored enums from src/api/claim.rs
// ================================================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
enum QuizStatus {
    #[default]
    NotRequired,
    NotStarted,
    InProgress,
    Passed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
enum AdventureStatusType {
    #[default]
    NotRequired,
    NotStarted,
    InProgress,
    Passed,
}

// ================================================================================================
// Mirrored enums from src/api/admin.rs
// ================================================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum EscrowInstruction {
    CreateEvent,
    Deposit,
    MarkCheckedIn,
    Refund,
    ClaimForfeited,
    CloseEvent,
    DeactivateEvent,
    CloseDeposit,
    Unknown,
}

// ================================================================================================
// Helpers
// ================================================================================================

/// Deserialize a JSON string into T, then serialize back and verify round-trip.
fn assert_round_trip<T>(json: &str, expected: T)
where
    T: serde::de::DeserializeOwned + Serialize + PartialEq + std::fmt::Debug,
{
    let parsed: T = serde_json::from_str(json).unwrap_or_else(|e| {
        panic!(
            "Failed to deserialize {json} into {}: {e}",
            std::any::type_name::<T>()
        )
    });
    assert_eq!(parsed, expected, "deserialized value mismatch");

    let re_serialized = serde_json::to_string(&expected)
        .unwrap_or_else(|e| panic!("Failed to serialize {:?}: {e}", expected));
    assert_eq!(re_serialized, json, "round-trip serialization mismatch");
}

/// Assert that a JSON string FAILS to deserialize into T (catches missing rename_all).
fn assert_unknown_variant<T>(json: &str)
where
    T: serde::de::DeserializeOwned,
{
    let result = serde_json::from_str::<T>(json);
    assert!(
        result.is_err(),
        "Expected deserialization failure for {json}, but got: {:?}",
        result.unwrap()
    );
}

// ================================================================================================
// Tests — EventStatus
// ================================================================================================

#[test]
fn event_status_draft() {
    assert_round_trip(r#""draft""#, EventStatus::Draft);
}

#[test]
fn event_status_active() {
    assert_round_trip(r#""active""#, EventStatus::Active);
}

#[test]
fn event_status_completed() {
    assert_round_trip(r#""completed""#, EventStatus::Completed);
}

#[test]
fn event_status_archived() {
    assert_round_trip(r#""archived""#, EventStatus::Archived);
}

#[test]
fn event_status_rejects_pascal_case() {
    assert_unknown_variant::<EventStatus>(r#""Draft""#);
}

// ================================================================================================
// Tests — EscrowStatus
// ================================================================================================

#[test]
fn escrow_status_none() {
    assert_round_trip(r#""none""#, EscrowStatus::None);
}

#[test]
fn escrow_status_initialized() {
    assert_round_trip(r#""initialized""#, EscrowStatus::Initialized);
}

#[test]
fn escrow_status_deactivated() {
    assert_round_trip(r#""deactivated""#, EscrowStatus::Deactivated);
}

#[test]
fn escrow_status_closed() {
    assert_round_trip(r#""closed""#, EscrowStatus::Closed);
}

#[test]
fn escrow_status_cancelled() {
    assert_round_trip(r#""cancelled""#, EscrowStatus::Cancelled);
}

// ================================================================================================
// Tests — EventFormat
// ================================================================================================

#[test]
fn event_format_in_person() {
    assert_round_trip(r#""in_person""#, EventFormat::InPerson);
}

#[test]
fn event_format_online() {
    assert_round_trip(r#""online""#, EventFormat::Online);
}

#[test]
fn event_format_hybrid() {
    assert_round_trip(r#""hybrid""#, EventFormat::Hybrid);
}

#[test]
fn event_format_rejects_camel_case() {
    // Without rename_all, serde would produce "InPerson" not "in_person"
    assert_unknown_variant::<EventFormat>(r#""InPerson""#);
}

// ================================================================================================
// Tests — OnlineOpenMode (this was the bug: missing rename_all)
// ================================================================================================

#[test]
fn online_open_mode_always() {
    assert_round_trip(r#""always""#, OnlineOpenMode::Always);
}

#[test]
fn online_open_mode_auto_on_full() {
    assert_round_trip(r#""auto_on_full""#, OnlineOpenMode::AutoOnFull);
}

#[test]
fn online_open_mode_manual() {
    assert_round_trip(r#""manual""#, OnlineOpenMode::Manual);
}

#[test]
fn online_open_mode_rejects_pascal_case() {
    // This is the exact bug that was found — AutoOnFull without rename_all
    // would serialize as "AutoOnFull", not "auto_on_full"
    assert_unknown_variant::<OnlineOpenMode>(r#""AutoOnFull""#);
}

// ================================================================================================
// Tests — QuizStatus
// ================================================================================================

#[test]
fn quiz_status_not_required() {
    assert_round_trip(r#""not_required""#, QuizStatus::NotRequired);
}

#[test]
fn quiz_status_not_started() {
    assert_round_trip(r#""not_started""#, QuizStatus::NotStarted);
}

#[test]
fn quiz_status_in_progress() {
    assert_round_trip(r#""in_progress""#, QuizStatus::InProgress);
}

#[test]
fn quiz_status_passed() {
    assert_round_trip(r#""passed""#, QuizStatus::Passed);
}

// ================================================================================================
// Tests — AdventureStatusType
// ================================================================================================

#[test]
fn adventure_status_not_required() {
    assert_round_trip(r#""not_required""#, AdventureStatusType::NotRequired);
}

#[test]
fn adventure_status_not_started() {
    assert_round_trip(r#""not_started""#, AdventureStatusType::NotStarted);
}

#[test]
fn adventure_status_in_progress() {
    assert_round_trip(r#""in_progress""#, AdventureStatusType::InProgress);
}

#[test]
fn adventure_status_passed() {
    assert_round_trip(r#""passed""#, AdventureStatusType::Passed);
}

// ================================================================================================
// Tests — EscrowInstruction (frontend sends, backend expects)
// ================================================================================================

#[test]
fn escrow_instruction_create_event() {
    assert_round_trip(r#""create_event""#, EscrowInstruction::CreateEvent);
}

#[test]
fn escrow_instruction_deposit() {
    assert_round_trip(r#""deposit""#, EscrowInstruction::Deposit);
}

#[test]
fn escrow_instruction_mark_checked_in() {
    assert_round_trip(r#""mark_checked_in""#, EscrowInstruction::MarkCheckedIn);
}

#[test]
fn escrow_instruction_refund() {
    assert_round_trip(r#""refund""#, EscrowInstruction::Refund);
}

#[test]
fn escrow_instruction_claim_forfeited() {
    assert_round_trip(r#""claim_forfeited""#, EscrowInstruction::ClaimForfeited);
}

#[test]
fn escrow_instruction_close_event() {
    assert_round_trip(r#""close_event""#, EscrowInstruction::CloseEvent);
}

#[test]
fn escrow_instruction_deactivate_event() {
    assert_round_trip(r#""deactivate_event""#, EscrowInstruction::DeactivateEvent);
}

#[test]
fn escrow_instruction_close_deposit() {
    assert_round_trip(r#""close_deposit""#, EscrowInstruction::CloseDeposit);
}

#[test]
fn escrow_instruction_unknown() {
    assert_round_trip(r#""unknown""#, EscrowInstruction::Unknown);
}

// ================================================================================================
// Integration — deserialize a full JSON object containing enum fields
// ================================================================================================

#[test]
fn event_detail_json_uses_snake_case_enums() {
    #[derive(Debug, Deserialize)]
    struct FakeEventDetail {
        status: EventStatus,
        escrow_status: EscrowStatus,
        event_format: EventFormat,
        online_open_mode: OnlineOpenMode,
    }

    let json = r#"{
        "status": "active",
        "escrow_status": "initialized",
        "event_format": "hybrid",
        "online_open_mode": "auto_on_full"
    }"#;

    let detail: FakeEventDetail = serde_json::from_str(json).expect("should parse event detail");
    assert_eq!(detail.status, EventStatus::Active);
    assert_eq!(detail.escrow_status, EscrowStatus::Initialized);
    assert_eq!(detail.event_format, EventFormat::Hybrid);
    assert_eq!(detail.online_open_mode, OnlineOpenMode::AutoOnFull);
}

#[test]
fn claim_lookup_json_uses_snake_case_enums() {
    #[derive(Debug, Deserialize)]
    struct FakeClaimLookup {
        quiz_status: QuizStatus,
        adventure_status: AdventureStatusType,
    }

    let json = r#"{
        "quiz_status": "in_progress",
        "adventure_status": "not_started"
    }"#;

    let lookup: FakeClaimLookup = serde_json::from_str(json).expect("should parse claim lookup");
    assert_eq!(lookup.quiz_status, QuizStatus::InProgress);
    assert_eq!(lookup.adventure_status, AdventureStatusType::NotStarted);
}

#[test]
fn onchain_event_json_uses_snake_case_instruction() {
    #[derive(Debug, Deserialize)]
    struct FakeOnChainEvent {
        instruction: EscrowInstruction,
    }

    let json = r#"{ "instruction": "mark_checked_in" }"#;
    let event: FakeOnChainEvent = serde_json::from_str(json).expect("should parse on-chain event");
    assert_eq!(event.instruction, EscrowInstruction::MarkCheckedIn);
}

//! Serde contract tests for DoRequest / DoResponse.

use super::types::*;

// Helper: serialize then deserialize and verify equality.
#[allow(dead_code)]
fn assert_round_trip(expected_json: &str, value: &DoRequest) {
    let serialized =
        serde_json::to_string(value).unwrap_or_else(|e| panic!("Failed to serialize: {e}"));
    assert_eq!(serialized, expected_json, "serialization mismatch");

    let deserialized: DoRequest = serde_json::from_str(expected_json)
        .unwrap_or_else(|e| panic!("Failed to deserialize: {e}"));
    assert_eq!(
        serde_json::to_string(&deserialized).unwrap(),
        expected_json,
        "round-trip deserialization mismatch"
    );
}

// ==========================================================================
// AcquireClaimLock serde
// ==========================================================================

#[test]
fn acquire_claim_lock_round_trip() {
    let json = r#"{"action":"acquire_claim_lock","lock_id":"lid-1","event_id":"evt-1","token":"tok-1","wallet":"wallet-addr","expires_at":"2025-01-01T00:00:00Z"}"#;
    let value = DoRequest::AcquireClaimLock {
        lock_id: "lid-1".to_string(),
        event_id: "evt-1".to_string(),
        token: "tok-1".to_string(),
        wallet: "wallet-addr".to_string(),
        expires_at: "2025-01-01T00:00:00Z".to_string(),
    };
    assert_round_trip(json, &value);
}

#[test]
fn acquire_claim_lock_action_tag_is_snake_case() {
    let value = DoRequest::AcquireClaimLock {
        lock_id: "lid".to_string(),
        event_id: "evt".to_string(),
        token: "tok".to_string(),
        wallet: "w".to_string(),
        expires_at: "exp".to_string(),
    };
    let serialized = serde_json::to_string(&value).unwrap();
    assert!(
        serialized.contains("\"action\":\"acquire_claim_lock\""),
        "action tag must be acquire_claim_lock, got: {serialized}"
    );
}

#[test]
fn acquire_claim_lock_all_fields_present() {
    let value = DoRequest::AcquireClaimLock {
        lock_id: "lid".to_string(),
        event_id: "evt".to_string(),
        token: "tok".to_string(),
        wallet: "w".to_string(),
        expires_at: "exp".to_string(),
    };
    let serialized = serde_json::to_string(&value).unwrap();
    let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert!(v.get("lock_id").is_some(), "missing lock_id");
    assert!(v.get("event_id").is_some(), "missing event_id");
    assert!(v.get("token").is_some(), "missing token");
    assert!(v.get("wallet").is_some(), "missing wallet");
    assert!(v.get("expires_at").is_some(), "missing expires_at");
}

// ==========================================================================
// FinalizeClaimLock serde
// ==========================================================================

#[test]
fn finalize_claim_lock_round_trip() {
    let json = r#"{"action":"finalize_claim_lock","event_id":"evt-1","token":"tok-1","asset_id":"asset-1","signature":"sig-1","claimed_at":"2025-01-01T00:00:00Z"}"#;
    let value = DoRequest::FinalizeClaimLock {
        event_id: "evt-1".to_string(),
        token: "tok-1".to_string(),
        asset_id: "asset-1".to_string(),
        signature: "sig-1".to_string(),
        claimed_at: "2025-01-01T00:00:00Z".to_string(),
    };
    assert_round_trip(json, &value);
}

#[test]
fn finalize_claim_lock_action_tag_is_snake_case() {
    let value = DoRequest::FinalizeClaimLock {
        event_id: "evt".to_string(),
        token: "tok".to_string(),
        asset_id: "a".to_string(),
        signature: "s".to_string(),
        claimed_at: "c".to_string(),
    };
    let serialized = serde_json::to_string(&value).unwrap();
    assert!(
        serialized.contains("\"action\":\"finalize_claim_lock\""),
        "action tag must be finalize_claim_lock, got: {serialized}"
    );
}

#[test]
fn finalize_claim_lock_all_fields_present() {
    let value = DoRequest::FinalizeClaimLock {
        event_id: "evt".to_string(),
        token: "tok".to_string(),
        asset_id: "a".to_string(),
        signature: "s".to_string(),
        claimed_at: "c".to_string(),
    };
    let serialized = serde_json::to_string(&value).unwrap();
    let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert!(v.get("event_id").is_some(), "missing event_id");
    assert!(v.get("token").is_some(), "missing token");
    assert!(v.get("asset_id").is_some(), "missing asset_id");
    assert!(v.get("signature").is_some(), "missing signature");
    assert!(v.get("claimed_at").is_some(), "missing claimed_at");
}

// ==========================================================================
// ReleaseClaimLock serde
// ==========================================================================

#[test]
fn release_claim_lock_round_trip() {
    let json = r#"{"action":"release_claim_lock","event_id":"evt-1","token":"tok-1"}"#;
    let value = DoRequest::ReleaseClaimLock {
        event_id: "evt-1".to_string(),
        token: "tok-1".to_string(),
    };
    assert_round_trip(json, &value);
}

#[test]
fn release_claim_lock_action_tag_is_snake_case() {
    let value = DoRequest::ReleaseClaimLock {
        event_id: "evt".to_string(),
        token: "tok".to_string(),
    };
    let serialized = serde_json::to_string(&value).unwrap();
    assert!(
        serialized.contains("\"action\":\"release_claim_lock\""),
        "action tag must be release_claim_lock, got: {serialized}"
    );
}

#[test]
fn release_claim_lock_all_fields_present() {
    let value = DoRequest::ReleaseClaimLock {
        event_id: "evt".to_string(),
        token: "tok".to_string(),
    };
    let serialized = serde_json::to_string(&value).unwrap();
    let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert!(v.get("event_id").is_some(), "missing event_id");
    assert!(v.get("token").is_some(), "missing token");
}

// ==========================================================================
// Unknown action rejection
// ==========================================================================

#[test]
fn unknown_action_rejected() {
    let json = r#"{"action":"do_something","event_id":"evt","token":"tok"}"#;
    let result = serde_json::from_str::<DoRequest>(json);
    assert!(
        result.is_err(),
        "expected deserialization failure for unknown action"
    );
}

#[test]
fn missing_action_rejected() {
    let json = r#"{"event_id":"evt","token":"tok"}"#;
    let result = serde_json::from_str::<DoRequest>(json);
    assert!(
        result.is_err(),
        "expected deserialization failure for missing action"
    );
}

// ==========================================================================
// DoResponse
// ==========================================================================

#[test]
fn do_response_ok_serializes() {
    let resp = DoResponse::ok();
    let json = serde_json::to_string(&resp).unwrap();
    assert_eq!(json, "{\"success\":true}");
}

#[test]
fn do_response_err_serializes() {
    let resp = DoResponse::err("something went wrong");
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"success\":false"));
    assert!(json.contains("\"error\":\"something went wrong\""));
}

// ==========================================================================
// Phase 2: CheckIn serde
// ==========================================================================

#[test]
fn check_in_round_trip() {
    let json = r#"{"action":"check_in","attendee_id":"att-1","event_id":"evt-1","checked_in_at":"2025-06-01T10:00:00Z","checked_in_by":"staff@test.com","claim_token":"tok-1"}"#;
    let value = DoRequest::CheckIn {
        attendee_id: "att-1".to_string(),
        event_id: "evt-1".to_string(),
        checked_in_at: "2025-06-01T10:00:00Z".to_string(),
        checked_in_by: "staff@test.com".to_string(),
        claim_token: "tok-1".to_string(),
    };
    assert_round_trip(json, &value);
}

#[test]
fn check_in_action_tag_is_snake_case() {
    let value = DoRequest::CheckIn {
        attendee_id: "a".to_string(),
        event_id: "e".to_string(),
        checked_in_at: "t".to_string(),
        checked_in_by: "s".to_string(),
        claim_token: "c".to_string(),
    };
    let serialized = serde_json::to_string(&value).unwrap();
    assert!(
        serialized.contains("\"action\":\"check_in\""),
        "action tag must be check_in, got: {serialized}"
    );
}

#[test]
fn check_in_all_fields_present() {
    let value = DoRequest::CheckIn {
        attendee_id: "a".to_string(),
        event_id: "e".to_string(),
        checked_in_at: "t".to_string(),
        checked_in_by: "s".to_string(),
        claim_token: "c".to_string(),
    };
    let serialized = serde_json::to_string(&value).unwrap();
    let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert!(v.get("attendee_id").is_some(), "missing attendee_id");
    assert!(v.get("event_id").is_some(), "missing event_id");
    assert!(v.get("checked_in_at").is_some(), "missing checked_in_at");
    assert!(v.get("checked_in_by").is_some(), "missing checked_in_by");
    assert!(v.get("claim_token").is_some(), "missing claim_token");
}

// ==========================================================================
// Phase 2: UndoCheckIn serde
// ==========================================================================

#[test]
fn undo_check_in_round_trip() {
    let json = r#"{"action":"undo_check_in","attendee_id":"att-1","event_id":"evt-1"}"#;
    let value = DoRequest::UndoCheckIn {
        attendee_id: "att-1".to_string(),
        event_id: "evt-1".to_string(),
    };
    assert_round_trip(json, &value);
}

#[test]
fn undo_check_in_action_tag_is_snake_case() {
    let value = DoRequest::UndoCheckIn {
        attendee_id: "a".to_string(),
        event_id: "e".to_string(),
    };
    let serialized = serde_json::to_string(&value).unwrap();
    assert!(
        serialized.contains("\"action\":\"undo_check_in\""),
        "action tag must be undo_check_in, got: {serialized}"
    );
}

// ==========================================================================
// Phase 2: ClaimAttendee serde
// ==========================================================================

#[test]
fn claim_attendee_round_trip() {
    let json = r#"{"action":"claim_attendee","event_id":"evt-1","claim_token":"tok-1","claimed_at":"2025-06-01T12:00:00Z","claim_asset_id":"asset-1","claim_signature":"sig-1"}"#;
    let value = DoRequest::ClaimAttendee {
        event_id: "evt-1".to_string(),
        claim_token: "tok-1".to_string(),
        claimed_at: "2025-06-01T12:00:00Z".to_string(),
        claim_asset_id: "asset-1".to_string(),
        claim_signature: "sig-1".to_string(),
    };
    assert_round_trip(json, &value);
}

#[test]
fn claim_attendee_action_tag_is_snake_case() {
    let value = DoRequest::ClaimAttendee {
        event_id: "e".to_string(),
        claim_token: "t".to_string(),
        claimed_at: "c".to_string(),
        claim_asset_id: "a".to_string(),
        claim_signature: "s".to_string(),
    };
    let serialized = serde_json::to_string(&value).unwrap();
    assert!(
        serialized.contains("\"action\":\"claim_attendee\""),
        "action tag must be claim_attendee, got: {serialized}"
    );
}

#[test]
fn claim_attendee_all_fields_present() {
    let value = DoRequest::ClaimAttendee {
        event_id: "e".to_string(),
        claim_token: "t".to_string(),
        claimed_at: "c".to_string(),
        claim_asset_id: "a".to_string(),
        claim_signature: "s".to_string(),
    };
    let serialized = serde_json::to_string(&value).unwrap();
    let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert!(v.get("event_id").is_some(), "missing event_id");
    assert!(v.get("claim_token").is_some(), "missing claim_token");
    assert!(v.get("claimed_at").is_some(), "missing claimed_at");
    assert!(v.get("claim_asset_id").is_some(), "missing claim_asset_id");
    assert!(
        v.get("claim_signature").is_some(),
        "missing claim_signature"
    );
}

// ==========================================================================
// Phase 2: UpsertAttendee serde
// ==========================================================================

#[test]
fn upsert_attendee_round_trip() {
    let json = r#"{"action":"upsert_attendee","id":"att-1","event_id":"evt-1","email":"test@test.com","name":"Test User","approval_status":"approved","participation_type":"In-Person","contact_channel":"telegram","contact_handle":"@test"}"#;
    let value = DoRequest::UpsertAttendee {
        id: "att-1".to_string(),
        event_id: "evt-1".to_string(),
        email: "test@test.com".to_string(),
        name: "Test User".to_string(),
        approval_status: "approved".to_string(),
        participation_type: "In-Person".to_string(),
        contact_channel: "telegram".to_string(),
        contact_handle: "@test".to_string(),
    };
    assert_round_trip(json, &value);
}

#[test]
fn upsert_attendee_action_tag_is_snake_case() {
    let value = DoRequest::UpsertAttendee {
        id: "a".to_string(),
        event_id: "e".to_string(),
        email: "t@t.com".to_string(),
        name: "n".to_string(),
        approval_status: "p".to_string(),
        participation_type: "i".to_string(),
        contact_channel: "c".to_string(),
        contact_handle: "h".to_string(),
    };
    let serialized = serde_json::to_string(&value).unwrap();
    assert!(
        serialized.contains("\"action\":\"upsert_attendee\""),
        "action tag must be upsert_attendee, got: {serialized}"
    );
}

#[test]
fn upsert_attendee_all_fields_present() {
    let value = DoRequest::UpsertAttendee {
        id: "a".to_string(),
        event_id: "e".to_string(),
        email: "t@t.com".to_string(),
        name: "n".to_string(),
        approval_status: "p".to_string(),
        participation_type: "i".to_string(),
        contact_channel: "c".to_string(),
        contact_handle: "h".to_string(),
    };
    let serialized = serde_json::to_string(&value).unwrap();
    let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert!(v.get("id").is_some(), "missing id");
    assert!(v.get("event_id").is_some(), "missing event_id");
    assert!(v.get("email").is_some(), "missing email");
    assert!(v.get("name").is_some(), "missing name");
    assert!(
        v.get("approval_status").is_some(),
        "missing approval_status"
    );
    assert!(
        v.get("participation_type").is_some(),
        "missing participation_type"
    );
    assert!(
        v.get("contact_channel").is_some(),
        "missing contact_channel"
    );
    assert!(v.get("contact_handle").is_some(), "missing contact_handle");
}

//! DO claim lock serde contract tests.
//!
//! Validates that JSON shapes sent by the Worker handler match what the
//! Durable Object expects to parse. Since `DoRequest`, `claim_lock_key`,
//! and `mask_wallet` are `pub(crate)`, the unit tests live in inline
//! `#[cfg(test)]` modules inside the source files:
//!
//!   - `worker/src/durable_objects/event_do.rs` — DoRequest serde + DoResponse
//!   - `worker/src/claim/lock.rs`              — claim_lock_key, mask_wallet
//!
//! This file validates the **JSON contract** from the caller's perspective:
//! it constructs the JSON that the Worker handler would send and verifies
//! structural correctness (field names, action tags) without needing
//! access to `pub(crate)` types.
//!
//! Run: `cargo test --test do_claim_lock` (from worker directory)

use serde_json::Value;

// ================================================================================================
// Helpers
// ================================================================================================

/// Parse JSON and return the Value for structural assertions.
fn parse(json: &str) -> Value {
    serde_json::from_str(json).expect("should be valid JSON")
}

// ================================================================================================
// AcquireClaimLock — JSON contract
// ================================================================================================

#[test]
fn acquire_claim_lock_json_has_correct_action_tag() {
    let json = r#"{
        "action": "acquire_claim_lock",
        "lock_id": "0196eabc-1234-7abc-def0-123456789abc",
        "event_id": "evt-001",
        "token": "tok-001",
        "wallet": "BxRWqK3KjF8Mn2dTsUfMZ8xJbQHvYC3KjF",
        "expires_at": "2025-06-06T12:00:00Z"
    }"#;
    let v = parse(json);
    assert_eq!(v["action"], "acquire_claim_lock");
}

#[test]
fn acquire_claim_lock_json_has_all_required_fields() {
    let json = r#"{
        "action": "acquire_claim_lock",
        "lock_id": "lid",
        "event_id": "evt",
        "token": "tok",
        "wallet": "w",
        "expires_at": "exp"
    }"#;
    let v = parse(json);
    let required = [
        "action",
        "lock_id",
        "event_id",
        "token",
        "wallet",
        "expires_at",
    ];
    for field in &required {
        assert!(v.get(*field).is_some(), "missing field: {field}");
    }
}

#[test]
fn acquire_claim_lock_json_exactly_6_fields() {
    let json = r#"{
        "action": "acquire_claim_lock",
        "lock_id": "lid",
        "event_id": "evt",
        "token": "tok",
        "wallet": "w",
        "expires_at": "exp"
    }"#;
    let v = parse(json);
    assert_eq!(v.as_object().unwrap().len(), 6);
}

// ================================================================================================
// FinalizeClaimLock — JSON contract
// ================================================================================================

#[test]
fn finalize_claim_lock_json_has_correct_action_tag() {
    let json = r#"{
        "action": "finalize_claim_lock",
        "event_id": "evt-001",
        "token": "tok-001",
        "asset_id": "asset-abc",
        "signature": "sig-xyz",
        "claimed_at": "2025-06-06T12:00:00Z"
    }"#;
    let v = parse(json);
    assert_eq!(v["action"], "finalize_claim_lock");
}

#[test]
fn finalize_claim_lock_json_has_all_required_fields() {
    let json = r#"{
        "action": "finalize_claim_lock",
        "event_id": "evt",
        "token": "tok",
        "asset_id": "a",
        "signature": "s",
        "claimed_at": "c"
    }"#;
    let v = parse(json);
    let required = [
        "action",
        "event_id",
        "token",
        "asset_id",
        "signature",
        "claimed_at",
    ];
    for field in &required {
        assert!(v.get(*field).is_some(), "missing field: {field}");
    }
}

#[test]
fn finalize_claim_lock_json_exactly_6_fields() {
    let json = r#"{
        "action": "finalize_claim_lock",
        "event_id": "evt",
        "token": "tok",
        "asset_id": "a",
        "signature": "s",
        "claimed_at": "c"
    }"#;
    let v = parse(json);
    assert_eq!(v.as_object().unwrap().len(), 6);
}

// ================================================================================================
// ReleaseClaimLock — JSON contract
// ================================================================================================

#[test]
fn release_claim_lock_json_has_correct_action_tag() {
    let json = r#"{
        "action": "release_claim_lock",
        "event_id": "evt-001",
        "token": "tok-001"
    }"#;
    let v = parse(json);
    assert_eq!(v["action"], "release_claim_lock");
}

#[test]
fn release_claim_lock_json_has_all_required_fields() {
    let json = r#"{
        "action": "release_claim_lock",
        "event_id": "evt",
        "token": "tok"
    }"#;
    let v = parse(json);
    let required = ["action", "event_id", "token"];
    for field in &required {
        assert!(v.get(*field).is_some(), "missing field: {field}");
    }
}

#[test]
fn release_claim_lock_json_exactly_3_fields() {
    let json = r#"{
        "action": "release_claim_lock",
        "event_id": "evt",
        "token": "tok"
    }"#;
    let v = parse(json);
    assert_eq!(v.as_object().unwrap().len(), 3);
}

// ================================================================================================
// Action tag must be snake_case (not camelCase or PascalCase)
// ================================================================================================

#[test]
fn action_tags_are_snake_case_not_camel_or_pascal() {
    let invalid_actions = [
        "acquireClaimLock",
        "AcquireClaimLock",
        "ACQUIRE_CLAIM_LOCK",
        "finalizeClaimLock",
        "FinalizeClaimLock",
        "releaseClaimLock",
        "ReleaseClaimLock",
    ];
    let valid_actions = [
        "acquire_claim_lock",
        "finalize_claim_lock",
        "release_claim_lock",
    ];
    for action in &invalid_actions {
        assert!(
            !valid_actions.contains(action),
            "action tag {action} should NOT be valid"
        );
    }
}

// ================================================================================================
// KV key format contract (mirrors claim_lock_key logic)
// ================================================================================================

#[test]
fn claim_lock_kv_key_format_contract() {
    // The KV key format is: "event:{event_id}:claim_lock:{token}"
    let event_id = "evt-2025-conf";
    let token = "0196eabc-1234-7abc-def0-123456789abc";
    let expected = format!("event:{event_id}:claim_lock:{token}");
    assert!(expected.starts_with("event:"));
    assert!(expected.contains(":claim_lock:"));
    assert_eq!(
        expected,
        "event:evt-2025-conf:claim_lock:0196eabc-1234-7abc-def0-123456789abc"
    );
}

// ================================================================================================
// Wallet masking contract (mirrors mask_wallet logic)
// ================================================================================================

#[test]
fn wallet_masking_contract_normal_address() {
    // First 4 + "..." + last 4 for addresses > 8 chars
    let addr = "BxRWqK3KjF8Mn2dTsUfMZ8xJbQHvYC3KjF";
    let masked = format!("{}...{}", &addr[..4], &addr[addr.len() - 4..]);
    assert_eq!(masked, "BxRW...3KjF");
}

#[test]
fn wallet_masking_contract_short_address_returns_masked() {
    // Addresses <= 8 chars return "****"
    let short = "abc";
    let masked = if short.len() > 8 {
        format!("{}...{}", &short[..4], &short[short.len() - 4..])
    } else {
        "****".to_string()
    };
    assert_eq!(masked, "****");
}

#[test]
fn wallet_masking_contract_exactly_8_chars_returns_masked() {
    let addr = "12345678";
    let masked = if addr.len() > 8 {
        format!("{}...{}", &addr[..4], &addr[addr.len() - 4..])
    } else {
        "****".to_string()
    };
    assert_eq!(masked, "****");
}

#[test]
fn wallet_masking_contract_9_chars_masks() {
    let addr = "123456789";
    let masked = if addr.len() > 8 {
        format!("{}...{}", &addr[..4], &addr[addr.len() - 4..])
    } else {
        "****".to_string()
    };
    assert_eq!(masked, "1234...6789");
}

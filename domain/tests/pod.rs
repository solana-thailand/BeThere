//! Plan 014 — Phase 1.2b: Zero-copy wire regression tests.
//!
//! Validates the three production Pod types (`FunnelSnapshot`,
//! `FinancialSnapshot`, `LevelScore`) against the shared envelope in
//! [`event_checkin_domain::wire`]. The envelope itself has its own unit tests
//! in `domain/src/wire.rs`; this file focuses on the type-specific round-trips
//! and tamper detection.
//!
//! Run: `cargo test -p event-checkin-domain --features wire --test pod`

#![cfg(feature = "wire")]

use bytemuck::{Pod, Zeroable};
use event_checkin_domain::models::adventure::LevelScore;
use event_checkin_domain::models::event_summary::{FinancialSnapshot, FunnelSnapshot};
use event_checkin_domain::wire::{self, WireError};

/// Assert at compile time that the layout is what we documented in the audit.
///
/// `LevelScore` = 3 × u32 (12) + u8 (1) + 3 explicit pad = 16 bytes under `repr(C)`.
/// `FunnelSnapshot` = 9 × u64 = 72 bytes.
/// `FinancialSnapshot` = 4 × u64 = 32 bytes.
const _: () = {
    assert!(size_of::<LevelScore>() == 16);
    assert!(size_of::<FunnelSnapshot>() == 72);
    assert!(size_of::<FinancialSnapshot>() == 32);
};

/// Compile-time proof that all three types satisfy `Pod` + `Zeroable`.
const _: () = {
    #[allow(dead_code)]
    fn assert_pod<T: Pod + Zeroable>() {}
    #[allow(dead_code)]
    fn _proofs() {
        assert_pod::<LevelScore>();
        assert_pod::<FunnelSnapshot>();
        assert_pod::<FinancialSnapshot>();
    }
};

// ---------------------------------------------------------------------------
// Round-trip tests — encode via shared `wire::pack`, decode via `wire::unpack`.
// ---------------------------------------------------------------------------

#[test]
fn level_score_round_trip_is_lossless() {
    let original = LevelScore {
        moves: 42,
        puzzles_solved: 7,
        time_seconds: 180,
        stars: 3,
        _pad: [0; 3],
    };
    let encoded = wire::pack(&original);
    let decoded: &LevelScore = wire::unpack(&encoded).expect("round-trip");
    assert_eq!(decoded, &original);
}

#[test]
fn funnel_snapshot_round_trip_is_lossless() {
    let original = FunnelSnapshot {
        registered_count: 200,
        deposited_count: 150,
        checked_in_count: 120,
        no_show_count: 30,
        claimed_count: 100,
        refunded_count: 5,
        post_event_reg_count: 0,
        in_person_registered_count: 180,
        in_person_checked_in_count: 120,
    };
    let encoded = wire::pack(&original);
    let decoded: &FunnelSnapshot = wire::unpack(&encoded).expect("round-trip");
    assert_eq!(decoded, &original);
}

#[test]
fn financial_snapshot_round_trip_is_lossless() {
    let original = FinancialSnapshot {
        usdc_deposited_total: 1_500_000_000, // 1500 USDC
        usdc_refunded_total: 0,
        thb_deposited_total: 150000, // 1500 THB in satang
        thb_refunded_total: 5000,
    };
    let encoded = wire::pack(&original);
    let decoded: &FinancialSnapshot = wire::unpack(&encoded).expect("round-trip");
    assert_eq!(decoded, &original);
}

// ---------------------------------------------------------------------------
// Envelope integrity — verifies the shared `wire` module rejects the failure
// modes a real Worker receiver must handle. These are type-parameterized
// regression tests on top of `wire.rs`'s own unit tests.
// ---------------------------------------------------------------------------

#[test]
fn truncated_body_is_rejected() {
    let value = LevelScore::default();
    let mut encoded = wire::pack(&value);
    encoded.truncate(encoded.len() - 1);
    assert!(matches!(
        wire::unpack::<LevelScore>(&encoded),
        Err(WireError::Truncated { .. })
    ));
}

#[test]
fn wrong_magic_is_rejected() {
    let value = LevelScore::default();
    let mut encoded = wire::pack(&value);
    encoded[0] = b'X'; // corrupt magic
    assert_eq!(
        wire::unpack::<LevelScore>(&encoded),
        Err(WireError::BadMagic)
    );
}

#[test]
fn version_skew_is_rejected() {
    let value = LevelScore::default();
    let mut encoded = wire::pack(&value);
    encoded[4] = 99; // a future version the receiver doesn't know
    assert_eq!(
        wire::unpack::<LevelScore>(&encoded),
        Err(WireError::UnsupportedVersion { got: 99, want: 1 })
    );
}

#[test]
fn payload_corruption_invalidates_hash() {
    let value = LevelScore {
        moves: 1,
        puzzles_solved: 1,
        time_seconds: 1,
        stars: 1,
        _pad: [0; 3],
    };
    let mut encoded = wire::pack(&value);
    // Flip a byte inside the payload (after the 8-byte header).
    encoded[wire::WIRE_HEADER_LEN] ^= 0xFF;
    assert_eq!(
        wire::unpack::<LevelScore>(&encoded),
        Err(WireError::HashMismatch)
    );
}

#[test]
fn default_zeroed_value_is_valid_pod() {
    // Zeroable contract: an all-zero byte pattern must be a valid value.
    // The wire envelope's reserved bytes are zeroed on encode, so a zeroed
    // payload must round-trip to `T::default()`.
    let zeroed = LevelScore::default();
    let encoded = wire::pack(&zeroed);
    let decoded: &LevelScore = wire::unpack(&encoded).expect("round-trip");
    assert_eq!(decoded, &LevelScore::default());
}

#[test]
fn envelope_size_matches_documented_layout() {
    let encoded = wire::pack(&LevelScore::default());
    // header(8) + payload(16) + blake3(32) = 56 bytes.
    assert_eq!(
        encoded.len(),
        wire::WIRE_HEADER_LEN + size_of::<LevelScore>() + wire::WIRE_TAG_LEN
    );
    assert_eq!(encoded.len(), 56);
}

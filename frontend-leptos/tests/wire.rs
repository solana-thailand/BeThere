//! Plan 014 — Phase 2.0: Frontend ↔ domain wire-format reachability smoke test.
//!
//! Purpose: prove that `frontend-leptos` (excluded from the workspace, built
//! under `trunk` for `wasm32-unknown-unknown`) can import the shared `domain`
//! crate and reach the `*Wire` Pod types that the worker will ship on
//! `?fmt=bin` endpoints (Plan 014 Phase 1.3+).
//!
//! This test does NOT depend on a running worker. It exercises the shared
//! envelope from `event_checkin_domain::wire` — the same module the worker
//! uses to encode — so any drift between encoder and decoder fails here.
//!
//! Run (native):  `cargo test -p event-checkin-frontend --test wire`
//! Run (wasm):    `wasm-pack test --node --test wire`

use event_checkin_domain::models::adventure::LevelScore;
use event_checkin_domain::models::event_summary::{FinancialSnapshot, FunnelSnapshot};
use event_checkin_domain::wire::{self, WireError};

/// Compile-time layout assertions — must match `domain/tests/pod.rs`.
const _: () = {
    assert!(size_of::<LevelScore>() == 16);
    assert!(size_of::<FunnelSnapshot>() == 72);
    assert!(size_of::<FinancialSnapshot>() == 32);
};

#[test]
fn frontend_can_reach_domain_wire_types() {
    // The single assertion that matters: the frontend crate can name the
    // domain's Pod types and round-trip them through the shared envelope.
    // If this compiles, Blocker A from the audit is resolved — Phase 1.3
    // has a decode path.
    let original = LevelScore {
        moves: 7,
        puzzles_solved: 2,
        time_seconds: 45,
        stars: 2,
        _pad: [0; 3],
    };
    let encoded = wire::pack(&original);
    let decoded: &LevelScore = wire::unpack(&encoded).expect("round-trip");
    assert_eq!(decoded, &original);
}

#[test]
fn frontend_decodes_worker_shaped_funnel_snapshot() {
    // Simulates: worker encodes a `FunnelSnapshot` via `wire::pack`, frontend
    // receives the bytes and decodes zero-copy via `wire::unpack`. This is
    // the exact shape the Phase 1.3 smoke endpoint will produce.
    let worker_side = FunnelSnapshot {
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
    let wire_bytes = wire::pack(&worker_side);

    // Frontend decode — note: zero allocation, just a cast.
    let frontend_side: &FunnelSnapshot = wire::unpack(&wire_bytes).expect("decode");
    assert_eq!(frontend_side, &worker_side);
    assert_eq!(frontend_side.checked_in_count, 120);
    assert_eq!(frontend_side.no_show_count, 30);
}

#[test]
fn frontend_rejects_tampered_wire_payload() {
    let original = FinancialSnapshot {
        usdc_deposited_total: 1_500_000_000,
        usdc_refunded_total: 0,
        thb_deposited_total: 150000,
        thb_refunded_total: 5000,
    };
    let mut encoded = wire::pack(&original);
    // Flip a payload byte — BLAKE3 must catch it.
    encoded[wire::WIRE_HEADER_LEN] ^= 0xFF;
    assert_eq!(
        wire::unpack::<FinancialSnapshot>(&encoded),
        Err(WireError::HashMismatch)
    );
}

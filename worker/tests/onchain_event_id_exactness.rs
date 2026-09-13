//! The on-chain event id must survive storage and retrieval byte-exactly.
//!
//! Issue 085. `events.on_chain_event_id` is a **u64** that seeds the
//! `EventEscrow` PDA, and every escrow transaction builder re-derives that PDA
//! from it (`solana_escrow::tx_builders::EscrowCtx::resolve`). A value that is
//! off by even one is not "slightly wrong" — it addresses an account that was
//! never created, and deposit, refund, close and rollover all fail with
//! `IllegalOwner` after ~195 compute units.
//!
//! Two precision bugs made that the normal case:
//!
//!   1. **Write** — above `i64::MAX` the value is not an INTEGER literal
//!      SQLite can store, so it silently became a REAL.
//!   2. **Read** — rows reach Rust via `JSON.stringify`, so anything above
//!      `2^53` arrives already rounded.
//!
//! These tests pin the boundaries and the real production values, so a
//! regression is caught here rather than by a failed transaction simulation.

/// SQLite integers are signed; above this a value cannot be stored as INTEGER.
const I64_MAX: u64 = 9_223_372_036_854_775_807;

/// float64 holds integers exactly only up to here.
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_992;

/// The generator, mirrored from `worker/src/handlers/deposit/mod.rs`.
///
/// Deliberately re-implemented rather than imported: this asserts the *values*
/// production depends on, so it must fail if the hash itself ever changes —
/// changing it would orphan every existing escrow account on-chain.
fn derive_on_chain_event_id(event_id: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in event_id.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    match hash {
        0 => 1,
        other => other,
    }
}

/// Real ids from production and staging, with the values observed after each
/// corruption path. These are the regression anchors.
#[test]
fn production_event_ids_are_reproducible_from_their_slug() {
    let cases = [
        // (event id, true FNV-1a value)
        ("islanddao-v4-demo", 15_159_210_065_911_598_203_u64),
        ("flow-test-event", 5_055_890_856_068_877_793),
    ];
    for (event_id, expected) in cases {
        assert_eq!(
            derive_on_chain_event_id(event_id),
            expected,
            "{event_id}: the on-chain id changed — every existing escrow PDA \
             for this event would be orphaned"
        );
    }
}

/// `islanddao-v4-demo` is the production event that was actually corrupted.
#[test]
fn the_corrupted_production_value_is_not_the_real_one() {
    let truth = derive_on_chain_event_id("islanddao-v4-demo");
    // What SQLite stored after the REAL fallback, read back.
    let stored_as_real = 15_159_210_065_911_599_104_u64;
    assert_ne!(truth, stored_as_real);
    assert!(
        truth > I64_MAX,
        "this event only corrupted because its id exceeds i64::MAX"
    );
}

/// The read path rounds above 2^53 even when the database is perfectly correct.
#[test]
fn values_above_the_float_safe_range_do_not_survive_a_json_round_trip() {
    // Observed: D1 holds 5_055_890_856_068_877_793, the Worker received
    // 5_055_890_856_068_877_000.
    let exact = derive_on_chain_event_id("flow-test-event");
    assert!(exact > MAX_SAFE_INTEGER, "the premise requires a large id");

    let through_float64 = exact as f64 as u64;
    assert_ne!(
        exact, through_float64,
        "if this ever passes, the read path is safe and the TEXT column is \
         no longer load-bearing"
    );

    // TEXT survives, which is why migration 0034 exists.
    let through_text: u64 = exact.to_string().parse().expect("round trips");
    assert_eq!(exact, through_text);
}

/// How common is this? Not an edge case — the hash output is spread across the
/// whole u64 range, so most real event ids land in the unsafe region.
#[test]
fn most_event_ids_fall_outside_the_safe_range() {
    let sample: Vec<u64> = (0..200)
        .map(|n| derive_on_chain_event_id(&format!("solana-thailand-event-{n}")))
        .collect();

    let unsafe_for_float = sample.iter().filter(|v| **v > MAX_SAFE_INTEGER).count();
    let unsafe_for_sqlite = sample.iter().filter(|v| **v > I64_MAX).count();

    assert!(
        unsafe_for_float > 190,
        "expected nearly every id to exceed 2^53, got {unsafe_for_float}/200 — \
         if this drops, re-check the generator"
    );
    assert!(
        (60..=140).contains(&unsafe_for_sqlite),
        "expected roughly half to exceed i64::MAX, got {unsafe_for_sqlite}/200"
    );
}

/// Migration 0034 must exist and add the exact column, since the reader now
/// depends on it.
#[test]
fn the_exact_column_migration_is_present() {
    let migration = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/0034_on_chain_event_id_text.sql"
    ))
    .expect("migration 0034 exists");

    assert!(
        migration.contains("on_chain_event_id_text TEXT"),
        "0034 must add the TEXT column the reader prefers"
    );
}

/// The writer must persist the exact value as a quoted literal, and the reader
/// must prefer it. A guard, because both halves are easy to drop silently.
#[test]
fn the_writer_and_reader_both_use_the_exact_column() {
    let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/db/events.rs"))
        .expect("events.rs is readable");

    assert!(
        source.contains("'{on_chain_event_id}'"),
        "upsert_event must write the id as a quoted TEXT literal, or SQLite \
         will store a value above i64::MAX as REAL again"
    );
    assert!(
        source.contains("fn exact_on_chain_event_id"),
        "the reader must resolve the id through the exact column"
    );
    assert!(
        !source.contains("on_chain_event_id: self.on_chain_event_id.unwrap_or(0)"),
        "to_event_config must not read the rounded numeric column directly"
    );
}

//! Pure on-chain identifiers shared by the worker and its tests.

/// Map a string event id to the `u64` that seeds the escrow `EventEscrow` PDA.
///
/// FNV-1a 64-bit, with 0 mapped to 1. Never change this: the bethere-escrow
/// program uses `event_id: u64` as a PDA seed, so a different hash re-derives
/// every PDA and orphans the existing escrow accounts. FNV-1a is enough here:
///   1. inputs are UUIDs or slugs, and the organizer pubkey is also a seed;
///   2. PDA seeds are public on-chain, so there is no secrecy requirement.
///
/// JWT blacklist keys use SHA-256 (VULN-007); on-chain ids stay FNV-1a.
/// Pinned by `domain/tests/fixtures/golden_vectors.json`.
pub fn on_chain_event_id(event_id: &str) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x100_0000_01b3;
    let hash = event_id.bytes().fold(FNV_OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    });
    match hash {
        0 => 1,
        other => other,
    }
}

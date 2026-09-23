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

/// The bethere-escrow instructions whose only argument is `event_id: u64`.
///
/// The discriminators mirror `#[instruction(discriminator = N)]` in
/// `bethere-escrow/src/lib.rs`; `domain/tests/escrow_ix_program_sync.rs` reads
/// that file and fails on drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventIx {
    Deposit,
    MarkCheckedIn,
    Refund,
    ClaimForfeited,
    CloseEvent,
    DeactivateEvent,
    CloseDeposit,
}

impl EventIx {
    pub const fn discriminator(self) -> u8 {
        match self {
            Self::Deposit => 1,
            Self::MarkCheckedIn => 2,
            Self::Refund => 3,
            Self::ClaimForfeited => 4,
            Self::CloseEvent => 5,
            Self::DeactivateEvent => 6,
            Self::CloseDeposit => 7,
        }
    }
}

/// Instruction data for one bethere-escrow instruction: a one-byte
/// discriminator, then each argument little-endian in declaration order.
/// Pinned by `domain/tests/fixtures/golden_vectors.json` (`escrow_ix_data`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscrowIxData {
    /// Discriminator 0.
    CreateEvent {
        event_id: u64,
        deposit_amount: u64,
        event_end: i64,
        refund_deadline: i64,
    },
    /// Discriminators 1–7: `[disc] + event_id`.
    EventScoped { ix: EventIx, event_id: u64 },
    /// Discriminator 8.
    RolloverDeposit {
        source_event_id: u64,
        target_event_id: u64,
    },
}

impl EscrowIxData {
    pub const fn discriminator(&self) -> u8 {
        match self {
            Self::CreateEvent { .. } => 0,
            Self::EventScoped { ix, .. } => ix.discriminator(),
            Self::RolloverDeposit { .. } => 8,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(33);
        data.push(self.discriminator());
        match *self {
            Self::CreateEvent {
                event_id,
                deposit_amount,
                event_end,
                refund_deadline,
            } => {
                data.extend_from_slice(&event_id.to_le_bytes());
                data.extend_from_slice(&deposit_amount.to_le_bytes());
                data.extend_from_slice(&event_end.to_le_bytes());
                data.extend_from_slice(&refund_deadline.to_le_bytes());
            }
            Self::EventScoped { event_id, .. } => {
                data.extend_from_slice(&event_id.to_le_bytes());
            }
            Self::RolloverDeposit {
                source_event_id,
                target_event_id,
            } => {
                data.extend_from_slice(&source_event_id.to_le_bytes());
                data.extend_from_slice(&target_event_id.to_le_bytes());
            }
        }
        data
    }
}

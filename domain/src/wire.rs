//! Zero-copy wire protocol envelope (Plan 014 Phase 1.3).
//!
//! Shared between the worker (encoder) and the Leptos frontend (decoder).
//! Both compile to `wasm32-unknown-unknown`, so the encode/decode primitives
//! must live here in the `domain` crate — the single source of truth.
//!
//! ## Envelope layout
//!
//! Single value ([`pack`] / [`unpack`]):
//!
//! ```text
//! +------+------+-----------+------------------+
//! | magic| ver  | reserved  | payload (Pod T)  |
//! | 4 B  | 1 B  | 3 B       | size_of::<T>()  |
//! +------+------+-----------+------------------+
//! | blake3(header || payload) — 32 B           |
//! +--------------------------------------------+
//! ```
//!
//! Batched slice ([`pack_slice`] / [`unpack_slice`]) — the shape used by
//! `GET /api/events`, `GET /api/attendees`, etc.:
//!
//! ```text
//! +------+------+-----------+--------+--------------------------+
//! | magic| ver  | reserved  | count  | payload (count × Pod T)  |
//! | 4 B  | 1 B  | 3 B       | 4 B    | count × size_of::<T>()  |
//! +------+------+-----------+--------+--------------------------+
//! | blake3(header || count || payload) — 32 B                    |
//! +---------------------------------------------------------------+
//! ```
//!
//! The hash input is `header || count || payload` so the version tag and row
//! count are themselves authenticated — a sender cannot silently downgrade the
//! format or change the row count under the receiver's nose. This mirrors the
//! katgpt-rs pattern of a version tag inside every committed blob.
//!
//! ## Content negotiation
//!
//! The JSON path stays canonical. Endpoints opt into the binary path via
//! `?fmt=bin`; the response carries [`CONTENT_TYPE`] so the frontend can
//! branch its decoder on the `Content-Type` header alone.

#![cfg(feature = "wire")]

use bytemuck::{Pod, bytes_of, cast_slice, from_bytes};

/// Wire envelope magic — ASCII `b"BTE1"` (BeThere wire v1).
pub const WIRE_MAGIC: [u8; 4] = *b"BTE1";

/// Wire envelope version. Bumped on any breaking layout change to a Pod type.
pub const WIRE_VERSION: u8 = 1;

/// Header is `[magic(4) | version(1) | reserved(3)]` — 8 bytes, 8-aligned.
pub const WIRE_HEADER_LEN: usize = 8;

/// BLAKE3 tag length appended after the payload.
pub const WIRE_TAG_LEN: usize = 32;

/// Content-Type the worker sends (and the frontend matches on) for bin paths.
pub const CONTENT_TYPE: &str = "application/x-bethere-bin";

/// Pack a Pod value into a versioned, BLAKE3-committed envelope.
///
/// Layout: `[magic(4) | version(1) | reserved(3)] || payload || blake3(32)`.
///
/// Allocates one `Vec<u8>` of exactly `WIRE_HEADER_LEN + size_of::<T>() + 32`
/// bytes. On the worker this is called once per response — the allocation is
/// amortized across the HTTP send.
///
/// For batch endpoints (list responses) use [`pack_slice`] instead — it adds a
/// `u32` row count to the envelope and hashes it in, and lets the receiver
/// return a `&[T]` rather than a single `&T`.
pub fn pack<T: Pod>(value: &T) -> Vec<u8> {
    let payload = bytes_of(value);
    let mut header = [0u8; WIRE_HEADER_LEN];
    header[..4].copy_from_slice(&WIRE_MAGIC);
    header[4] = WIRE_VERSION;

    let mut hash_input = Vec::with_capacity(WIRE_HEADER_LEN + payload.len());
    hash_input.extend_from_slice(&header);
    hash_input.extend_from_slice(payload);

    let hash = blake3::hash(&hash_input);

    let mut out = hash_input;
    out.extend_from_slice(hash.as_bytes());
    out
}

/// Length (in bytes) of the row-count prefix on slice envelopes.
///
/// Stored as little-endian `u32`, immediately after the 8-byte header. Capped
/// at 32 bits — that's 4G rows of `u8`, far past any realistic batch response
/// (`GET /api/contacts/audience` tops out around 1k rows).
pub const WIRE_COUNT_LEN: usize = 4;

/// Pack a slice of Pod values into a versioned, BLAKE3-committed envelope.
///
/// Layout: `[magic(4) | version(1) | reserved(3)] || count_le(4) || payload ||
/// blake3(32)`. The little-endian row count is part of the hashed body so the
/// receiver can detect a row-count mismatch without a separate field.
///
/// This is the natural shape for batch endpoints (`GET /api/events`, etc.). The
/// receiver calls [`unpack_slice`] to get back a borrowed `&[T]` — zero copy,
/// zero allocation on the happy path. Each row is `bytemuck::cast_slice`'d
/// in place from the payload bytes.
///
/// Allocates one `Vec<u8>` of exactly `WIRE_HEADER_LEN + WIRE_COUNT_LEN +
/// count × size_of::<T>() + WIRE_TAG_LEN` bytes.
pub fn pack_slice<T: Pod>(values: &[T]) -> Vec<u8> {
    let payload = cast_slice::<T, u8>(values);
    let mut header = [0u8; WIRE_HEADER_LEN];
    header[..4].copy_from_slice(&WIRE_MAGIC);
    header[4] = WIRE_VERSION;

    let count_le: [u8; WIRE_COUNT_LEN] = (values.len() as u32).to_le_bytes();

    let mut hash_input = Vec::with_capacity(WIRE_HEADER_LEN + WIRE_COUNT_LEN + payload.len());
    hash_input.extend_from_slice(&header);
    hash_input.extend_from_slice(&count_le);
    hash_input.extend_from_slice(payload);

    let hash = blake3::hash(&hash_input);

    let mut out = hash_input;
    out.extend_from_slice(hash.as_bytes());
    out
}

/// Validate magic + version + BLAKE3, then return a zero-copy view of the
/// payload as `&[T]`. The row count is read from the envelope and used to bound
/// the returned slice; it is also covered by the BLAKE3 tag, so a sender cannot
/// change it without invalidating the hash.
///
/// On the happy path the only allocations are the BLAKE3 hasher's internal
/// scratch (one-time) — the returned slice is a `bytemuck::cast_slice` of the
/// input buffer.
///
/// Returns the same error set as [`unpack`], plus [`WireError::Truncated`] when
/// the encoded row count doesn't fit the body.
pub fn unpack_slice<T: Pod + Copy>(buf: &[u8]) -> Result<&[T], WireError> {
    let needed_min = WIRE_HEADER_LEN + WIRE_COUNT_LEN + WIRE_TAG_LEN;
    if buf.len() < needed_min {
        return Err(WireError::Truncated {
            got: buf.len(),
            want: needed_min,
        });
    }
    if buf[..4] != WIRE_MAGIC {
        return Err(WireError::BadMagic);
    }
    let got = buf[4];
    if got != WIRE_VERSION {
        return Err(WireError::UnsupportedVersion {
            got,
            want: WIRE_VERSION,
        });
    }

    // Panic-free fixed-index read. Safety is established by the bounds check
    // above (`buf.len() >= WIRE_HEADER_LEN + WIRE_COUNT_LEN + WIRE_TAG_LEN`),
    // but we avoid `try_into().expect(...)` so the decode path has zero
    // panic sites — a hard requirement for production WASM receivers.
    let count = u32::from_le_bytes([
        buf[WIRE_HEADER_LEN],
        buf[WIRE_HEADER_LEN + 1],
        buf[WIRE_HEADER_LEN + 2],
        buf[WIRE_HEADER_LEN + 3],
    ]) as usize;
    let payload_len = count.checked_mul(size_of::<T>()).ok_or({
        // Overflow only possible on adversarial input. Treat as truncation —
        // the body can't possibly contain that many rows.
        WireError::Truncated {
            got: buf.len(),
            want: usize::MAX,
        }
    })?;
    let body_start = WIRE_HEADER_LEN + WIRE_COUNT_LEN;
    let body_end = body_start + payload_len;
    if buf.len() < body_end + WIRE_TAG_LEN {
        return Err(WireError::Truncated {
            got: buf.len(),
            want: body_end + WIRE_TAG_LEN,
        });
    }

    // Hash covers header + count + payload, mirroring `pack_slice` exactly.
    let header_with_count_and_payload = &buf[..body_end];
    let payload = &buf[body_start..body_end];
    let tag = &buf[body_end..body_end + WIRE_TAG_LEN];

    let expected = blake3::hash(header_with_count_and_payload);
    if expected.as_bytes() != tag {
        return Err(WireError::HashMismatch);
    }
    // Zero-copy: bytemuck casts the existing slice in place.
    Ok(cast_slice::<u8, T>(payload))
}

/// Validate magic + version, recompute BLAKE3, and return a zero-copy view of
/// the payload. No allocation on the happy path — `from_bytes` is a cast.
///
/// Errors mirror the failure modes a real receiver must handle: truncated body,
/// wrong magic (not a BeThere wire payload), version skew (sender newer/older
/// than receiver), and corruption (BLAKE3 mismatch).
pub fn unpack<T: Pod + Copy>(buf: &[u8]) -> Result<&T, WireError> {
    let payload_len = size_of::<T>();
    let needed = WIRE_HEADER_LEN + payload_len + WIRE_TAG_LEN;
    if buf.len() < needed {
        return Err(WireError::Truncated {
            got: buf.len(),
            want: needed,
        });
    }
    if buf[..4] != WIRE_MAGIC {
        return Err(WireError::BadMagic);
    }
    let got = buf[4];
    if got != WIRE_VERSION {
        return Err(WireError::UnsupportedVersion {
            got,
            want: WIRE_VERSION,
        });
    }
    let split = WIRE_HEADER_LEN + payload_len;
    let header_with_payload = &buf[..split];
    let payload = &buf[WIRE_HEADER_LEN..split];
    let tag = &buf[split..split + WIRE_TAG_LEN];

    let expected = blake3::hash(header_with_payload);
    if expected.as_bytes() != tag {
        return Err(WireError::HashMismatch);
    }
    // Zero-copy: casts the existing slice without allocating a T.
    Ok(from_bytes(payload))
}

/// Errors returned by [`unpack`]. Ordered by the receiver's decision tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    /// Body shorter than `header + payload + tag`. Includes observed/needed.
    Truncated { got: usize, want: usize },
    /// Magic bytes don't match [`WIRE_MAGIC`] — not a BeThere wire payload.
    BadMagic,
    /// Version byte doesn't match [`WIRE_VERSION`] — sender/receiver skew.
    UnsupportedVersion { got: u8, want: u8 },
    /// BLAKE3 recomputation mismatch — payload or header corrupted in transit.
    HashMismatch,
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated { got, want } => {
                write!(f, "wire body truncated: got {got} bytes, need {want}")
            }
            Self::BadMagic => write!(f, "wire magic mismatch — not a BeThere payload"),
            Self::UnsupportedVersion { got, want } => {
                write!(
                    f,
                    "wire version skew: got v{got}, receiver supports v{want}"
                )
            }
            Self::HashMismatch => write!(f, "wire BLAKE3 mismatch — payload corrupted"),
        }
    }
}

impl std::error::Error for WireError {}

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::Zeroable;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
    #[repr(C)]
    struct Sample {
        a: u32,
        b: u32,
    }

    #[test]
    fn pack_unpack_round_trips() {
        let original = Sample { a: 1, b: 2 };
        let encoded = pack(&original);
        let decoded: &Sample = unpack(&encoded).expect("round-trip");
        assert_eq!(decoded, &original);
    }

    #[test]
    fn pack_size_is_header_plus_payload_plus_tag() {
        let encoded = pack(&Sample { a: 0, b: 0 });
        assert_eq!(
            encoded.len(),
            WIRE_HEADER_LEN + size_of::<Sample>() + WIRE_TAG_LEN
        );
    }

    #[test]
    fn pack_slice_round_trips_empty() {
        let empty: [Sample; 0] = [];
        let encoded = pack_slice(&empty);
        let decoded: &[Sample] = unpack_slice(&encoded).expect("round-trip empty");
        assert!(decoded.is_empty());
    }

    #[test]
    fn pack_slice_round_trips_many() {
        let values: Vec<Sample> = (0..500u32)
            .map(|i| Sample {
                a: i,
                b: i.wrapping_mul(7),
            })
            .collect();
        let encoded = pack_slice(&values);
        let decoded: &[Sample] = unpack_slice(&encoded).expect("round-trip many");
        assert_eq!(decoded.len(), values.len());
        assert_eq!(decoded, values.as_slice());
    }

    #[test]
    fn pack_slice_size_matches_layout() {
        let values = [Sample { a: 1, b: 2 }, Sample { a: 3, b: 4 }];
        let encoded = pack_slice(&values);
        assert_eq!(
            encoded.len(),
            WIRE_HEADER_LEN + WIRE_COUNT_LEN + (values.len() * size_of::<Sample>()) + WIRE_TAG_LEN
        );
    }

    #[test]
    fn pack_slice_truncated_is_rejected() {
        let values = vec![Sample { a: 1, b: 2 }; 10];
        let mut encoded = pack_slice(&values);
        encoded.truncate(encoded.len() - 1);
        assert!(matches!(
            unpack_slice::<Sample>(&encoded),
            Err(WireError::Truncated { .. })
        ));
    }

    #[test]
    fn pack_slice_corruption_invalidates_hash() {
        let values = vec![Sample { a: 1, b: 2 }; 10];
        let mut encoded = pack_slice(&values);
        // Flip a byte in the payload body (after header + count).
        encoded[WIRE_HEADER_LEN + WIRE_COUNT_LEN] ^= 0xFF;
        assert_eq!(
            unpack_slice::<Sample>(&encoded),
            Err(WireError::HashMismatch)
        );
    }

    #[test]
    fn truncated_is_rejected() {
        let mut encoded = pack(&Sample { a: 1, b: 2 });
        encoded.truncate(encoded.len() - 1);
        assert!(matches!(
            unpack::<Sample>(&encoded),
            Err(WireError::Truncated { .. })
        ));
    }
}

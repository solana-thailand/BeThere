//! The short booking code printed under the ticket QR (`.issues/178`).
//!
//! Six characters an attendee can read aloud at the door and staff can type
//! into the scanner. It is **not** a credential:
//!
//! - it is random, never derived from `claim_token` (a capability, and a
//!   prefix of it would be a partial secret, `.issues/071`) or from the
//!   attendee id (global across events, `.issues/153`);
//! - it is unique only within one event (`idx_attendees_event_display_code`,
//!   migration 0058), so every lookup must be scoped by event;
//! - the only thing that accepts it is the staff, event-scoped lookup. Claim
//!   and deposit endpoints never do.
//!
//! The type carries no randomness source, so it builds for wasm32 in both the
//! worker and the frontend: the caller supplies bytes from its own CSPRNG
//! (`crypto.getRandomValues` in the worker).

use std::fmt;

/// The characters a code is drawn from: digits and capitals without the
/// look-alikes `0`/`O`, `1`/`I`/`L`. 31 symbols, so 31^6 ≈ 8.9 × 10^8 codes
/// per event.
pub const DISPLAY_CODE_ALPHABET: &[u8; 31] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";

/// Characters in a code.
pub const DISPLAY_CODE_LEN: usize = 6;

/// Random bytes a caller should hand to [`DisplayCode::from_random_bytes`].
///
/// Rejection sampling discards a byte with probability 8/256, so 16 bytes
/// leave fewer than 6 usable ones with odds far below 10^-12. A `None` from
/// that rare case is a cue to draw again, not an error.
pub const DISPLAY_CODE_RANDOM_BYTES: usize = 16;

/// Largest multiple of the alphabet size that fits in a byte (8 × 31). Bytes
/// at or above it are rejected so `byte % 31` stays uniform.
const UNBIASED_BYTE_LIMIT: u8 =
    (256 / DISPLAY_CODE_ALPHABET.len() * DISPLAY_CODE_ALPHABET.len()) as u8;

/// Why typed or stored text is not a display code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayCodeError {
    /// Not exactly [`DISPLAY_CODE_LEN`] characters after normalisation.
    Length(usize),
    /// A character outside [`DISPLAY_CODE_ALPHABET`] (includes `0 O 1 I L`).
    Character(char),
}

impl fmt::Display for DisplayCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length(len) => write!(
                f,
                "a ticket code has {DISPLAY_CODE_LEN} characters, got {len}"
            ),
            Self::Character(c) => write!(f, "'{c}' is not used in ticket codes"),
        }
    }
}

impl std::error::Error for DisplayCodeError {}

/// A validated 6-character booking display code.
#[derive(Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DisplayCode([u8; DISPLAY_CODE_LEN]);

impl DisplayCode {
    /// Build a code from CSPRNG bytes by rejection sampling.
    ///
    /// `None` when fewer than [`DISPLAY_CODE_LEN`] bytes fall under the
    /// unbiased limit; draw fresh bytes and call again.
    pub fn from_random_bytes(bytes: &[u8]) -> Option<Self> {
        let mut out = [0u8; DISPLAY_CODE_LEN];
        let mut filled = 0;
        for &b in bytes {
            if filled == DISPLAY_CODE_LEN {
                break;
            }
            if b >= UNBIASED_BYTE_LIMIT {
                continue;
            }
            out[filled] = DISPLAY_CODE_ALPHABET[usize::from(b) % DISPLAY_CODE_ALPHABET.len()];
            filled += 1;
        }
        (filled == DISPLAY_CODE_LEN).then_some(Self(out))
    }

    /// Parse what a person typed or read aloud.
    ///
    /// Trims, drops an optional leading `Nº` / `nº` / `№` (the ticket prints one),
    /// drops spaces and dashes anywhere, and uppercases ASCII. Nothing else is
    /// mapped: an `O`, `0`, `I`, `1` or `L` is rejected rather than guessed,
    /// because guessing would turn one mistyped code into someone else's.
    pub fn parse(input: &str) -> Result<Self, DisplayCodeError> {
        let trimmed = input.trim();
        let body = ["Nº", "nº", "№"]
            .iter()
            .find_map(|prefix| trimmed.strip_prefix(prefix))
            .unwrap_or(trimmed);
        let mut out = [0u8; DISPLAY_CODE_LEN];
        let mut len = 0usize;
        for c in body.chars() {
            if c.is_whitespace() || c == '-' {
                continue;
            }
            let upper = c.to_ascii_uppercase();
            let allowed = upper.is_ascii() && DISPLAY_CODE_ALPHABET.contains(&(upper as u8));
            if !allowed {
                return Err(DisplayCodeError::Character(c));
            }
            if len < DISPLAY_CODE_LEN {
                out[len] = upper as u8;
            }
            len += 1;
        }
        match len == DISPLAY_CODE_LEN {
            true => Ok(Self(out)),
            false => Err(DisplayCodeError::Length(len)),
        }
    }

    /// The code as stored and printed (without the `Nº` prefix).
    pub fn as_str(&self) -> &str {
        // Every byte comes from the ASCII alphabet, so this cannot fail.
        std::str::from_utf8(&self.0).unwrap_or_default()
    }
}

impl fmt::Display for DisplayCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for DisplayCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DisplayCode({})", self.as_str())
    }
}

impl std::str::FromStr for DisplayCode {
    type Err = DisplayCodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl TryFrom<String> for DisplayCode {
    type Error = DisplayCodeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<DisplayCode> for String {
    fn from(code: DisplayCode) -> Self {
        code.as_str().to_string()
    }
}

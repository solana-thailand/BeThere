//! URL slugs from organiser text: event ids, org ids, feedback series keys.
//!
//! The ASCII kebab form is kept wherever a name has one, so an ASCII name
//! gets the slug it always got. A name written in Thai (or emoji) has
//! none: "เวิร์กช็อป" gave an empty id, and "series-" for every all-Thai
//! series. Those get a short hash of the text instead (build plan Phase 1.0).
//!
//! The hash is deterministic, not random: the feedback series key is looked up
//! again on every request, so the same title must give the same slug. Event
//! ids still go through `deduplicate_slug`, which settles collisions.

/// An ASCII slug shorter than this, from a name that lost letters on the way
/// (`"ครั้งที่ 1"` → `"1"`), says nothing; it is replaced by the hash.
pub const MIN_ASCII_SLUG_LEN: usize = 3;

/// Width of the base-36 hash body: 36^6 ≈ 2.2e9 values.
const HASH_LEN: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Slug {
    /// The kebab form of the ASCII letters and digits; empty for blank input.
    Ascii(String),
    /// The name kept too little ASCII; a short hash of the trimmed,
    /// lowercased text.
    Hashed(String),
}

impl Slug {
    pub fn from_text(input: &str) -> Slug {
        let trimmed = input.trim();
        let ascii = ascii_kebab(trimmed);
        let lost_letters = trimmed
            .chars()
            .any(|c| !c.is_ascii() && c.is_alphanumeric());
        match (
            ascii.is_empty(),
            lost_letters && ascii.len() < MIN_ASCII_SLUG_LEN,
        ) {
            (true, _) if trimmed.is_empty() => Slug::Ascii(ascii),
            (true, _) | (false, true) => Slug::Hashed(short_hash(&trimmed.to_lowercase())),
            (false, false) => Slug::Ascii(ascii),
        }
    }

    /// `Ascii` as is; `Hashed` as `{prefix}-{hash}`, e.g. `event-k3m9x2`.
    pub fn or_prefixed(self, prefix: &str) -> String {
        match self {
            Slug::Ascii(s) => s,
            Slug::Hashed(h) => format!("{prefix}-{h}"),
        }
    }

    /// The slug body without a prefix, for callers that add their own.
    pub fn into_inner(self) -> String {
        match self {
            Slug::Ascii(s) | Slug::Hashed(s) => s,
        }
    }
}

/// Lowercase, every run of non-ASCII-alphanumerics becomes one `-`, no `-`
/// at either end. The behaviour of the four builders this replaces.
fn ascii_kebab(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c.is_ascii_alphanumeric() {
            true => out.push(c.to_ascii_lowercase()),
            false if !out.is_empty() && !out.ends_with('-') => out.push('-'),
            false => {}
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    out
}

/// FNV-1a 64 in base 36, first `HASH_LEN` digits. Not a security hash: it
/// only has to be stable across builds and targets (native and wasm32).
fn short_hash(text: &str) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = text
        .bytes()
        .fold(OFFSET, |h, b| (h ^ u64::from(b)).wrapping_mul(PRIME));
    let mut out = String::with_capacity(HASH_LEN);
    for _ in 0..HASH_LEN {
        let d = (h % 36) as u8;
        out.push(char::from(match d {
            0..=9 => b'0' + d,
            _ => b'a' + d - 10,
        }));
        h /= 36;
    }
    out
}

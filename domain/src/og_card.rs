//! Per-event share cards (`.issues/183`, option B).
//!
//! The organizer's browser draws a 1200×630 PNG when the event is saved and
//! uploads it to R2 under `og/{event_id}.png`; the Worker names it in the
//! `og:image` of `/e/{slug}`. Both sides need the same few rules, so they
//! live here:
//!
//! - [`check_og_png`]: what the upload route accepts (PNG magic bytes, the
//!   IHDR size, a byte cap). The label (`Content-Type`) is never trusted.
//! - [`event_when`] / [`event_summary`]: the date line on the card and in the
//!   `og:description`, so the picture and the text never disagree.

use chrono::{DateTime, FixedOffset};

/// The card size every platform crops to (1.91:1).
pub const OG_WIDTH: u32 = 1200;
pub const OG_HEIGHT: u32 = 630;

/// Upload cap. A 1200×630 canvas PNG with the riso grain lands well under
/// 1 MB; 2 MB leaves room and keeps Worker memory bounded.
pub const OG_MAX_BYTES: usize = 2 * 1024 * 1024;

/// Bangkok, where every event so far has been held (`handlers/crawl.rs`
/// makes the same assumption for `llms.txt`).
pub const EVENT_UTC_OFFSET_SECS: i32 = 7 * 3600;

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Why an upload is not a share card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OgPngError {
    Empty,
    TooLarge { bytes: usize },
    NotPng,
    WrongSize { width: u32, height: u32 },
}

impl std::fmt::Display for OgPngError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "share card body is empty"),
            Self::TooLarge { bytes } => write!(
                f,
                "share card exceeds {} MB (got {bytes} bytes)",
                OG_MAX_BYTES / (1024 * 1024)
            ),
            Self::NotPng => write!(f, "share card must be a PNG file"),
            Self::WrongSize { width, height } => write!(
                f,
                "share card must be {OG_WIDTH}x{OG_HEIGHT} (got {width}x{height})"
            ),
        }
    }
}

/// Width and height from a PNG's IHDR chunk, which the spec requires to come
/// first: signature (8), chunk length (4), `IHDR` (4), width (4), height (4).
pub fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.get(..8)? != PNG_SIGNATURE || bytes.get(12..16)? != b"IHDR" {
        return None;
    }
    let be = |at: usize| -> Option<u32> {
        let raw: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
        Some(u32::from_be_bytes(raw))
    };
    Some((be(16)?, be(20)?))
}

/// Accept only a PNG of exactly [`OG_WIDTH`]×[`OG_HEIGHT`] within
/// [`OG_MAX_BYTES`].
pub fn check_og_png(bytes: &[u8]) -> Result<(), OgPngError> {
    if bytes.is_empty() {
        return Err(OgPngError::Empty);
    }
    if bytes.len() > OG_MAX_BYTES {
        return Err(OgPngError::TooLarge { bytes: bytes.len() });
    }
    match png_dimensions(bytes) {
        None => Err(OgPngError::NotPng),
        Some((OG_WIDTH, OG_HEIGHT)) => Ok(()),
        Some((width, height)) => Err(OgPngError::WrongSize { width, height }),
    }
}

/// The start, for people: `Sat 12 Oct 2026 · 18:00 (UTC+7)`.
pub fn event_when(start_ms: i64, time_tba: bool) -> String {
    const TBA: &str = "Date to be announced";
    if time_tba || start_ms <= 0 {
        return TBA.to_string();
    }
    let at = DateTime::from_timestamp_millis(start_ms);
    match (at, FixedOffset::east_opt(EVENT_UTC_OFFSET_SECS)) {
        (Some(at), Some(tz)) => at
            .with_timezone(&tz)
            .format("%a %-d %b %Y · %H:%M (UTC+7)")
            .to_string(),
        _ => TBA.to_string(),
    }
}

/// When and where on one line: `Sat 12 Oct 2026 · 18:00 (UTC+7) · Bangkok`.
pub fn event_summary(start_ms: i64, time_tba: bool, location: &str) -> String {
    let when = event_when(start_ms, time_tba);
    match location.split_whitespace().collect::<Vec<_>>().join(" ") {
        place if place.is_empty() => when,
        place => format!("{when} · {place}"),
    }
}
